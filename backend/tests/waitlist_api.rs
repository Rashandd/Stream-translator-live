//! Waitlist lead-capture uçtan-uca testleri (bkz. `docs/overlay-builder-plan.md` §6).
//!
//! `AppState::skeleton` ile `db: None` olduğu için başarı yolu (201) DB'siz
//! test edilir — gerçek Postgres'e karşı duplicate/unique-constraint testi
//! `TEST_DATABASE_URL` ile ayrı, `#[ignore]`'lu bir testte (bkz.
//! `billing::persist::stripe_persist_roundtrip_against_real_db` ile aynı desen).

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use streamdubber_backend::{
    app, auth::jwt::Jwt, config::ServiceConfig,
    gateway::{stt::MockStt, translate::MockTranslate, Gateway},
    state::AppState,
};

async fn spawn_server() -> SocketAddr {
    let service_config = Arc::new(ServiceConfig::with_defaults());
    let mut gateway = Gateway::new(Arc::clone(&service_config));
    gateway.register_stt(Arc::new(MockStt::new("deepgram")));
    gateway.register_translate(Arc::new(MockTranslate::new("deepl")));

    let state = AppState::skeleton(
        Jwt::new(b"waitlist-api-test-secret"),
        Arc::clone(&service_config),
        gateway,
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        axum::serve(listener, app::router(state)).await.expect("serve");
    });
    addr
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

fn post_json(path: &str, body: &str) -> String {
    format!(
        "POST {path} HTTP/1.1\r\nHost: t\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[tokio::test]
async fn valid_signup_is_accepted_without_a_database() {
    let addr = spawn_server().await;
    let (status, body) = http(
        addr,
        &post_json(
            "/api/waitlist",
            r#"{"email":"person@example.com","interest":"watch-party"}"#,
        ),
    )
    .await;
    assert_eq!(status, 201, "body: {body}");
    assert!(body.contains("\"ok\""));
}

#[tokio::test]
async fn malformed_email_is_rejected() {
    let addr = spawn_server().await;
    let (status, _) = http(
        addr,
        &post_json(
            "/api/waitlist",
            r#"{"email":"not-an-email","interest":"watch-party"}"#,
        ),
    )
    .await;
    assert_eq!(status, 400);
}

#[tokio::test]
async fn empty_interest_is_rejected() {
    let addr = spawn_server().await;
    let (status, _) = http(
        addr,
        &post_json("/api/waitlist", r#"{"email":"person@example.com","interest":""}"#),
    )
    .await;
    assert_eq!(status, 400);
}

#[tokio::test]
async fn repeated_requests_are_rate_limited() {
    let addr = spawn_server().await;
    let mut last_status = 0;
    for i in 0..10 {
        let body = format!(r#"{{"email":"person{i}@example.com","interest":"watch-party"}}"#);
        let (status, _) = http(addr, &post_json("/api/waitlist", &body)).await;
        last_status = status;
    }
    assert_eq!(last_status, 429);
}
