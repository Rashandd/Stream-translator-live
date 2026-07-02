//! Loglama API uçtan-uca testleri (Faz 1.12) — admin-only erişim, filtreler,
//! config-driven seviye ve WS aktivitesinin merkezi depoya düşmesi.

use std::net::SocketAddr;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

use streamdubber_backend::{
    app,
    auth::jwt::{Claims, Jwt},
    billing::Billing,
    config::ServiceConfig,
    gateway::{stt::MockStt, translate::MockTranslate, Gateway},
    integrations::Integrations,
    logging::{LogLevel, Logger},
    overlay::OverlayHub,
    plans::PlanRegistry,
    state::AppState,
    usage::UsageTracker,
};

const SECRET: &[u8] = b"logs-api-test-secret";

/// Sunucuyu ayağa kaldırır; testlerin kayıt basabilmesi için logger ve
/// seviye değiştirebilmesi için config handle'ları da döner.
async fn spawn_server() -> (SocketAddr, Arc<Logger>, Arc<ServiceConfig>) {
    let service_config = Arc::new(ServiceConfig::with_defaults());
    let mut gateway = Gateway::new(Arc::clone(&service_config));
    gateway.register_stt(Arc::new(MockStt::new("deepgram")));
    gateway.register_translate(Arc::new(MockTranslate::new("deepl")));

    let logger = Arc::new(Logger::in_memory(Arc::clone(&service_config)));

    let state = AppState {
        jwt: Arc::new(Jwt::new(SECRET)),
        gateway: Arc::new(gateway),
        logger: Arc::clone(&logger),
        service_config: Arc::clone(&service_config),
        integrations: Arc::new(Integrations::empty()),
        billing: Arc::new(Billing::empty()),
        overlay: Arc::new(OverlayHub::new()),
        plans: Arc::new(PlanRegistry::with_defaults()),
        usage: Arc::new(UsageTracker::new()),
    };

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        axum::serve(listener, app::router(state)).await.expect("serve");
    });
    (addr, logger, service_config)
}

fn token(role: &str) -> String {
    let jwt = Jwt::new(SECRET);
    let claims = Claims {
        sub: "00000000-0000-0000-0000-000000000003".into(),
        role: role.into(),
        plan: "free".into(),
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

fn get_logs(path_query: &str, auth_token: Option<&str>) -> String {
    let auth = auth_token
        .map(|t| format!("Authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    format!("GET {path_query} HTTP/1.1\r\nHost: t\r\n{auth}Connection: close\r\n\r\n")
}

#[tokio::test]
async fn admin_queries_logs_with_filters() {
    let (addr, logger, _) = spawn_server().await;
    logger.log(LogLevel::Info, "ws", "bağlantı açıldı", Some("u1"), None);
    logger.log(LogLevel::Error, "billing", "webhook hatası", Some("u2"), None);
    logger.log(LogLevel::Warn, "gateway", "fallback devrede", Some("u1"), Some("s9"));

    let admin = token("admin");

    // Filtresiz: üçü de döner.
    let (status, body) = http(addr, &get_logs("/admin/logs", Some(&admin))).await;
    assert_eq!(status, 200, "body: {body}");
    assert!(body.contains("webhook hatası") && body.contains("fallback devrede"));

    // Seviye filtresi: sadece error.
    let (_, body) = http(addr, &get_logs("/admin/logs?level=error", Some(&admin))).await;
    assert!(body.contains("webhook hatası"), "body: {body}");
    assert!(!body.contains("fallback devrede"), "body: {body}");

    // Servis filtresi.
    let (_, body) = http(addr, &get_logs("/admin/logs?service=ws", Some(&admin))).await;
    assert!(body.contains("bağlantı açıldı") && !body.contains("webhook"), "body: {body}");

    // Kullanıcı + session filtresi.
    let (_, body) = http(
        addr,
        &get_logs("/admin/logs?user_id=u1&session_id=s9", Some(&admin)),
    )
    .await;
    assert!(body.contains("fallback devrede") && !body.contains("bağlantı açıldı"), "body: {body}");
}

#[tokio::test]
async fn broadcaster_and_anonymous_are_rejected() {
    let (addr, _, _) = spawn_server().await;
    let (status, _) = http(addr, &get_logs("/admin/logs", Some(&token("broadcaster")))).await;
    assert_eq!(status, 403);
    let (status, _) = http(addr, &get_logs("/admin/logs", None)).await;
    assert_eq!(status, 401);
}

#[tokio::test]
async fn level_toggle_via_admin_config_applies_without_restart() {
    let (addr, logger, _) = spawn_server().await;
    let admin = token("admin");

    // Varsayılan seviye info → debug düşer.
    logger.log(LogLevel::Debug, "ws", "önce", None, None);

    // Admin, config API'sinden seviyeyi debug'a çeker (restart yok).
    let payload = r#"{"key":"log.level","value":"debug"}"#;
    let req = format!(
        "PUT /admin/config HTTP/1.1\r\nHost: t\r\nAuthorization: Bearer {admin}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    let (status, _) = http(addr, &req).await;
    assert_eq!(status, 200);

    logger.log(LogLevel::Debug, "ws", "sonra", None, None);

    let (_, body) = http(addr, &get_logs("/admin/logs?level=debug", Some(&admin))).await;
    assert!(body.contains("sonra"), "body: {body}");
    assert!(!body.contains("önce"), "body: {body}");
}

#[tokio::test]
async fn sensitive_values_never_reach_the_store() {
    let (addr, logger, _) = spawn_server().await;
    logger.log(
        LogLevel::Error,
        "gateway",
        "deepgram isteği reddedildi api_key=dg_secret_123",
        None,
        None,
    );

    let (_, body) = http(addr, &get_logs("/admin/logs", Some(&token("admin")))).await;
    assert!(body.contains("[REDACTED]"), "body: {body}");
    assert!(!body.contains("dg_secret_123"), "body: {body}");
}

#[tokio::test]
async fn ws_activity_lands_in_central_log_store() {
    let (addr, _, _) = spawn_server().await;

    // Yayıncı SDK'sı gibi bağlan: auth → stream_start.
    let url = format!("ws://{addr}/ws/stream");
    let (mut ws, _) = tokio_tungstenite::connect_async(url).await.expect("connect");

    let jwt = Jwt::new(SECRET);
    let claims = Claims {
        sub: "broadcaster-log-test".into(),
        role: "broadcaster".into(),
        plan: "free".into(),
        exp: chrono::Utc::now().timestamp() + 3600,
    };
    let broadcaster_token = jwt.issue(&claims).expect("issue");

    ws.send(Message::Text(
        serde_json::json!({"type":"auth","token": broadcaster_token})
            .to_string()
            .into(),
    ))
    .await
    .expect("send auth");
    let _ready = ws.next().await.expect("open").expect("session_ready");

    ws.send(Message::Text(
        serde_json::json!({
            "type":"stream_start","session_id":"log-s1","source_lang":"tr",
            "target_langs":["en"],
            "audio_format":{"codec":"opus","sample_rate":16000,"channels":1}
        })
        .to_string()
        .into(),
    ))
    .await
    .expect("send stream_start");
    let _ack = ws.next().await.expect("open").expect("stream_ack");

    // Admin log deposunda hem auth hem stream_start kaydı görünür.
    let (status, body) = http(
        addr,
        &get_logs(
            "/admin/logs?service=ws&user_id=broadcaster-log-test",
            Some(&token("admin")),
        ),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    assert!(body.contains("websocket authenticated"), "body: {body}");
    assert!(body.contains("stream started"), "body: {body}");
    assert!(body.contains("log-s1"), "body: {body}");
}
