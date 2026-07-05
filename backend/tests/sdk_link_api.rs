//! SDK komut kanalı + STT ayar senkronu uçtan-uca testleri (Faz 2.6b).
//!
//! Akış: panel komut bırakır → SDK yoklar (tek seferlik teslim) → benchmark
//! raporlar → panel PUT /api/stt/mode ile modu değiştirir → policy yanıtı
//! mod + benchmark'ı yansıtır.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use streamdubber_backend::{
    app,
    auth::jwt::{Claims, Jwt},
    config::ServiceConfig,
    gateway::{stt::MockStt, translate::MockTranslate, Gateway},
    state::AppState,
};

const SECRET: &[u8] = b"sdk-link-test-secret";

async fn spawn_server() -> (SocketAddr, Arc<ServiceConfig>) {
    let service_config = Arc::new(ServiceConfig::with_defaults());
    let mut gateway = Gateway::new(Arc::clone(&service_config));
    gateway.register_stt(Arc::new(MockStt::new("deepgram")));
    gateway.register_translate(Arc::new(MockTranslate::new("deepl")));

    let state = AppState::skeleton(Jwt::new(SECRET), Arc::clone(&service_config), gateway);

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        axum::serve(listener, app::router(state)).await.expect("serve");
    });
    (addr, service_config)
}

fn token_for(sub: &str, plan: &str) -> String {
    let jwt = Jwt::new(SECRET);
    let claims = Claims {
        sub: sub.into(),
        role: "broadcaster".into(),
        plan: plan.into(),
        exp: chrono::Utc::now().timestamp() + 3600,
    };
    jwt.issue(&claims).expect("issue token")
}

async fn http(addr: SocketAddr, request: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    stream.write_all(request.as_bytes()).await.expect("write");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.expect("read");
    let text = String::from_utf8_lossy(&buf).to_string();
    let status: u16 = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .expect("status line");
    let body = text.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    (status, body)
}

