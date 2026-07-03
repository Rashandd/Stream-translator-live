//! Admin secrets API uçtan-uca testleri.
//!
//! Sözleşme: secret'lar admin panelden GİRİLİR, sadece baş+son maskeli
//! OKUNUR. Tam değer hiçbir yanıtta yer almaz. Yazma sadece admin'e açıktır
//! ve yönetilen ad listesiyle sınırlıdır; güncelleme audit log'a düşer.

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
    secrets::{ManagedSecrets, MemorySecretStore},
    state::AppState,
    usage::UsageTracker,
};

const SECRET: &[u8] = b"secrets-api-test-secret";

/// Taban store'u boş (env'den bağımsız) bir test sunucusu kurar.
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
        secrets: Arc::new(ManagedSecrets::new(Box::new(MemorySecretStore::new()), "env")),
    };

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
        plan: "pro".into(),
        exp: chrono::Utc::now().timestamp() + 3600,
    };
    jwt.issue(&claims).expect("issue token")
}

/// Tek bir HTTP isteği gönderir, (status, body) döndürür.
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
async fn admin_lists_managed_secrets_masked() {
    let addr = spawn_server().await;
    let (status, body) = http(addr, &request("GET", "/admin/secrets", Some(&token("admin")), None)).await;
    assert_eq!(status, 200, "body: {body}");

    let items: serde_json::Value = serde_json::from_str(&body).expect("json");
    let names: Vec<&str> = items
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"DEEPGRAM_KEY"), "names: {names:?}");
    assert!(names.contains(&"STRIPE_SECRET_KEY"));
    // JWT_SECRET panelden yönetilmez — listede olmamalı.
    assert!(!names.contains(&"JWT_SECRET"));

    // Boş store → hepsi unset.
    for item in items.as_array().unwrap() {
        assert_eq!(item["configured"], false);
        assert_eq!(item["source"], "unset");
        assert!(item["masked"].is_null());
    }
}

#[tokio::test]
async fn put_secret_activates_and_reads_back_masked_only() {
    let addr = spawn_server().await;
    let raw_value = "dg_live_supersecret_9876";

    let (status, body) = http(
        addr,
        &request(
            "PUT",
            "/admin/secrets/DEEPGRAM_KEY",
            Some(&token("admin")),
            Some(&format!(r#"{{"value":"{raw_value}"}}"#)),
        ),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    // Yanıt tam değeri asla içermez — sadece baş+son maskesi.
    assert!(!body.contains(raw_value), "full secret leaked: {body}");
    assert!(body.contains(r#""masked":"dg_l…9876""#), "body: {body}");

    let (_, body) = http(addr, &request("GET", "/admin/secrets", Some(&token("admin")), None)).await;
    let items: serde_json::Value = serde_json::from_str(&body).expect("json");
    let dg = items
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["name"] == "DEEPGRAM_KEY")
        .expect("deepgram row");
    assert_eq!(dg["configured"], true);
    assert_eq!(dg["source"], "panel");
    assert_eq!(dg["masked"], "dg_l…9876");
    assert!(!body.contains(raw_value));

    // Güncelleme audit iz bırakır (değersiz).
    let (_, logs) = http(
        addr,
        &request("GET", "/admin/logs?service=secrets", Some(&token("admin")), None),
    )
    .await;
    assert!(logs.contains("secret updated: DEEPGRAM_KEY"), "logs: {logs}");
    assert!(!logs.contains(raw_value));
}

#[tokio::test]
async fn delete_clears_override_back_to_unset() {
    let addr = spawn_server().await;
    let admin = token("admin");

    let (status, _) = http(
        addr,
        &request(
            "PUT",
            "/admin/secrets/DEEPL_KEY",
            Some(&admin),
            Some(r#"{"value":"dl_key_123456"}"#),
        ),
    )
    .await;
    assert_eq!(status, 200);

    let (status, body) = http(
        addr,
        &request("DELETE", "/admin/secrets/DEEPL_KEY", Some(&admin), None),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");

    let items: serde_json::Value = serde_json::from_str(&body).expect("json");
    let dl = items
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["name"] == "DEEPL_KEY")
        .expect("deepl row");
    assert_eq!(dl["configured"], false);
    assert_eq!(dl["source"], "unset");
}

#[tokio::test]
async fn unknown_name_and_empty_value_are_rejected() {
    let addr = spawn_server().await;
    let admin = token("admin");

    // JWT_SECRET yönetilen listede değil → 404.
    let (status, _) = http(
        addr,
        &request(
            "PUT",
            "/admin/secrets/JWT_SECRET",
            Some(&admin),
            Some(r#"{"value":"x"}"#),
        ),
    )
    .await;
    assert_eq!(status, 404);

    let (status, _) = http(
        addr,
        &request(
            "PUT",
            "/admin/secrets/ATTACKER_KEY",
            Some(&admin),
            Some(r#"{"value":"x"}"#),
        ),
    )
    .await;
    assert_eq!(status, 404);

    let (status, _) = http(
        addr,
        &request(
            "PUT",
            "/admin/secrets/DEEPL_KEY",
            Some(&admin),
            Some(r#"{"value":"  "}"#),
        ),
    )
    .await;
    assert_eq!(status, 400);
}

#[tokio::test]
async fn secrets_api_requires_admin() {
    let addr = spawn_server().await;

    let (status, _) = http(addr, &request("GET", "/admin/secrets", None, None)).await;
    assert_eq!(status, 401);

    let (status, _) = http(
        addr,
        &request("GET", "/admin/secrets", Some(&token("broadcaster")), None),
    )
    .await;
    assert_eq!(status, 403);

    let (status, _) = http(
        addr,
        &request(
            "PUT",
            "/admin/secrets/DEEPGRAM_KEY",
            Some(&token("broadcaster")),
            Some(r#"{"value":"x-123456"}"#),
        ),
    )
    .await;
    assert_eq!(status, 403);
}
