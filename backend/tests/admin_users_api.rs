//! Admin kullanıcı yönetimi (Faz 1.14) uçtan-uca testleri — liste/arama,
//! detay, plan değiştirme. Parola hash'i hiçbir yanıtta dönmemeli.

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

const SECRET: &[u8] = b"admin-users-api-test-secret";

async fn spawn_server() -> (SocketAddr, Arc<UserStore>) {
    let service_config = Arc::new(ServiceConfig::with_defaults());
    let mut gateway = Gateway::new(Arc::clone(&service_config));
    gateway.register_stt(Arc::new(MockStt::new("deepgram")));
    gateway.register_translate(Arc::new(MockTranslate::new("deepl")));

    let state = AppState::skeleton(Jwt::new(SECRET), Arc::clone(&service_config), gateway);
    let users = Arc::clone(&state.users);
    users.signup("alice@example.com", "alice-pass-1", "free").expect("seed alice");
    users.signup("bob@example.com", "bob-pass-1", "starter").expect("seed bob");

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        axum::serve(listener, app::router(state)).await.expect("serve");
    });
    (addr, users)
}

fn token(role: &str) -> String {
    let jwt = Jwt::new(SECRET);
    let claims = Claims {
        sub: "00000000-0000-0000-0000-000000000099".into(),
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
async fn list_requires_admin() {
    let (addr, _users) = spawn_server().await;
    let (status, _) = http(addr, &request("GET", "/admin/users", None, None)).await;
    assert_eq!(status, 401);

    let (status, _) = http(
        addr,
        &request("GET", "/admin/users", Some(&token("broadcaster")), None),
    )
    .await;
    assert_eq!(status, 403);
}

#[tokio::test]
async fn list_returns_seeded_users_without_password_hash() {
    let (addr, _users) = spawn_server().await;
    let (status, body) = http(addr, &request("GET", "/admin/users", Some(&token("admin")), None)).await;
    assert_eq!(status, 200, "body: {body}");

    let parsed: serde_json::Value = serde_json::from_str(&body).expect("json");
    let emails: Vec<&str> = parsed.as_array().unwrap().iter().map(|u| u["email"].as_str().unwrap()).collect();
    assert!(emails.contains(&"alice@example.com"));
    assert!(emails.contains(&"bob@example.com"));
    assert!(!body.contains("password_hash"), "password hash must never leak: {body}");
}

#[tokio::test]
async fn list_search_filters_by_email_substring() {
    let (addr, _users) = spawn_server().await;
    let (status, body) = http(
        addr,
        &request("GET", "/admin/users?q=alice", Some(&token("admin")), None),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("json");
    let items = parsed.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["email"], "alice@example.com");
}

#[tokio::test]
async fn get_by_id_returns_detail_and_unknown_id_is_404() {
    let (addr, users) = spawn_server().await;
    let alice = users.get("alice@example.com").expect("alice seeded");

    let (status, body) = http(
        addr,
        &request("GET", &format!("/admin/users/{}", alice.id), Some(&token("admin")), None),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(parsed["email"], "alice@example.com");
    assert_eq!(parsed["plan"], "free");

    let (status, _) = http(
        addr,
        &request("GET", "/admin/users/no-such-id", Some(&token("admin")), None),
    )
    .await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn admin_can_change_a_users_plan() {
    let (addr, users) = spawn_server().await;
    let alice = users.get("alice@example.com").expect("alice seeded");
    assert_eq!(alice.plan, "free");

    let (status, body) = http(
        addr,
        &request(
            "PUT",
            &format!("/admin/users/{}/plan", alice.id),
            Some(&token("admin")),
            Some(r#"{"plan":"pro"}"#),
        ),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(parsed["plan"], "pro");

    // Değişiklik kalıcı — store'dan tekrar okununca da yeni plan görünür.
    assert_eq!(users.get("alice@example.com").unwrap().plan, "pro");
}

#[tokio::test]
async fn setting_an_unknown_plan_is_rejected() {
    let (addr, users) = spawn_server().await;
    let alice = users.get("alice@example.com").expect("alice seeded");

    let (status, _) = http(
        addr,
        &request(
            "PUT",
            &format!("/admin/users/{}/plan", alice.id),
            Some(&token("admin")),
            Some(r#"{"plan":"nonexistent-plan"}"#),
        ),
    )
    .await;
    assert_eq!(status, 404);
    // Reddedilen istek planı değiştirmemiş olmalı.
    assert_eq!(users.get("alice@example.com").unwrap().plan, "free");
}

#[tokio::test]
async fn broadcaster_cannot_change_plans() {
    let (addr, users) = spawn_server().await;
    let alice = users.get("alice@example.com").expect("alice seeded");

    let (status, _) = http(
        addr,
        &request(
            "PUT",
            &format!("/admin/users/{}/plan", alice.id),
            Some(&token("broadcaster")),
            Some(r#"{"plan":"pro"}"#),
        ),
    )
    .await;
    assert_eq!(status, 403);
}
