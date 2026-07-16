//! Bootstrap admin login uçtan-uca testleri — `POST /api/auth/login`.
//!
//! Sözleşme: doğru kimlik → `role=admin` JWT (admin uçları açılır);
//! yanlış kimlik → 401 (alan söylenmez). 1.4 kullanıcı sistemiyle uç
//! artık yayıncı deposuna da bakar: bootstrap env tanımsızken yanıt
//! 503 değil 401'dir (hesap varlığı sızdırılmaz). Şifre asla loglanmaz.
//! Yayıncı akışları `tests/users_api.rs`'te.

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

const TEST_ORIGIN: &str = "http://localhost:3000";

/// A same-site browser fetch: carries `Origin`, which is what flips
/// `finish_principal_login` from the legacy Bearer-JSON response to the
/// HttpOnly cookie session (`auth::session::create_browser_session`).
fn post_login_browser(json: &str) -> String {
    format!(
        "POST /api/auth/login HTTP/1.1\r\nHost: t\r\nOrigin: {TEST_ORIGIN}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
        json.len()
    )
}

fn get_with_cookie(path: &str, cookie: &str) -> String {
    format!("GET {path} HTTP/1.1\r\nHost: t\r\nCookie: {cookie}\r\nConnection: close\r\n\r\n")
}

fn get_with_cookie_and_origin(path: &str, cookie: &str, origin: &str) -> String {
    format!(
        "GET {path} HTTP/1.1\r\nHost: t\r\nOrigin: {origin}\r\nCookie: {cookie}\r\nConnection: close\r\n\r\n"
    )
}

fn put_with_cookie(path: &str, cookie: &str, csrf: Option<&str>, json: &str) -> String {
    let csrf_header = csrf
        .map(|value| format!("x-csrf-token: {value}\r\n"))
        .unwrap_or_default();
    format!(
        "PUT {path} HTTP/1.1\r\nHost: t\r\nOrigin: {TEST_ORIGIN}\r\nCookie: {cookie}\r\n{csrf_header}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
        json.len()
    )
}

/// Extracts every `Set-Cookie` header's `name=value` pair (dropping
/// attributes like `Path`/`HttpOnly`) from a raw HTTP/1.1 response.
fn set_cookies(raw_response: &str) -> std::collections::HashMap<String, String> {
    raw_response
        .lines()
        .filter_map(|line| line.strip_prefix("set-cookie: ").or_else(|| line.strip_prefix("Set-Cookie: ")))
        .filter_map(|value| value.split(';').next())
        .filter_map(|pair| pair.split_once('='))
        .map(|(name, value)| (name.trim().to_string(), value.trim().to_string()))
        .collect()
}

async fn http_raw(addr: SocketAddr, request: &str) -> (u16, String, String) {
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
    let split = text.split("\r\n\r\n").collect::<Vec<_>>();
    let headers = split.first().copied().unwrap_or("").to_string();
    let body = split.get(1).copied().unwrap_or("").to_string();
    (status, headers, body)
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
    // Birleşik uç (1.4) mesajı "login failed" — admin/yayıncı ayrımı yapılmaz.
    assert!(logs.contains("login failed"), "logs: {logs}");
    assert!(!logs.contains("wrong-pass-xyz"), "password leaked: {logs}");
    assert!(!logs.contains("correct-pass"), "password leaked: {logs}");
}

