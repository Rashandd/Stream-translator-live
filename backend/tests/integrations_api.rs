//! OAuth entegrasyon uçlarının admin/broadcaster ayrımı (V11).
//!
//! Sözleşme: broadcaster akışı (`GET /api/integrations/{provider}/authorize`)
//! plan erişimini uygular; admin doğrulama akışı
//! (`GET /admin/integrations/{provider}/authorize`) bunu atlar — operatör
//! kendi bootstrap hesabıyla (plan="streamer") bağlantıyı test edebilmeli.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use streamdubber_backend::{
    app,
    auth::jwt::{Claims, Jwt},
    config::ServiceConfig,
    gateway::{stt::MockStt, translate::MockTranslate, Gateway},
    integrations::{token_store::TokenVault, Integrations},
    secrets::{names, MemorySecretStore, SecretStore},
    state::AppState,
};

const SECRET: &[u8] = b"integrations-api-test-secret";

async fn spawn_server() -> SocketAddr {
    let service_config = Arc::new(ServiceConfig::with_defaults());
    let mut gateway = Gateway::new(Arc::clone(&service_config));
    gateway.register_stt(Arc::new(MockStt::new("deepgram")));
    gateway.register_translate(Arc::new(MockTranslate::new("deepl")));

    // streamlabs client id/secret yapılandırılmış → begin() adımı authorize
    // URL üretebilir (token exchange'e kadar gitmiyoruz).
    let store = MemorySecretStore::with([
        (names::STREAMLABS_CLIENT_ID, "cid-live"),
        (names::STREAMLABS_CLIENT_SECRET, "sec-live"),
    ]);
    let integrations = Arc::new(Integrations::from_secrets(
        Arc::new(store) as Arc<dyn SecretStore>,
        Arc::new(TokenVault::in_memory()),
    ));

    let mut state = AppState::skeleton(Jwt::new(SECRET), Arc::clone(&service_config), gateway);
    state.integrations = integrations;
    let state = state;

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        axum::serve(listener, app::router(state)).await.expect("serve");
    });
    addr
}

fn token(role: &str, plan: &str) -> String {
    let jwt = Jwt::new(SECRET);
    let claims = Claims {
        sub: "00000000-0000-0000-0000-000000000042".into(),
        role: role.into(),
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

fn request(method: &str, path: &str, token: Option<&str>) -> String {
    let auth = token
        .map(|t| format!("Authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    format!("{method} {path} HTTP/1.1\r\nHost: t\r\n{auth}Connection: close\r\n\r\n")
}

#[tokio::test]
async fn broadcaster_authorize_respects_plan_gate() {
    let addr = spawn_server().await;
    // integration.streamlabs.plans = "pro" (seed varsayılanı) — "starter" reddedilir.
    let (status, body) = http(
        addr,
        &request(
            "GET",
            "/api/integrations/streamlabs/authorize",
            Some(&token("broadcaster", "starter")),
        ),
    )
    .await;
    assert_eq!(status, 403, "body: {body}");
}

#[tokio::test]
async fn broadcaster_authorize_succeeds_within_plan() {
    let addr = spawn_server().await;
    let (status, body) = http(
        addr,
        &request(
            "GET",
            "/api/integrations/streamlabs/authorize",
            Some(&token("broadcaster", "pro")),
        ),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    assert!(body.contains("authorize_url"));
}

#[tokio::test]
async fn admin_authorize_bypasses_plan_gate() {
    let addr = spawn_server().await;
    // Bootstrap admin claim'i plan="streamer" — streamlabs "pro" listesinde
    // değil, ama admin yolu bunu atlamalı.
    let (status, body) = http(
        addr,
        &request(
            "GET",
            "/admin/integrations/streamlabs/authorize",
            Some(&token("admin", "streamer")),
        ),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    assert!(body.contains("authorize_url"));
}

#[tokio::test]
async fn admin_authorize_requires_admin_role() {
    let addr = spawn_server().await;
    let (status, _) = http(addr, &request("GET", "/admin/integrations/streamlabs/authorize", None)).await;
    assert_eq!(status, 401);

    let (status, _) = http(
        addr,
        &request(
            "GET",
            "/admin/integrations/streamlabs/authorize",
            Some(&token("broadcaster", "streamer")),
        ),
    )
    .await;
    assert_eq!(status, 403);
}
