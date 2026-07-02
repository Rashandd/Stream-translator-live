//! Billing/Stripe API uçtan-uca testleri — checkout başlatma + webhook.
//!
//! Gerçek Stripe key'i olmadan (billing::empty()) checkout her zaman
//! "yapılandırılmamış" (503) döner çünkü client_id yoktur; auth/plan
//! doğrulaması ise Stripe'a hiç gitmeden önce çalışır.

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

const SECRET: &[u8] = b"billing-api-test-secret";

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

fn token(plan: &str) -> String {
    let jwt = Jwt::new(SECRET);
    let claims = Claims {
        sub: "00000000-0000-0000-0000-000000000001".into(),
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

fn post_checkout(token: Option<&str>, json: &str) -> String {
    let auth = token
        .map(|t| format!("Authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    format!(
        "POST /api/billing/checkout HTTP/1.1\r\nHost: t\r\n{auth}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
        json.len()
    )
}

fn post_webhook(body: &str, signature: Option<&str>) -> String {
    let sig = signature
        .map(|s| format!("Stripe-Signature: {s}\r\n"))
        .unwrap_or_default();
    format!(
        "POST /webhooks/stripe HTTP/1.1\r\nHost: t\r\n{sig}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[tokio::test]
async fn checkout_requires_auth() {
    let addr = spawn_server().await;
    let (status, _) = http(addr, &post_checkout(None, r#"{"plan":"pro"}"#)).await;
    assert_eq!(status, 401);
}

#[tokio::test]
async fn checkout_rejects_plan_without_stripe_price() {
    let addr = spawn_server().await;
    // Faz 1 seed'inde tüm planların stripe_price_id'si boş (Stripe'ta ürün
    // henüz oluşturulmadı) -> 404.
    let (status, body) = http(addr, &post_checkout(Some(&token("pro")), r#"{"plan":"pro"}"#)).await;
    assert_eq!(status, 404, "body: {body}");
}

#[tokio::test]
async fn checkout_rejects_unknown_plan() {
    let addr = spawn_server().await;
    let (status, _) = http(addr, &post_checkout(Some(&token("pro")), r#"{"plan":"ghost"}"#)).await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn webhook_requires_signature() {
    let addr = spawn_server().await;
    let (status, _) = http(addr, &post_webhook("{}", None)).await;
    // billing::empty() -> webhook secret yok -> 503 (imza header'ı bile kontrol edilmeden).
    assert_eq!(status, 503);
}