async fn request(
    addr: SocketAddr,
    method: &str,
    path: &str,
    bearer: Option<&str>,
    body: Option<&str>,
) -> (u16, String) {
    let auth = bearer
        .map(|t| format!("Authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    let body = body.unwrap_or("");
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\n{auth}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    http(addr, &req).await
}

#[tokio::test]
async fn command_queue_full_flow_one_shot_delivery() {
    let (addr, _config) = spawn_server().await;
    let panel = token_for("user-a", "free");

    // Panel benchmark komutu bırakır.
    let (status, body) = request(
        addr,
        "POST",
        "/api/sdk/commands",
        Some(&panel),
        Some(r#"{"command":"benchmark"}"#),
    )
    .await;
    assert_eq!(status, 202);
    let cmd: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(cmd["command"], "benchmark");
    assert!(!cmd["id"].as_str().unwrap_or_default().is_empty());

    // SDK yoklar — komut gelir.
    let (status, body) = request(addr, "GET", "/api/sdk/commands", Some(&panel), None).await;
    assert_eq!(status, 200);
    let got: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(got["commands"].as_array().unwrap().len(), 1);

    // İkinci yoklama boş — tek seferlik teslim.
    let (_, body) = request(addr, "GET", "/api/sdk/commands", Some(&panel), None).await;
    let got: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert!(got["commands"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn commands_are_isolated_per_user() {
    let (addr, _config) = spawn_server().await;
    let user_a = token_for("user-a", "free");
    let user_b = token_for("user-b", "free");

    request(
        addr,
        "POST",
        "/api/sdk/commands",
        Some(&user_a),
        Some(r#"{"command":"benchmark"}"#),
    )
    .await;

    // B'nin kuyruğu boş; A'nınki dolu.
    let (_, body) = request(addr, "GET", "/api/sdk/commands", Some(&user_b), None).await;
    let got: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert!(got["commands"].as_array().unwrap().is_empty());

    let (_, body) = request(addr, "GET", "/api/sdk/commands", Some(&user_a), None).await;
    let got: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(got["commands"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn unknown_command_rejected_and_auth_required() {
    let (addr, _config) = spawn_server().await;
    let panel = token_for("user-a", "free");

    let (status, _) = request(
        addr,
        "POST",
        "/api/sdk/commands",
        Some(&panel),
        Some(r#"{"command":"rm-rf"}"#),
    )
    .await;
    assert_eq!(status, 422);

    for (method, path) in [
        ("POST", "/api/sdk/commands"),
        ("GET", "/api/sdk/commands"),
        ("POST", "/api/sdk/benchmark"),
        ("PUT", "/api/stt/mode"),
    ] {
        let (status, _) = request(addr, method, path, None, Some("{}")).await;
        assert_eq!(status, 401, "{method} {path} auth istemeli");
    }
}

#[tokio::test]
async fn local_mode_requires_passed_benchmark() {
    let (addr, _config) = spawn_server().await;
    let panel = token_for("user-a", "free");

    // Benchmark yokken local'e geçiş 409.
    let (status, body) = request(
        addr,
        "PUT",
        "/api/stt/mode",
        Some(&panel),
        Some(r#"{"mode":"local"}"#),
    )
    .await;
    assert_eq!(status, 409);
    assert!(body.contains("benchmark_required"));

    // Kalan (geçmeyen) benchmark da yetmez.
    request(
        addr,
        "POST",
        "/api/sdk/benchmark",
        Some(&panel),
        Some(r#"{"passed":false,"rtf":1.9,"p95_ms":4200}"#),
    )
    .await;
    let (status, _) = request(
        addr,
        "PUT",
        "/api/stt/mode",
        Some(&panel),
        Some(r#"{"mode":"local"}"#),
    )
    .await;
    assert_eq!(status, 409);

    // Geçen benchmark sonrası local kabul edilir.
    let (status, body) = request(
        addr,
        "POST",
        "/api/sdk/benchmark",
        Some(&panel),
        Some(r#"{"passed":true,"rtf":0.31,"p95_ms":640,"gpu_name":"RTX 3060","vram_mb":12288}"#),
    )
    .await;
    assert_eq!(status, 200);
    let report: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert!(report["reported_at"].as_i64().unwrap_or(0) > 0);

    let (status, body) = request(
        addr,
        "PUT",
        "/api/stt/mode",
        Some(&panel),
        Some(r#"{"mode":"local"}"#),
    )
    .await;
    assert_eq!(status, 200);
    let got: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(got["mode"], "local");

    // Policy yanıtı mod + benchmark'ı yansıtır (SDK tek çağrıda okur).
    let (_, body) = request(addr, "GET", "/api/stt/policy", Some(&panel), None).await;
    let policy: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(policy["mode"], "local");
    assert_eq!(policy["benchmark"]["passed"], true);
    assert_eq!(policy["benchmark"]["gpu_name"], "RTX 3060");

    // Cloud'a dönüş şartsız.
    let (status, _) = request(
        addr,
        "PUT",
        "/api/stt/mode",
        Some(&panel),
        Some(r#"{"mode":"cloud"}"#),
    )
    .await;
    assert_eq!(status, 200);
}

#[tokio::test]
async fn local_mode_denied_when_plan_not_allowed() {
    let (addr, config) = spawn_server().await;
    config.set("stt.local.plans", "pro,streamer");
    let panel = token_for("user-a", "free");

    // Geçen benchmark olsa bile plan izni yoksa 403.
    request(
        addr,
        "POST",
        "/api/sdk/benchmark",
        Some(&panel),
        Some(r#"{"passed":true,"rtf":0.3,"p95_ms":500}"#),
    )
    .await;
    let (status, body) = request(
        addr,
        "PUT",
        "/api/stt/mode",
        Some(&panel),
        Some(r#"{"mode":"local"}"#),
    )
    .await;
    assert_eq!(status, 403);
    assert!(body.contains("local_stt_not_allowed_for_plan"));
}