#[tokio::test]
async fn unconfigured_bootstrap_falls_through_to_401() {
    // 1.4 öncesi bu durum 503'tü ("admin login not configured"). Artık uç
    // yayıncı deposuna da baktığı için bilinmeyen kimlik her koşulda 401 —
    // hesap/konfigürasyon varlığı dışarı sızmaz.
    let addr = spawn_server(None).await;
    let (status, _) = http(
        addr,
        &post_login(r#"{"email":"a@b.c","password":"x"}"#),
    )
    .await;
    assert_eq!(status, 401);
}

/// Regression for the browser-session migration (c76149d): a browser login
/// (Origin header present) must set the HttpOnly cookie trio and must NOT
/// leak a bearer token in the body — the admin frontend used to blindly
/// store `data.token` (`undefined`) from this exact response, sending
/// `Authorization: Bearer undefined` on the very next request and showing
/// "session expired" seconds after a successful login.
#[tokio::test]
async fn browser_login_sets_cookies_and_omits_bearer_token() {
    let addr = spawn_server(Some(BootstrapAdmin::with_plain("ops@sd.io", "dev-pass"))).await;

    let (status, headers, body) = http_raw(
        addr,
        &post_login_browser(r#"{"email":"ops@sd.io","password":"dev-pass"}"#),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");

    let json: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert!(json.get("token").is_none(), "browser response must not carry a bearer token");
    assert_eq!(json["authenticated"], true);
    assert_eq!(json["role"], "admin");
    assert!(json["csrf_token"].as_str().is_some_and(|v| !v.is_empty()));

    let cookies = set_cookies(&headers);
    assert!(cookies.contains_key("sd_access"));
    assert!(cookies.contains_key("sd_refresh"));
    assert!(cookies.contains_key("sd_csrf"));
    assert_eq!(cookies["sd_csrf"], json["csrf_token"].as_str().unwrap());
}

/// The cookie set by a browser login must actually authenticate
/// `GET /api/auth/session` — this is the route `AuthGuard` polls instead of
/// trusting a client-side token that may not exist.
#[tokio::test]
async fn cookie_from_browser_login_authenticates_session_endpoint() {
    let addr = spawn_server(Some(BootstrapAdmin::with_plain("ops@sd.io", "dev-pass"))).await;
    let (_, headers, _) = http_raw(
        addr,
        &post_login_browser(r#"{"email":"ops@sd.io","password":"dev-pass"}"#),
    )
    .await;
    let cookies = set_cookies(&headers);
    let cookie_header = format!("sd_access={}", cookies["sd_access"]);

    let (status, _, body) = http_raw(addr, &get_with_cookie("/api/auth/session", &cookie_header)).await;
    assert_eq!(status, 200, "body: {body}");
    let json: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(json["authenticated"], true);
    assert_eq!(json["role"], "admin");
}

/// `GET /api/auth/session` without any cookie must not authenticate —
/// AuthGuard relies on this to redirect unauthenticated visitors to login.
#[tokio::test]
async fn session_endpoint_without_cookie_is_401() {
    let addr = spawn_server(Some(BootstrapAdmin::with_plain("ops@sd.io", "dev-pass"))).await;
    let (status, _, _) = http_raw(
        addr,
        "GET /api/auth/session HTTP/1.1\r\nHost: t\r\nConnection: close\r\n\r\n",
    )
    .await;
    assert_eq!(status, 401);
}

/// `dev.strovak.com` cannot read a host-only cookie owned by
/// `dev.api.strovak.com`. The browser therefore needs a credentialed safe
/// request that reflects the non-secret double-submit value without first
/// validating the short-lived access token. The configured dashboard CORS
/// layer must protect that response and intermediaries must never cache it.
#[tokio::test]
async fn csrf_bootstrap_returns_cookie_without_a_live_access_token_and_is_not_cacheable() {
    let addr = spawn_server(Some(BootstrapAdmin::with_plain("ops@sd.io", "dev-pass"))).await;
    let (_, login_headers, _) = http_raw(
        addr,
        &post_login_browser(r#"{"email":"ops@sd.io","password":"dev-pass"}"#),
    )
    .await;
    let cookies = set_cookies(&login_headers);
    let csrf = cookies.get("sd_csrf").expect("csrf cookie");

    // Deliberately omit sd_access: bootstrap must still work after its
    // 15-minute JWT expires, otherwise the client cannot authorize refresh.
    let (status, response_headers, body) = http_raw(
        addr,
        &get_with_cookie_and_origin(
            "/api/auth/csrf",
            &format!("sd_csrf={csrf}"),
            TEST_ORIGIN,
        ),
    )
    .await;

    assert_eq!(status, 200, "body: {body}");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body).expect("json")["csrf_token"],
        csrf.as_str()
    );
    let response_headers = response_headers.to_ascii_lowercase();
    assert!(
        response_headers.contains("cache-control: no-store"),
        "headers: {response_headers}"
    );
    assert!(
        response_headers.contains(&format!("access-control-allow-origin: {TEST_ORIGIN}")),
        "headers: {response_headers}"
    );
    assert!(
        response_headers.contains("access-control-allow-credentials: true"),
        "headers: {response_headers}"
    );
}

#[tokio::test]
async fn csrf_bootstrap_without_the_double_submit_cookie_is_401_and_not_cacheable() {
    let addr = spawn_server(Some(BootstrapAdmin::with_plain("ops@sd.io", "dev-pass"))).await;
    let (status, response_headers, _) = http_raw(
        addr,
        &get_with_cookie_and_origin("/api/auth/csrf", "unrelated=value", TEST_ORIGIN),
    )
    .await;

    assert_eq!(status, 401);
    assert!(
        response_headers
            .to_ascii_lowercase()
            .contains("cache-control: no-store"),
        "headers: {response_headers}"
    );
}

/// Cookie-authenticated mutations must carry a matching double-submit CSRF
/// header (`require_cookie_request`); the cookie alone is not enough,
/// otherwise any cross-site page could ride the admin's cookies.
#[tokio::test]
async fn cookie_mutation_without_csrf_header_is_forbidden_but_with_it_succeeds() {
    let addr = spawn_server(Some(BootstrapAdmin::with_plain("ops@sd.io", "dev-pass"))).await;
    let (_, headers, _) = http_raw(
        addr,
        &post_login_browser(r#"{"email":"ops@sd.io","password":"dev-pass"}"#),
    )
    .await;
    let cookies = set_cookies(&headers);
    let cookie_header = format!("sd_access={}; sd_csrf={}", cookies["sd_access"], cookies["sd_csrf"]);
    let body = r#"{"key":"log.level","value":"debug"}"#;

    let (status, _, resp_body) = http_raw(addr, &put_with_cookie("/admin/config", &cookie_header, None, body)).await;
    assert_eq!(status, 403, "body: {resp_body}");

    let (status, _, resp_body) = http_raw(
        addr,
        &put_with_cookie("/admin/config", &cookie_header, Some(&cookies["sd_csrf"]), body),
    )
    .await;
    assert_eq!(status, 200, "body: {resp_body}");
}
