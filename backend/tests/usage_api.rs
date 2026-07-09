//! `GET /api/usage/summary` uçtan-uca testleri — dashboard billing sayfasının
//! plan + bu ayki kullanım kartlarını doldurduğu uç.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use streamdubber_backend::{
    app,
    auth::jwt::{Claims, Jwt},
    config::ServiceConfig,
    gateway::{stt::MockStt, translate::MockTranslate, Gateway},
    plans::PlanDefinition,
    state::AppState,
};

const SECRET: &[u8] = b"usage-api-test-secret";

async fn spawn_server() -> (SocketAddr, AppState) {
    let service_config = Arc::new(ServiceConfig::with_defaults());
    let mut gateway = Gateway::new(Arc::clone(&service_config));
    gateway.register_stt(Arc::new(MockStt::new("deepgram")));
    gateway.register_translate(Arc::new(MockTranslate::new("deepl")));

    let state = AppState::skeleton(Jwt::new(SECRET), Arc::clone(&service_config), gateway);
    let state_clone = state.clone();

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        axum::serve(listener, app::router(state)).await.expect("serve");
    });
    (addr, state_clone)
}

fn token(sub: &str, plan: &str) -> String {
    let jwt = Jwt::new(SECRET);
    let claims = Claims {
        sub: sub.into(),
        role: "broadcaster".into(),
        plan: plan.into(),
        exp: chrono::Utc::now().timestamp() + 3600,
    };
    jwt.issue(&claims).expect("issue token")
}

async fn http_get_with_token(addr: SocketAddr, path: &str, token: &str) -> (u16, String) {
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: t\r\nAuthorization: Bearer {token}\r\nConnection: close\r\n\r\n"
    );
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

#[tokio::test]
async fn reports_zero_usage_and_the_plan_catalog_limit_for_a_fresh_user() {
    let (addr, _state) = spawn_server().await;
    let (status, body) = http_get_with_token(addr, "/api/usage/summary", &token("u1", "free")).await;
    assert_eq!(status, 200, "body: {body}");

    let parsed: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(parsed["plan"], "free");
    assert_eq!(parsed["price_cents"], 0);
    assert_eq!(parsed["used_seconds"], 0);
    assert_eq!(parsed["monthly_hours"], 5);
    assert_eq!(parsed["remaining_seconds"], 5 * 3600);
    assert_eq!(parsed["limit_reached"], false);
    assert!(parsed["period"].as_str().expect("period").starts_with("20"));
}

#[tokio::test]
async fn reflects_recorded_usage_for_the_authenticated_user_only() {
    let (addr, state) = spawn_server().await;
    state.usage.record("u1", 120.0, 5);
    // Farklı kullanıcının kullanımı u1'i etkilemez.
    state.usage.record("u2", 999.0, 5);

    let (status, body) = http_get_with_token(addr, "/api/usage/summary", &token("u1", "free")).await;
    assert_eq!(status, 200, "body: {body}");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(parsed["used_seconds"], 120);
    assert_eq!(parsed["remaining_seconds"], 5 * 3600 - 120);
}

#[tokio::test]
async fn unlimited_plan_reports_negative_one_and_never_reaches_the_limit() {
    let (addr, state) = spawn_server().await;
    // Sınırsız bir plan senaryosu (admin panelden `monthly_hours=-1` girilmiş).
    state.plans.set(PlanDefinition {
        name: "enterprise".into(),
        price_cents: 0,
        monthly_hours: -1,
        max_languages: -1,
        max_platforms: -1,
        has_watermark: false,
        has_voice_clone: true,
    });
    state.usage.record("u1", 10_000.0, -1);

    let (status, body) = http_get_with_token(addr, "/api/usage/summary", &token("u1", "enterprise")).await;
    assert_eq!(status, 200, "body: {body}");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(parsed["plan"], "enterprise");
    assert_eq!(parsed["monthly_hours"], -1);
    assert_eq!(parsed["remaining_seconds"], -1);
    assert_eq!(parsed["limit_reached"], false);
}

#[tokio::test]
async fn requires_auth() {
    let (addr, _state) = spawn_server().await;
    let (status, _) = http_get_with_token(addr, "/api/usage/summary", "not-a-real-token").await;
    assert_eq!(status, 401);
}
