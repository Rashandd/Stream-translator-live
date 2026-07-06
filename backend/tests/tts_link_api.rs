//! TTS ayar senkronu uçtan-uca testleri (Faz 2.2b — `sdk_link_api.rs` ile
//! aynı desen, TTS/XTTS-v2 tarafı için).
//!
//! Akış: panel `tts_benchmark` komutu bırakır (komut kanalı STT ile
//! paylaşılır) → SDK yoklar → benchmark raporlar → panel PUT /api/tts/mode
//! ile modu değiştirir → policy yanıtı mod + benchmark'ı yansıtır.

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

const SECRET: &[u8] = b"tts-link-test-secret";

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
async fn tts_benchmark_command_shares_the_queue_with_stt() {
    let (addr, _config) = spawn_server().await;
    let panel = token_for("user-a", "pro");

    let (status, body) = request(
        addr,
        "POST",
        "/api/sdk/commands",
        Some(&panel),
        Some(r#"{"command":"tts_benchmark"}"#),
    )
    .await;
    assert_eq!(status, 202, "body: {body}");
    let cmd: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(cmd["command"], "tts_benchmark");

    let (status, body) = request(addr, "GET", "/api/sdk/commands", Some(&panel), None).await;
    assert_eq!(status, 200);
    let got: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(got["commands"][0]["command"], "tts_benchmark");
}

#[tokio::test]
async fn tts_routes_require_auth() {
    let (addr, _config) = spawn_server().await;
    for (method, path) in [
        ("GET", "/api/tts/policy"),
        ("PUT", "/api/tts/mode"),
        ("POST", "/api/tts/benchmark"),
    ] {
        let (status, _) = request(addr, method, path, None, Some("{}")).await;
        assert_eq!(status, 401, "{method} {path} auth istemeli");
    }
}

#[tokio::test]
async fn local_mode_requires_passed_benchmark() {
    let (addr, _config) = spawn_server().await;
    // tts.local.plans varsayılanı "pro,streamer" — pro burada yeterli.
    let panel = token_for("user-a", "pro");

    let (status, body) = request(
        addr,
        "PUT",
        "/api/tts/mode",
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
        "/api/tts/benchmark",
        Some(&panel),
        Some(r#"{"passed":false,"rtf":1.4,"p95_ms":3800}"#),
    )
    .await;
    let (status, _) = request(
        addr,
        "PUT",
        "/api/tts/mode",
        Some(&panel),
        Some(r#"{"mode":"local"}"#),
    )
    .await;
    assert_eq!(status, 409);

    // Geçen benchmark sonrası local kabul edilir.
    let (status, body) = request(
        addr,
        "POST",
        "/api/tts/benchmark",
        Some(&panel),
        Some(r#"{"passed":true,"rtf":0.28,"p95_ms":1800,"gpu_name":"RTX 3070","vram_mb":8192}"#),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    let report: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert!(report["reported_at"].as_i64().unwrap_or(0) > 0);

    let (status, body) = request(
        addr,
        "PUT",
        "/api/tts/mode",
        Some(&panel),
        Some(r#"{"mode":"local"}"#),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    let got: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(got["mode"], "local");

    // Policy yanıtı mod + benchmark'ı yansıtır (SDK tek çağrıda okur).
    let (_, body) = request(addr, "GET", "/api/tts/policy", Some(&panel), None).await;
    let policy: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(policy["mode"], "local");
    assert_eq!(policy["benchmark"]["passed"], true);
    assert_eq!(policy["benchmark"]["gpu_name"], "RTX 3070");
    assert_eq!(policy["watchdog"]["p95_ms"], 2000);

    // Cloud'a dönüş şartsız.
    let (status, _) = request(
        addr,
        "PUT",
        "/api/tts/mode",
        Some(&panel),
        Some(r#"{"mode":"cloud"}"#),
    )
    .await;
    assert_eq!(status, 200);
}

#[tokio::test]
async fn local_mode_denied_for_free_plan_by_default() {
    let (addr, _config) = spawn_server().await;
    // tts.local.plans varsayılanı "pro,streamer" — free zaten dışarıda,
    // config'i özellikle daraltmaya gerek yok (has_voice_clone ile aynı kilit).
    let panel = token_for("user-a", "free");

    request(
        addr,
        "POST",
        "/api/tts/benchmark",
        Some(&panel),
        Some(r#"{"passed":true,"rtf":0.2,"p95_ms":900}"#),
    )
    .await;
    let (status, body) = request(
        addr,
        "PUT",
        "/api/tts/mode",
        Some(&panel),
        Some(r#"{"mode":"local"}"#),
    )
    .await;
    assert_eq!(status, 403);
    assert!(body.contains("local_tts_not_allowed_for_plan"));
}

#[tokio::test]
async fn tts_and_stt_modes_are_independent_for_the_same_user() {
    let (addr, _config) = spawn_server().await;
    let panel = token_for("user-a", "pro");

    request(
        addr,
        "POST",
        "/api/sdk/benchmark",
        Some(&panel),
        Some(r#"{"passed":true,"rtf":0.3,"p95_ms":500}"#),
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
    assert_eq!(status, 200);

    // TTS hâlâ cloud (STT modunu değiştirmek TTS'i etkilemez).
    let (_, body) = request(addr, "GET", "/api/tts/policy", Some(&panel), None).await;
    let policy: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(policy["mode"], "cloud");
    assert!(policy["benchmark"].is_null());
}
