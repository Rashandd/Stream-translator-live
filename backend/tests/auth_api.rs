//! Bootstrap admin login uçtan-uca testleri — `POST /api/auth/login`.
//!
//! Sözleşme: doğru kimlik → `role=admin` JWT (admin uçları açılır);
//! yanlış kimlik → 401 (alan söylenmez); env tanımsız → 503. Şifre asla
//! loglanmaz.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use streamdubber_backend::{
    app,
    auth::bootstrap::BootstrapAdmin,
    auth::jwt::Jwt,
    config::ServiceConfig,
    gateway::{stt::MockStt, translate::MockTranslate, Gateway},
    state::AppState,
};

const SECRET: &[u8] = b"auth-api-test-secret";

async fn spawn_server(admin: Option<BootstrapAdmin>) -> SocketAddr {
    let service_config = Arc::new(ServiceConfig::with_defaults());
    let mut gateway = Gateway::new(Arc::clone(&service_config));
    gateway.register_stt(Arc::new(MockStt::new("deepgram")));
    gateway.register_translate(Arc::new(MockTranslate::new("deepl")));

    let mut state = AppState::skeleton(Jwt::new(SECRET), Arc::clone(&service_config), gateway);
    state.bootstrap_admin = Arc::new(admin);
    let state = state;

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

fn post_login(json: &str) -> String {
    format!(
        "POST /api/auth/login HTTP/1.1\r\nHost: t\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
        json.len()
    )
}

fn get_with_token(path: &str, token: &str) -> String {
    format!(
        "GET {path} HTTP/1.1\r\nHost: t\r\nAuthorization: Bearer {token}\r\nConnection: close\r\n\r\n"
    )
}

#[tokio::test]
async fn login_issues_admin_token_that_opens_admin_endpoints() {
    let addr = spawn_server(Some(BootstrapAdmin::with_plain("ops@sd.io", "dev-pass"))).await;

    let (status, body) = http(
        addr,
        &post_login(r#"{"email":"ops@sd.io","password":"dev-pass"}"#),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");

    let parsed: serde_json::Value = serde_json::from_str(&body).expect("json");
    let token = parsed["token"].as_str().expect("token");
    assert!(parsed["expires_at"].as_i64().expect("expires_at") > chrono::Utc::now().timestamp());

    // Token claim'leri doğru ve admin ucu gerçekten açılıyor.
    let claims = Jwt::new(SECRET).verify(token).expect("valid token");
    assert_eq!(claims.role, "admin");
    assert_eq!(claims.sub, "admin-bootstrap");

    let (status, body) = http(addr, &get_with_token("/admin/ping", token)).await;
    assert_eq!(status, 200);
    assert_eq!(body, "admin:admin-bootstrap");
}

#[tokio::test]
async fn wrong_credentials_are_401_and_password_never_logged() {
    let addr = spawn_server(Some(BootstrapAdmin::with_plain("ops@sd.io", "correct-pass"))).await;

    let (status, _) = http(
        addr,
        &post_login(r#"{"email":"ops@sd.io","password":"wrong-pass-xyz"}"#),
    )
    .await;
    assert_eq!(status, 401);

    let (status, _) = http(
        addr,
        &post_login(r#"{"email":"other@sd.io","password":"correct-pass"}"#),
    )
    .await;
    assert_eq!(status, 401);

    // Audit izinde deneme var ama şifre yok (giriş için admin token gerekir —
    // doğru kimlikle alınır).
    let (_, body) = http(
        addr,
        &post_login(r#"{"email":"ops@sd.io","password":"correct-pass"}"#),
    )
    .await;
    let token = serde_json::from_str::<serde_json::Value>(&body).unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string();
    let (_, logs) = http(addr, &get_with_token("/admin/logs?service=auth", &token)).await;
    assert!(logs.contains("admin login failed"), "logs: {logs}");
    assert!(!logs.contains("wrong-pass-xyz"), "password leaked: {logs}");
    assert!(!logs.contains("correct-pass"), "password leaked: {logs}");
}

#[tokio::test]
async fn unconfigured_login_is_503() {
    let addr = spawn_server(None).await;
    let (status, _) = http(
        addr,
        &post_login(r#"{"email":"a@b.c","password":"x"}"#),
    )
    .await;
    assert_eq!(status, 503);
}
