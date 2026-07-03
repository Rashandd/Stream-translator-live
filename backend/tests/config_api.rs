//! Config API uçtan-uca testleri — admin yazar, yayıncı okur, yetkisiz reddedilir.
//!
//! `service_config` runtime store'unun HTTP üzerinden değiştirilebildiğini ve
//! değişikliğin (aynı süreç içinde) anında okunabildiğini doğrular.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use streamdubber_backend::{
    app,
    auth::jwt::{Claims, Jwt},
    billing::Billing,
    config::ServiceConfig,
    gateway::{stt::MockStt, translate::MockTranslate, Gateway},
    integrations::Integrations,
    overlay::OverlayHub,
    plans::PlanRegistry,
    state::AppState,
    usage::UsageTracker,
};

const SECRET: &[u8] = b"config-api-test-secret";

async fn spawn_server() -> SocketAddr {
    let service_config = Arc::new(ServiceConfig::with_defaults());
    let mut gateway = Gateway::new(Arc::clone(&service_config));
    gateway.register_stt(Arc::new(MockStt::new("deepgram")));
    gateway.register_translate(Arc::new(MockTranslate::new("deepl")));

    let state = AppState {
        jwt: Arc::new(Jwt::new(SECRET)),
        gateway: Arc::new(gateway),
        logger: Arc::new(streamdubber_backend::logging::Logger::in_memory(Arc::clone(&service_config))),
        service_config,
        integrations: Arc::new(Integrations::empty()),
        billing: Arc::new(Billing::empty()),
        overlay: Arc::new(OverlayHub::new()),
        plans: Arc::new(PlanRegistry::with_defaults()),
        usage: Arc::new(UsageTracker::new()),
        secrets: Arc::new(streamdubber_backend::secrets::ManagedSecrets::env()),
    };

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        axum::serve(listener, app::router(state)).await.expect("serve");
    });
    addr
}

fn token(role: &str) -> String {
    let jwt = Jwt::new(SECRET);
    let claims = Claims {
        sub: "00000000-0000-0000-0000-000000000001".into(),
        role: role.into(),
        plan: "pro".into(),
        exp: chrono::Utc::now().timestamp() + 3600,
    };
    jwt.issue(&claims).expect("issue token")
}

/// Tek bir HTTP isteği gönderir, (status, body) döndürür. `Connection: close`
/// sayesinde `read_to_end` yanıt bitince tamamlanır.
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

fn get(path: &str) -> String {
    format!("GET {path} HTTP/1.1\r\nHost: t\r\nConnection: close\r\n\r\n")
}

fn put_config(token: Option<&str>, json: &str) -> String {
    let auth = token
        .map(|t| format!("Authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    format!(
        "PUT /admin/config HTTP/1.1\r\nHost: t\r\n{auth}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
        json.len()
    )
}

#[tokio::test]
async fn public_get_returns_seeded_config() {
    let addr = spawn_server().await;
    let (status, body) = http(addr, &get("/config")).await;
    assert_eq!(status, 200);
    assert!(body.contains("\"stt.provider\":\"deepgram\""), "body: {body}");
    assert!(body.contains("\"dist.cloud.plans\":\"pro\""), "body: {body}");
}

#[tokio::test]
async fn admin_update_is_visible_to_readers() {
    let addr = spawn_server().await;

    let (status, body) = http(
        addr,
        &put_config(Some(&token("admin")), r#"{"key":"stt.provider","value":"whisper"}"#),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");

    // Aynı süreç içinde okuma anında yeni değeri görür.
    let (status, body) = http(addr, &get("/config")).await;
    assert_eq!(status, 200);
    assert!(body.contains("\"stt.provider\":\"whisper\""), "body: {body}");
}

#[tokio::test]
async fn write_requires_auth() {
    let addr = spawn_server().await;
    let (status, _) = http(addr, &put_config(None, r#"{"key":"stt.provider","value":"x"}"#)).await;
    assert_eq!(status, 401);
}

#[tokio::test]
async fn broadcaster_cannot_write() {
    let addr = spawn_server().await;
    let (status, _) = http(
        addr,
        &put_config(Some(&token("broadcaster")), r#"{"key":"stt.provider","value":"x"}"#),
    )
    .await;
    assert_eq!(status, 403);
}

#[tokio::test]
async fn unknown_key_is_rejected() {
    let addr = spawn_server().await;
    let (status, _) = http(
        addr,
        &put_config(Some(&token("admin")), r#"{"key":"attacker.key","value":"x"}"#),
    )
    .await;
    assert_eq!(status, 404);
}
