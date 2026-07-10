//! OAuth entegrasyon uçlarının admin/broadcaster ayrımı (V11).
//!
//! Sözleşme: broadcaster akışı (`GET /api/integrations/{provider}/authorize`)
//! OAuth sağlayıcı yapılandırılmışsa plan ayrımı yapmadan authorize URL üretir.
//! Admin doğrulama akışı aynı şekilde operatörün kendi hesabıyla bağlantıyı
//! test edebilmesini sağlar.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use streamdubber_backend::{
    app,
    auth::jwt::{Claims, Jwt},
    config::ServiceConfig,
    gateway::{stt::MockStt, translate::MockTranslate, Gateway},
    integrations::{oauth::TokenResponse, token_store::TokenVault, Integrations},
    secrets::{names, MemorySecretStore, SecretStore},
    state::AppState,
};

const SECRET: &[u8] = b"integrations-api-test-secret";

async fn spawn_server() -> SocketAddr {
    spawn_server_with(None, false).await
}

async fn spawn_server_with(
    streamlabs_test_url: Option<String>,
    preload_streamlabs_token: bool,
) -> SocketAddr {
    let service_config = Arc::new(ServiceConfig::with_defaults());
    if let Some(url) = streamlabs_test_url {
        service_config.set("providers.streamlabs.test_url", url);
    }
    let mut gateway = Gateway::new(Arc::clone(&service_config));
    gateway.register_stt(Arc::new(MockStt::new("deepgram")));
    gateway.register_translate(Arc::new(MockTranslate::new("deepl")));

    // streamlabs client id/secret yapılandırılmış → begin() adımı authorize
    // URL üretebilir (token exchange'e kadar gitmiyoruz).
    let store = MemorySecretStore::with([
        (names::STREAMLABS_CLIENT_ID, "cid-live"),
        (names::STREAMLABS_CLIENT_SECRET, "sec-live"),
    ]);
    let token_vault = Arc::new(TokenVault::in_memory());
    if preload_streamlabs_token {
        token_vault
            .store(
                "00000000-0000-0000-0000-000000000042",
                "streamlabs",
                &TokenResponse {
                    access_token: "at-live".into(),
                    refresh_token: Some("rt-live".into()),
                    expires_in: Some(3600),
                    token_type: Some("Bearer".into()),
                },
            )
            .expect("preload streamlabs token");
    }
    let integrations = Arc::new(Integrations::from_secrets(
        Arc::new(store) as Arc<dyn SecretStore>,
        token_vault,
    ));

    let mut state = AppState::skeleton(Jwt::new(SECRET), Arc::clone(&service_config), gateway);
    state.integrations = integrations;
    let state = state;

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        axum::serve(listener, app::router(state))
            .await
            .expect("serve");
    });
    addr
}

async fn spawn_streamlabs_check_mock() -> String {
    use axum::extract::Query;
    use axum::http::StatusCode;
    use axum::routing::get;
    use axum::{Json, Router};
    use std::collections::HashMap;

    async fn socket_token(
        Query(q): Query<HashMap<String, String>>,
    ) -> (StatusCode, Json<serde_json::Value>) {
        if q.get("access_token").map(String::as_str) == Some("at-live") {
            (
                StatusCode::OK,
                Json(serde_json::json!({ "socket_token": "socket-ok" })),
            )
        } else {
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "bad token" })),
            )
        }
    }

    let app = Router::new().route("/socket/token", get(socket_token));
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });
    format!("http://{addr}/socket/token")
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
async fn broadcaster_authorize_ignores_plan_gate_when_oauth_is_configured() {
    let addr = spawn_server().await;
    let (status, body) = http(
        addr,
        &request(
            "GET",
            "/api/integrations/streamlabs/authorize",
            Some(&token("broadcaster", "starter")),
        ),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    assert!(body.contains("authorize_url"));
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
async fn admin_authorize_can_verify_configured_oauth() {
    let addr = spawn_server().await;
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
    let (status, _) = http(
        addr,
        &request("GET", "/admin/integrations/streamlabs/authorize", None),
    )
    .await;
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

#[tokio::test]
async fn broadcaster_can_test_connected_streamlabs() {
    let test_url = spawn_streamlabs_check_mock().await;
    let addr = spawn_server_with(Some(test_url), true).await;
    let (status, body) = http(
        addr,
        &request(
            "POST",
            "/api/integrations/streamlabs/test",
            Some(&token("broadcaster", "pro")),
        ),
    )
    .await;

    assert_eq!(status, 200, "body: {body}");
    assert!(body.contains(r#""provider":"streamlabs""#), "body: {body}");
    assert!(body.contains(r#""ok":true"#), "body: {body}");
}

#[tokio::test]
async fn broadcaster_test_reports_not_connected() {
    let addr = spawn_server().await;
    let (status, body) = http(
        addr,
        &request(
            "POST",
            "/api/integrations/streamlabs/test",
            Some(&token("broadcaster", "pro")),
        ),
    )
    .await;

    assert_eq!(status, 409, "body: {body}");
    assert!(body.contains("not connected"), "body: {body}");
}
