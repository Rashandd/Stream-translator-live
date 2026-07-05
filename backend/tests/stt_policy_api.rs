//! `GET /api/stt/policy` uçtan-uca testleri (Faz 2.6b — docs/local-stt-plan.md).

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

const SECRET: &[u8] = b"stt-policy-test-secret";

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

fn token(plan: &str) -> String {
    let jwt = Jwt::new(SECRET);
    let claims = Claims {
        sub: "00000000-0000-0000-0000-00000000aaaa".into(),
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

async fn get_policy(addr: SocketAddr, bearer: Option<&str>) -> (u16, String) {
    let auth = bearer
        .map(|t| format!("Authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    let req = format!(
        "GET /api/stt/policy HTTP/1.1\r\nHost: localhost\r\n{auth}Connection: close\r\n\r\n"
    );
    http(addr, &req).await
}

#[tokio::test]
async fn policy_returns_seed_defaults_for_allowed_plan() {
    let (addr, _config) = spawn_server().await;
    let (status, body) = get_policy(addr, Some(&token("free"))).await;
    assert_eq!(status, 200);

    let json: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(json["local_allowed"], true);
    assert_eq!(json["usage_multiplier"], 0.5);
    assert_eq!(json["requirements"]["min_vram_mb"], 6144);
    assert_eq!(json["requirements"]["benchmark_rtf_max"], 0.5);
    assert_eq!(json["watchdog"]["p95_ms"], 1500);
    assert_eq!(json["watchdog"]["retry_minutes"], 5);
    assert_eq!(json["max_chars"], 500);
}

#[tokio::test]
async fn policy_requires_auth() {
    let (addr, _config) = spawn_server().await;
    let (status, _body) = get_policy(addr, None).await;
    assert_eq!(status, 401);
}

#[tokio::test]
async fn policy_reflects_runtime_config_change_without_restart() {
    let (addr, config) = spawn_server().await;

    // Admin lokal STT'yi pro'ya daraltır ve eşiği değiştirir — restart yok.
    config.set("stt.local.plans", "pro");
    config.set("stt.local.watchdog_p95_ms", "2000");

    let (status, body) = get_policy(addr, Some(&token("free"))).await;
    assert_eq!(status, 200);
    let json: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(json["local_allowed"], false);
    assert_eq!(json["watchdog"]["p95_ms"], 2000);

    let (_, body) = get_policy(addr, Some(&token("pro"))).await;
    let json: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(json["local_allowed"], true);
}
