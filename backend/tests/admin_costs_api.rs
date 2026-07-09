//! Admin maliyet özeti (Faz 1.14) uçtan-uca testleri —
//! `GET /admin/costs/summary`: toplam kullanım saati, tahmini maliyet,
//! tahmini gelir.

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
};

const SECRET: &[u8] = b"admin-costs-api-test-secret";

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

fn token(role: &str) -> String {
    let jwt = Jwt::new(SECRET);
    let claims = Claims {
        sub: "00000000-0000-0000-0000-000000000077".into(),
        role: role.into(),
        plan: "streamer".into(),
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
async fn requires_admin() {
    let (addr, _state) = spawn_server().await;
    let (status, _) = http_get_with_token(addr, "/admin/costs/summary", "not-a-real-token").await;
    assert_eq!(status, 401);

    let (status, _) = http_get_with_token(addr, "/admin/costs/summary", &token("broadcaster")).await;
    assert_eq!(status, 403);
}

#[tokio::test]
async fn reports_zero_usage_and_zero_revenue_with_no_users() {
    let (addr, _state) = spawn_server().await;
    let (status, body) = http_get_with_token(addr, "/admin/costs/summary", &token("admin")).await;
    assert_eq!(status, 200, "body: {body}");

    let parsed: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(parsed["user_count"], 0);
    assert_eq!(parsed["total_usage_hours"], 0.0);
    assert_eq!(parsed["estimated_cost_cents"], 0);
    assert_eq!(parsed["estimated_monthly_revenue_cents"], 0);
    assert!(parsed["period"].as_str().unwrap().starts_with("20"));
}

#[tokio::test]
async fn estimates_cost_from_usage_and_revenue_from_seeded_users_plans() {
    let (addr, state) = spawn_server().await;

    // İki yayıncı: biri free (0 gelir), biri starter (999 cent).
    state.users.signup("free-user@example.com", "free-pass-1", "free").expect("seed free");
    state.users.signup("starter-user@example.com", "starter-pass-1", "starter").expect("seed starter");

    // 2 saatlik toplam kullanım kaydı.
    state.usage.record("free-user", 3600.0, 5);
    state.usage.record("starter-user", 3600.0, 30);

    let (status, body) = http_get_with_token(addr, "/admin/costs/summary", &token("admin")).await;
    assert_eq!(status, 200, "body: {body}");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("json");

    assert_eq!(parsed["user_count"], 2);
    assert_eq!(parsed["total_usage_hours"], 2.0);
    // Varsayılan oranlar: stt 0.05 + translate 0.04 = 0.09 $/saat × 2 saat = 0.18$ = 18 cent.
    assert_eq!(parsed["estimated_cost_cents"], 18);
    // free(0) + starter(999) = 999 cent.
    assert_eq!(parsed["estimated_monthly_revenue_cents"], 999);
}

#[tokio::test]
async fn honors_admin_configured_hourly_rates() {
    let (addr, state) = spawn_server().await;
    state.service_config.set("cost.stt.usd_per_hour", "0.10");
    state.service_config.set("cost.translate.usd_per_hour", "0.10");
    state.usage.record("someone", 3600.0, 5);

    let (status, body) = http_get_with_token(addr, "/admin/costs/summary", &token("admin")).await;
    assert_eq!(status, 200, "body: {body}");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(parsed["stt_usd_per_hour"], 0.10);
    assert_eq!(parsed["translate_usd_per_hour"], 0.10);
    // 0.20 $/saat × 1 saat = 20 cent.
    assert_eq!(parsed["estimated_cost_cents"], 20);
}
