//! "Founding 5" kapalı beta başvuruları — uçtan uca testler: yayıncı
//! başvurur, admin listeler/onaylar/iptal eder, yetkisiz erişim reddedilir.

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
    users::UserStore,
};

const SECRET: &[u8] = b"founding-api-test-secret";

async fn spawn_server() -> (SocketAddr, Arc<UserStore>) {
    let service_config = Arc::new(ServiceConfig::with_defaults());
    let mut gateway = Gateway::new(Arc::clone(&service_config));
    gateway.register_stt(Arc::new(MockStt::new("deepgram")));
    gateway.register_translate(Arc::new(MockTranslate::new("deepl")));

    let state = AppState::skeleton(Jwt::new(SECRET), Arc::clone(&service_config), gateway);
    let users = Arc::clone(&state.users);

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        axum::serve(listener, app::router(state))
            .await
            .expect("serve");
    });
    (addr, users)
}

fn token_for(sub: &str, role: &str) -> String {
    let jwt = Jwt::new(SECRET);
    let claims = Claims {
        sub: sub.into(),
        role: role.into(),
        plan: "free".into(),
        exp: chrono::Utc::now().timestamp() + 3600,
    };
    jwt.issue(&claims).expect("issue token")
}

fn admin_token() -> String {
    token_for("00000000-0000-0000-0000-000000000099", "admin")
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
async fn submit_requires_auth() {
    let (addr, _users) = spawn_server().await;
    let (status, _) = http(
        addr,
        &request(
            "POST",
            "/api/founding/application",
            None,
            Some(r#"{"platform":"kick","channel_url":"https://kick.com/alice"}"#),
        ),
    )
    .await;
    assert_eq!(status, 401);
}

#[tokio::test]
async fn submit_then_read_own_application() {
    let (addr, users) = spawn_server().await;
    let alice = users
        .signup("alice@example.com", "alice-pass-1", "free")
        .expect("seed alice");
    let token = token_for(&alice.id, "broadcaster");

    let (status, body) = http(
        addr,
        &request(
            "POST",
            "/api/founding/application",
            Some(&token),
            Some(r#"{"platform":"kick","channel_url":"https://kick.com/alice","message":"hi"}"#),
        ),
    )
    .await;
    assert_eq!(status, 201, "body: {body}");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(parsed["status"], "pending");
    assert_eq!(parsed["email"], "alice@example.com");
    assert_eq!(parsed["platform"], "kick");

    let (status, body) = http(
        addr,
        &request("GET", "/api/founding/application", Some(&token), None),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(parsed["channel_url"], "https://kick.com/alice");
}

#[tokio::test]
async fn get_own_application_404_when_not_applied() {
    let (addr, users) = spawn_server().await;
    let bob = users
        .signup("bob@example.com", "bob-pass-1", "free")
        .expect("seed bob");
    let token = token_for(&bob.id, "broadcaster");

    let (status, _) = http(
        addr,
        &request("GET", "/api/founding/application", Some(&token), None),
    )
    .await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn empty_channel_url_is_rejected() {
    let (addr, users) = spawn_server().await;
    let alice = users
        .signup("alice2@example.com", "alice-pass-1", "free")
        .expect("seed alice");
    let token = token_for(&alice.id, "broadcaster");

    let (status, _) = http(
        addr,
        &request(
            "POST",
            "/api/founding/application",
            Some(&token),
            Some(r#"{"platform":"kick","channel_url":"   "}"#),
        ),
    )
    .await;
    assert_eq!(status, 422);
}

#[tokio::test]
async fn admin_lists_approves_and_revokes() {
    let (addr, users) = spawn_server().await;
    let alice = users
        .signup("alice3@example.com", "alice-pass-1", "free")
        .expect("seed alice");
    let user_token = token_for(&alice.id, "broadcaster");
    let admin = admin_token();

    http(
        addr,
        &request(
            "POST",
            "/api/founding/application",
            Some(&user_token),
            Some(r#"{"platform":"youtube","channel_url":"https://youtube.com/@alice"}"#),
        ),
    )
    .await;

    // Broadcaster admin route'una erişemez.
    let (status, _) = http(
        addr,
        &request(
            "GET",
            "/admin/founding/applications",
            Some(&user_token),
            None,
        ),
    )
    .await;
    assert_eq!(status, 403);

    let (status, body) = http(
        addr,
        &request("GET", "/admin/founding/applications", Some(&admin), None),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("json");
    let items = parsed.as_array().unwrap();
    assert!(items
        .iter()
        .any(|a| a["email"] == "alice3@example.com" && a["status"] == "pending"));

    let (status, body) = http(
        addr,
        &request(
            "POST",
            &format!("/admin/founding/{}/approve", alice.id),
            Some(&admin),
            None,
        ),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(parsed["status"], "approved");

    let (status, body) = http(
        addr,
        &request(
            "POST",
            &format!("/admin/founding/{}/revoke", alice.id),
            Some(&admin),
            None,
        ),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(parsed["status"], "revoked");
}

#[tokio::test]
async fn approve_unknown_user_is_404() {
    let (addr, _users) = spawn_server().await;
    let admin = admin_token();
    let (status, _) = http(
        addr,
        &request(
            "POST",
            "/admin/founding/no-such-user/approve",
            Some(&admin),
            None,
        ),
    )
    .await;
    assert_eq!(status, 404);
}
