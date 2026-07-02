//! Plans API uçtan-uca testleri (Faz 1.11) — public liste + admin güncelleme.

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

const SECRET: &[u8] = b"plans-api-test-secret";

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
        sub: "00000000-0000-0000-0000-000000000002".into(),
        role: role.into(),
        plan: "free".into(),
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

fn get(path: &str) -> String {
    format!("GET {path} HTTP/1.1\r\nHost: t\r\nConnection: close\r\n\r\n")
}

fn put_plan(token: Option<&str>, json: &str) -> String {
    let auth = token
        .map(|t| format!("Authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    format!(
        "PUT /admin/plans HTTP/1.1\r\nHost: t\r\n{auth}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
        json.len()
    )
}

#[tokio::test]
async fn public_lists_four_seeded_plans() {
    let addr = spawn_server().await;
    let (status, body) = http(addr, &get("/api/plans")).await;
    assert_eq!(status, 200);
    assert!(body.contains("\"name\":\"free\""), "body: {body}");
    assert!(body.contains("\"name\":\"streamer\""), "body: {body}");
}

#[tokio::test]
async fn admin_can_update_a_plan() {
    let addr = spawn_server().await;
    let updated = r#"{"name":"free","price_cents":0,"monthly_hours":10,"max_languages":1,"max_platforms":1,"has_watermark":true,"has_voice_clone":false}"#;
    let (status, body) = http(addr, &put_plan(Some(&token("admin")), updated)).await;
    assert_eq!(status, 200, "body: {body}");
    assert!(body.contains("\"monthly_hours\":10"), "body: {body}");

    let (_, body) = http(addr, &get("/api/plans")).await;
    assert!(body.contains("\"monthly_hours\":10"), "body: {body}");
}

#[tokio::test]
async fn broadcaster_cannot_update_plan() {
    let addr = spawn_server().await;
    let updated = r#"{"name":"free","price_cents":0,"monthly_hours":10,"max_languages":1,"max_platforms":1,"has_watermark":true,"has_voice_clone":false}"#;
    let (status, _) = http(addr, &put_plan(Some(&token("broadcaster")), updated)).await;
    assert_eq!(status, 403);
}

#[tokio::test]
async fn update_requires_auth() {
    let addr = spawn_server().await;
    let updated = r#"{"name":"free","price_cents":0,"monthly_hours":10,"max_languages":1,"max_platforms":1,"has_watermark":true,"has_voice_clone":false}"#;
    let (status, _) = http(addr, &put_plan(None, updated)).await;
    assert_eq!(status, 401);
}
