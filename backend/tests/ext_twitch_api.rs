//! Twitch Extension EBS uçtan-uca testleri (Faz 2.4 / plan B2).
//!
//! Kapsam: feature gate (varsayılan kapalı), kanal bağlama akışı (dashboard
//! kodu → Twitch config claim), state endpoint'i (poll fallback), canlı
//! caption WebSocket'i (SDK → EBS → viewer), secret rotasyonu, rate limit,
//! viewer kapasitesi ve CORS preflight.

use std::net::SocketAddr;
use std::sync::Arc;

use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

use streamdubber_backend::{
    app,
    auth::jwt::{Claims, Jwt},
    config::ServiceConfig,
    gateway::{stt::MockStt, translate::MockTranslate, Gateway},
    secrets::{names, ManagedSecrets},
    state::AppState,
};

const APP_SECRET: &[u8] = b"ext-twitch-api-test-secret";
/// SDK yayıncısının StreamDubber kullanıcı kimliği.
const USER_ID: &str = "22222222-2222-2222-2222-222222222222";
const CHANNEL: &str = "ch-100";

struct TestServer {
    addr: SocketAddr,
    config: Arc<ServiceConfig>,
    secrets: Arc<ManagedSecrets>,
}

async fn spawn_server() -> TestServer {
    let service_config = Arc::new(ServiceConfig::with_defaults());
    let mut gateway = Gateway::new(Arc::clone(&service_config));
    gateway.register_stt(Arc::new(MockStt::new("deepgram")));
    gateway.register_translate(Arc::new(MockTranslate::new("deepl")));

    let state = AppState::skeleton(Jwt::new(APP_SECRET), Arc::clone(&service_config), gateway);
    let secrets = Arc::clone(&state.secrets);

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        axum::serve(listener, app::router(state)).await.expect("serve");
    });
    TestServer {
        addr,
        config: service_config,
        secrets,
    }
}

fn tw_secret_b64(raw: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(raw)
}

/// Extension'ı açar ve Twitch shared secret'ını panel katmanından ayarlar.
fn enable_ext(server: &TestServer, secret_b64: &str) {
    server.config.set("ext.twitch.enabled", "true");
    server
        .secrets
        .set(names::TWITCH_EXT_SECRET, secret_b64)
        .expect("set twitch secret");
}

/// StreamDubber (dashboard) JWT'si.
fn app_token(sub: &str) -> String {
    let jwt = Jwt::new(APP_SECRET);
    let claims = Claims {
        sub: sub.into(),
        role: "broadcaster".into(),
        plan: "free".into(),
        exp: chrono::Utc::now().timestamp() + 3600,
    };
    jwt.issue(&claims).expect("issue app token")
}

#[derive(serde::Serialize)]
struct TwClaims {
    exp: i64,
    channel_id: String,
    role: String,
    opaque_user_id: String,
}

/// Twitch extension JWT'si — verilen base64 secret ile imzalanır.
fn twitch_token(secret_b64: &str, channel: &str, role: &str, opaque: &str) -> String {
    let key = jsonwebtoken::EncodingKey::from_base64_secret(secret_b64).expect("key");
    let claims = TwClaims {
        exp: chrono::Utc::now().timestamp() + 300,
        channel_id: channel.into(),
        role: role.into(),
        opaque_user_id: opaque.into(),
    };
    jsonwebtoken::encode(&jsonwebtoken::Header::default(), &claims, &key).expect("sign")
}

/// Tek bir HTTP isteği gönderir, tam yanıt metnini döndürür.
async fn http_raw(addr: SocketAddr, request: &str) -> String {
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    stream.write_all(request.as_bytes()).await.expect("write");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.expect("read");
    String::from_utf8_lossy(&buf).to_string()
}

/// (status, body) çifti.
async fn http(addr: SocketAddr, request: &str) -> (u16, String) {
    let text = http_raw(addr, request).await;
    let status: u16 = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .expect("status line");
    let body = text.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    (status, body)
}

