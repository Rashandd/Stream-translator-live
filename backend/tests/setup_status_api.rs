//! Production Setup aggregator API uçtan-uca testleri (V11).
//!
//! Sözleşme: `GET /admin/setup/status` sadece admin'e açık; mevcut alt
//! sistemlerin (secrets/config/plans/provider testleri) durumunu tek bir
//! yanıtta toplar, yeni bir "gerçek" durum üretmez.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use streamdubber_backend::{
    app,
    auth::jwt::{Claims, Jwt},
    config::ServiceConfig,
    gateway::{stt::MockStt, translate::MockTranslate, Gateway},
    secrets::{ManagedSecrets, MemorySecretStore},
    state::AppState,
};

const SECRET: &[u8] = b"setup-status-api-test-secret";

async fn spawn_server() -> SocketAddr {
    let service_config = Arc::new(ServiceConfig::with_defaults());
    let mut gateway = Gateway::new(Arc::clone(&service_config));
    gateway.register_stt(Arc::new(MockStt::new("deepgram")));
    gateway.register_translate(Arc::new(MockTranslate::new("deepl")));

    let mut state = AppState::skeleton(Jwt::new(SECRET), Arc::clone(&service_config), gateway);
    state.secrets = Arc::new(ManagedSecrets::new(Box::new(MemorySecretStore::new()), "env"));
    let state = state;

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
        sub: "00000000-0000-0000-0000-000000000042".into(),
        role: role.into(),
        plan: "streamer".into(),
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

fn request(method: &str, path: &str, token: Option<&str>, json: Option<&str>) -> String {
    let auth = token
        .map(|t| format!("Authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    match json {
        Some(body) => format!(
            "{method} {path} HTTP/1.1\r\nHost: t\r\n{auth}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ),
        None => format!("{method} {path} HTTP/1.1\r\nHost: t\r\n{auth}Connection: close\r\n\r\n"),
    }
}

#[tokio::test]
async fn requires_admin() {
    let addr = spawn_server().await;

    let (status, _) = http(addr, &request("GET", "/admin/setup/status", None, None)).await;
    assert_eq!(status, 401);

    let (status, _) = http(
        addr,
        &request("GET", "/admin/setup/status", Some(&token("broadcaster")), None),
    )
    .await;
    assert_eq!(status, 403);
}

#[tokio::test]
async fn reports_expected_checks_and_providers() {
    let addr = spawn_server().await;
    let (status, body) = http(addr, &request("GET", "/admin/setup/status", Some(&token("admin")), None)).await;
    assert_eq!(status, 200, "body: {body}");

    let parsed: serde_json::Value = serde_json::from_str(&body).expect("json");
    let checks = parsed["checks"].as_array().expect("checks array");
    let ids: Vec<&str> = checks.iter().map(|c| c["id"].as_str().unwrap()).collect();
    for expected in [
        "admin_login",
        "secrets",
        "mock_fallback",
        "tls",
        "redirect_base",
        "stripe_prices",
        "logs",
        "persistence",
    ] {
        assert!(ids.contains(&expected), "missing check {expected}: {ids:?}");
    }

    // Bootstrap admin skeleton'da yok → bu kontrol "bad" olmalı.
    let admin_login = checks.iter().find(|c| c["id"] == "admin_login").unwrap();
    assert_eq!(admin_login["tone"], "bad");

    // Test ortamında TLS_CERT_PATH/TLS_KEY_PATH yok → "bad" olmalı.
    let tls = checks.iter().find(|c| c["id"] == "tls").unwrap();
    assert_eq!(tls["tone"], "bad");

    let providers = parsed["providers"].as_array().expect("providers array");
    let names: Vec<&str> = providers.iter().map(|p| p["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"deepgram"));
    assert!(names.contains(&"stripe"));
    assert!(names.contains(&"youtube"));
    assert!(names.contains(&"streamlabs"));

    let youtube = providers.iter().find(|p| p["name"] == "youtube").unwrap();
    assert_eq!(youtube["kind"], "oauth");
    assert_eq!(youtube["connect_path"], "/admin/integrations/youtube/authorize");
    assert_eq!(youtube["configured"], false);

    let deepgram = providers.iter().find(|p| p["name"] == "deepgram").unwrap();
    assert_eq!(deepgram["kind"], "api_key");
    assert!(deepgram["connect_path"].is_null());
    assert!(deepgram["last_tested_at_ms"].is_null());
}

#[tokio::test]
async fn provider_test_result_shows_up_in_setup_status() {
    let addr = spawn_server().await;
    let admin = token("admin");

    // Key'siz test → 503, henüz kaydedilmez (run_check ağa çıkmadan reddeder).
    let (status, _) = http(
        addr,
        &request("POST", "/admin/providers/deepgram/test", Some(&admin), None),
    )
    .await;
    assert_eq!(status, 503);

    let (_, body) = http(addr, &request("GET", "/admin/setup/status", Some(&admin), None)).await;
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("json");
    let deepgram = parsed["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "deepgram")
        .unwrap();
    assert!(deepgram["last_tested_at_ms"].is_null(), "not-configured test should not record");
}