fn get_req(path: &str, bearer: Option<&str>) -> String {
    let auth = bearer
        .map(|t| format!("Authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    format!("GET {path} HTTP/1.1\r\nHost: t\r\n{auth}Connection: close\r\n\r\n")
}

fn post_json(path: &str, bearer: Option<&str>, json: &str) -> String {
    let auth = bearer
        .map(|t| format!("Authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    format!(
        "POST {path} HTTP/1.1\r\nHost: t\r\n{auth}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
        json.len()
    )
}

fn delete_req(path: &str, bearer: Option<&str>) -> String {
    let auth = bearer
        .map(|t| format!("Authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    format!("DELETE {path} HTTP/1.1\r\nHost: t\r\n{auth}Connection: close\r\n\r\n")
}

/// Dashboard'dan bağlama kodu üretir, Twitch broadcaster JWT ile claim eder.
async fn bind_channel(server: &TestServer, secret_b64: &str, user: &str, channel: &str) {
    let (status, body) = http(
        server.addr,
        &post_json("/api/ext/twitch/code", Some(&app_token(user)), "{}"),
    )
    .await;
    assert_eq!(status, 200, "code issue failed: {body}");
    let code = serde_json::from_str::<Value>(&body).expect("json")["code"]
        .as_str()
        .expect("code")
        .to_string();

    let tw = twitch_token(secret_b64, channel, "broadcaster", "U-bc");
    let (status, body) = http(
        server.addr,
        &post_json(
            "/ext/twitch/bind",
            Some(&tw),
            &format!(r#"{{"code":"{code}"}}"#),
        ),
    )
    .await;
    assert_eq!(status, 200, "bind failed: {body}");
    assert!(body.contains(r#""status":"linked""#), "body: {body}");
}

// --- WS yardımcıları (overlay_api ile aynı desen) ---------------------------

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn next_json(ws: &mut Ws) -> Value {
    loop {
        let msg = ws.next().await.expect("stream open").expect("ws message");
        if let Message::Text(text) = msg {
            return serde_json::from_str(text.as_str()).expect("valid json");
        }
    }
}

async fn send_json(ws: &mut Ws, json: Value) {
    ws.send(Message::Text(json.to_string().into())).await.expect("send");
}

fn audio_frame(payload: &[u8]) -> Vec<u8> {
    let len = payload.len() as u16;
    let mut buf = vec![0x53, 0x44, 0x41, 0x55, 0x01, 0x00];
    buf.extend_from_slice(&len.to_be_bytes());
    buf.extend_from_slice(payload);
    buf
}

/// SDK'yı bağlar, yayını başlatır ve bir altyazı ürettirir; canlı SDK
/// soketini döndürür (drop edilirse yayın "kapanır").
async fn start_stream_with_subtitle(server: &TestServer) -> (Ws, Value) {
    let sdk_url = format!("ws://{}/ws/stream", server.addr);
    let (mut sdk, _) = tokio_tungstenite::connect_async(sdk_url).await.expect("connect sdk");
    send_json(&mut sdk, serde_json::json!({"type":"auth","token": app_token(USER_ID)})).await;
    let _ready = next_json(&mut sdk).await;
    send_json(
        &mut sdk,
        serde_json::json!({
            "type":"stream_start",
            "session_id":"s1",
            "source_lang":"tr",
            "target_langs":["en"],
            "audio_format":{"codec":"opus","sample_rate":16000,"channels":1}
        }),
    )
    .await;
    let _ack = next_json(&mut sdk).await;
    sdk.send(Message::Binary(audio_frame(&[1, 2, 3]).into())).await.expect("send audio");
    let subtitle = next_json(&mut sdk).await;
    assert_eq!(subtitle["type"], "subtitle");
    (sdk, subtitle)
}

/// WS bağlantı denemesinin HTTP hata koduyla reddedildiğini doğrular.
async fn expect_ws_rejects(url: &str, expected_status: u16) {
    match tokio_tungstenite::connect_async(url).await {
        Err(tokio_tungstenite::tungstenite::Error::Http(resp)) => {
            assert_eq!(resp.status().as_u16(), expected_status);
        }
        Ok(_) => panic!("connection unexpectedly accepted"),
        Err(other) => panic!("unexpected error: {other}"),
    }
}

// --- Testler -----------------------------------------------------------------

#[tokio::test]
async fn surface_is_disabled_by_default() {
    let server = spawn_server().await;
    // Secret yapılandırılmış olsa bile gate kapalıyken her uç 503.
    server
        .secrets
        .set(names::TWITCH_EXT_SECRET, &tw_secret_b64(b"s"))
        .expect("set");

    let (status, _) = http(
        server.addr,
        &post_json("/api/ext/twitch/code", Some(&app_token(USER_ID)), "{}"),
    )
    .await;
    assert_eq!(status, 503);

    let tw = twitch_token(&tw_secret_b64(b"s"), CHANNEL, "viewer", "U-1");
    let (status, _) = http(
        server.addr,
        &get_req(&format!("/ext/twitch/state/{CHANNEL}"), Some(&tw)),
    )
    .await;
    assert_eq!(status, 503);

    let url = format!("ws://{}/ext/twitch/captions/{CHANNEL}?token={tw}", server.addr);
    expect_ws_rejects(&url, 503).await;
}

#[tokio::test]
async fn bind_flow_links_channel_and_state_reports_it() {
    let server = spawn_server().await;
    let secret = tw_secret_b64(b"bind-secret");
    enable_ext(&server, &secret);

    bind_channel(&server, &secret, USER_ID, CHANNEL).await;

    // Dashboard durumu bağlı kanalı listeler.
    let (status, body) = http(
        server.addr,
        &get_req("/api/ext/twitch/binding", Some(&app_token(USER_ID))),
    )
    .await;
    assert_eq!(status, 200);
    assert!(body.contains(r#""linked":true"#), "body: {body}");
    assert!(body.contains(CHANNEL), "body: {body}");

    // Viewer state: configured ama canlı yayın yok.
    let viewer = twitch_token(&secret, CHANNEL, "viewer", "U-1");
    let (status, body) = http(
        server.addr,
        &get_req(&format!("/ext/twitch/state/{CHANNEL}"), Some(&viewer)),
    )
    .await;
    assert_eq!(status, 200);
    assert!(body.contains(r#""configured":true"#), "body: {body}");
    assert!(body.contains(r#""live":false"#), "body: {body}");

    // Unlink: dashboard bağları kaldırır, state yeniden "not configured".
    let (status, body) = http(
        server.addr,
        &delete_req("/api/ext/twitch/binding", Some(&app_token(USER_ID))),
    )
    .await;
    assert_eq!(status, 200);
    assert!(body.contains(r#""linked":false"#), "body: {body}");

    let (status, body) = http(
        server.addr,
        &get_req(&format!("/ext/twitch/state/{CHANNEL}"), Some(&viewer)),
    )
    .await;
    assert_eq!(status, 200);
    assert!(body.contains(r#""configured":false"#), "body: {body}");
}

#[tokio::test]
async fn bind_requires_broadcaster_role_and_valid_code() {
    let server = spawn_server().await;
    let secret = tw_secret_b64(b"role-secret");
    enable_ext(&server, &secret);

    // Viewer rolü kod claim edemez.
    let viewer = twitch_token(&secret, CHANNEL, "viewer", "U-1");
    let (status, _) = http(
        server.addr,
        &post_json("/ext/twitch/bind", Some(&viewer), r#"{"code":"123456"}"#),
    )
    .await;
    assert_eq!(status, 403);

    let bc = twitch_token(&secret, CHANNEL, "broadcaster", "U-bc");
    // Bilinmeyen kod 404.
    let (status, _) = http(
        server.addr,
        &post_json("/ext/twitch/bind", Some(&bc), r#"{"code":"123456"}"#),
    )
    .await;
    assert_eq!(status, 404);
    // Bozuk format 400.
    let (status, _) = http(
        server.addr,
        &post_json("/ext/twitch/bind", Some(&bc), r#"{"code":"12ab"}"#),
    )
    .await;
    assert_eq!(status, 400);
}

#[tokio::test]
async fn state_rejects_channel_mismatch_and_bad_token() {
    let server = spawn_server().await;
    let secret = tw_secret_b64(b"mismatch-secret");
    enable_ext(&server, &secret);

    // Token ch-1 için, istek ch-2'ye: yatay erişim engellenir.
    let viewer = twitch_token(&secret, "ch-1", "viewer", "U-1");
    let (status, _) = http(
        server.addr,
        &get_req("/ext/twitch/state/ch-2", Some(&viewer)),
    )
    .await;
    assert_eq!(status, 403);

    // Yanlış secret'la imzalı token 401.
    let forged = twitch_token(&tw_secret_b64(b"other"), "ch-1", "viewer", "U-1");
    let (status, _) = http(
        server.addr,
        &get_req("/ext/twitch/state/ch-1", Some(&forged)),
    )
    .await;
    assert_eq!(status, 401);

    // Başlıksız istek 401.
    let (status, _) = http(server.addr, &get_req("/ext/twitch/state/ch-1", None)).await;
    assert_eq!(status, 401);
}

#[tokio::test]
async fn rotated_secret_still_verifies_previous_tokens() {
    let server = spawn_server().await;
    let old = tw_secret_b64(b"old-secret");
    let new = tw_secret_b64(b"new-secret");
    enable_ext(&server, &new);
    server
        .secrets
        .set(names::TWITCH_EXT_SECRET_PREVIOUS, &old)
        .expect("set previous");

    // Rotasyondan önce imzalanmış (eski secret) token hâlâ kabul edilir.
    let viewer = twitch_token(&old, CHANNEL, "viewer", "U-1");
    let (status, body) = http(
        server.addr,
        &get_req(&format!("/ext/twitch/state/{CHANNEL}"), Some(&viewer)),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
}

#[tokio::test]
async fn state_rate_limit_returns_429() {
    let server = spawn_server().await;
    let secret = tw_secret_b64(b"rate-secret");
    enable_ext(&server, &secret);
    server.config.set("ext.twitch.state_rate_per_min", "2");

    let viewer = twitch_token(&secret, CHANNEL, "viewer", "U-rate");
    for _ in 0..2 {
        let (status, _) = http(
            server.addr,
            &get_req(&format!("/ext/twitch/state/{CHANNEL}"), Some(&viewer)),
        )
        .await;
        assert_eq!(status, 200);
    }
    let (status, _) = http(
        server.addr,
        &get_req(&format!("/ext/twitch/state/{CHANNEL}"), Some(&viewer)),
    )
    .await;
    assert_eq!(status, 429);

    // Farklı viewer etkilenmez.
    let other = twitch_token(&secret, CHANNEL, "viewer", "U-other");
    let (status, _) = http(
        server.addr,
        &get_req(&format!("/ext/twitch/state/{CHANNEL}"), Some(&other)),
    )
    .await;
    assert_eq!(status, 200);
}

#[tokio::test]
async fn viewer_ws_receives_captions_without_overlay_token() {
    let server = spawn_server().await;
    let secret = tw_secret_b64(b"captions-secret");
    enable_ext(&server, &secret);
    bind_channel(&server, &secret, USER_ID, CHANNEL).await;

    // SDK canlı yayında bir altyazı üretir (snapshot'a düşer).
    let (_sdk, sdk_subtitle) = start_stream_with_subtitle(&server).await;

    // State artık live + son satırı içeriyor.
    let viewer = twitch_token(&secret, CHANNEL, "viewer", "U-1");
    let (status, body) = http(
        server.addr,
        &get_req(&format!("/ext/twitch/state/{CHANNEL}"), Some(&viewer)),
    )
    .await;
    assert_eq!(status, 200);
    assert!(body.contains(r#""live":true"#), "body: {body}");
    assert!(body.contains(r#""languages":["tr","en"]"#), "body: {body}");

    // Viewer WS: snapshot replay'i alır; mesaj SDK'nınkiyle birebir aynı ve
    // overlay token'ı İÇERMEZ.
    let url = format!(
        "ws://{}/ext/twitch/captions/{CHANNEL}?token={viewer}",
        server.addr
    );
    let (mut ws, _) = tokio_tungstenite::connect_async(url).await.expect("connect viewer ws");
    let replayed = next_json(&mut ws).await;
    assert_eq!(replayed, sdk_subtitle);
    assert!(replayed.get("overlay_token").is_none());
}

#[tokio::test]
async fn viewer_ws_rejections_cover_all_gates() {
    let server = spawn_server().await;
    let secret = tw_secret_b64(b"gates-secret");
    enable_ext(&server, &secret);

    // Binding yokken 404.
    let viewer = twitch_token(&secret, CHANNEL, "viewer", "U-1");
    let url = format!(
        "ws://{}/ext/twitch/captions/{CHANNEL}?token={viewer}",
        server.addr
    );
    expect_ws_rejects(&url, 404).await;

    // Binding var ama yayın yok: 409.
    bind_channel(&server, &secret, USER_ID, CHANNEL).await;
    expect_ws_rejects(&url, 409).await;

    // Geçersiz token: 401. Token'sız: 401.
    let bad = format!(
        "ws://{}/ext/twitch/captions/{CHANNEL}?token=garbage",
        server.addr
    );
    expect_ws_rejects(&bad, 401).await;
    let missing = format!("ws://{}/ext/twitch/captions/{CHANNEL}", server.addr);
    expect_ws_rejects(&missing, 401).await;
}

#[tokio::test]
async fn viewer_capacity_is_enforced_per_channel() {
    let server = spawn_server().await;
    let secret = tw_secret_b64(b"cap-secret");
    enable_ext(&server, &secret);
    server.config.set("ext.twitch.max_viewers_per_channel", "1");
    bind_channel(&server, &secret, USER_ID, CHANNEL).await;
    let (_sdk, _) = start_stream_with_subtitle(&server).await;

    let viewer = twitch_token(&secret, CHANNEL, "viewer", "U-1");
    let url = format!(
        "ws://{}/ext/twitch/captions/{CHANNEL}?token={viewer}",
        server.addr
    );
    let (_ws1, _) = tokio_tungstenite::connect_async(&url).await.expect("first viewer");
    expect_ws_rejects(&url, 429).await;
}

#[tokio::test]
async fn cors_preflight_allows_twitch_cdn_origin() {
    let server = spawn_server().await;
    let request = format!(
        "OPTIONS /ext/twitch/state/{CHANNEL} HTTP/1.1\r\nHost: t\r\nOrigin: https://abc123.ext-twitch.tv\r\nAccess-Control-Request-Method: GET\r\nAccess-Control-Request-Headers: authorization\r\nConnection: close\r\n\r\n"
    );
    let text = http_raw(server.addr, &request).await.to_lowercase();
    assert!(
        text.contains("access-control-allow-origin: *"),
        "response: {text}"
    );
}
