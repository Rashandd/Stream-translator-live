//! WebSocket sunucu uçtan-uca testleri — gerçek tokio-tungstenite client ile.
//!
//! `docs/websocket-protocol.md` akışını soket üzerinden doğrular:
//! auth → session_ready → stream_start → binary frame → subtitle, ping → pong.

use std::net::SocketAddr;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use streamdubber_backend::{
    app,
    auth::jwt::{Claims, Jwt},
    config::ServiceConfig,
    gateway::{stt::MockStt, translate::MockTranslate, Gateway},
    state::AppState,
};

const SECRET: &[u8] = b"integration-test-secret";

/// Test app'ini rastgele portta ayağa kaldırır ve adresini döndürür.
async fn spawn_server() -> SocketAddr {
    let service_config = Arc::new(ServiceConfig::with_defaults());
    let mut gateway = Gateway::new(Arc::clone(&service_config));
    gateway.register_stt(Arc::new(MockStt::new("deepgram")));
    gateway.register_translate(Arc::new(MockTranslate::new("deepl")));

    let state = AppState::skeleton(Jwt::new(SECRET), Arc::clone(&service_config), gateway);

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        axum::serve(listener, app::router(state))
            .await
            .expect("serve");
    });
    addr
}

/// Geçerli bir access token üretir.
fn valid_token() -> String {
    let jwt = Jwt::new(SECRET);
    let claims = Claims {
        sub: "11111111-1111-1111-1111-111111111111".into(),
        role: "broadcaster".into(),
        plan: "free".into(),
        exp: chrono::Utc::now().timestamp() + 3600,
    };
    jwt.issue(&claims).expect("issue token")
}

/// SDAU binary ses frame'i kurar.
fn audio_frame(payload: &[u8]) -> Vec<u8> {
    let len = payload.len() as u16;
    let mut buf = vec![0x53, 0x44, 0x41, 0x55, 0x01, 0x00];
    buf.extend_from_slice(&len.to_be_bytes());
    buf.extend_from_slice(payload);
    buf
}

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Bir sonraki text mesajını JSON olarak okur.
async fn next_json(ws: &mut Ws) -> Value {
    loop {
        let msg = ws.next().await.expect("stream open").expect("ws message");
        if let Message::Text(text) = msg {
            return serde_json::from_str(text.as_str()).expect("valid json");
        }
    }
}

async fn send_json(ws: &mut Ws, json: Value) {
    ws.send(Message::Text(json.to_string().into()))
        .await
        .expect("send");
}

#[tokio::test]
async fn full_stream_flow() {
    let addr = spawn_server().await;
    let url = format!("ws://{addr}/ws/stream");
    let (mut ws, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("connect");

    // auth → session_ready
    send_json(
        &mut ws,
        serde_json::json!({"type":"auth","token": valid_token()}),
    )
    .await;
    let ready = next_json(&mut ws).await;
    assert_eq!(ready["type"], "session_ready");
    assert_eq!(ready["user_id"], "11111111-1111-1111-1111-111111111111");
    assert_eq!(ready["plan"], "free");

    // stream_start → stream_ack
    send_json(
        &mut ws,
        serde_json::json!({
            "type":"stream_start",
            "session_id":"s1",
            "source_lang":"tr",
            "target_langs":["en","de"],
            "audio_format":{"codec":"opus","sample_rate":16000,"channels":1}
        }),
    )
    .await;
    let ack = next_json(&mut ws).await;
    assert_eq!(ack["type"], "stream_ack");
    assert_eq!(ack["status"], "streaming");

    // binary audio frame → subtitle
    ws.send(Message::Binary(audio_frame(&[1, 2, 3, 4]).into()))
        .await
        .expect("send binary");
    let subtitle = next_json(&mut ws).await;
    assert_eq!(subtitle["type"], "subtitle");
    assert_eq!(subtitle["sequence"], 0);
    assert_eq!(subtitle["source"]["lang"], "tr");
    assert_eq!(subtitle["translations"].as_array().unwrap().len(), 2);

    // ping → pong
    send_json(&mut ws, serde_json::json!({"type":"ping","ts":12345})).await;
    let pong = next_json(&mut ws).await;
    assert_eq!(pong["type"], "pong");
    assert_eq!(pong["ts"], 12345);
}

#[tokio::test]
async fn invalid_token_is_rejected() {
    let addr = spawn_server().await;
    let url = format!("ws://{addr}/ws/stream");
    let (mut ws, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("connect");

    send_json(
        &mut ws,
        serde_json::json!({"type":"auth","token":"garbage.token"}),
    )
    .await;
    let err = next_json(&mut ws).await;
    assert_eq!(err["type"], "error");
    assert_eq!(err["code"], 4001);
}

/// Local STT tam akışı (Faz 2.6b): stt_source=local ile stream açılır,
/// ses yerine transcript mesajı gönderilir, backend STT'yi atlayıp çevirir.
#[tokio::test]
async fn local_stt_transcript_flow() {
    let addr = spawn_server().await;
    let url = format!("ws://{addr}/ws/stream");
    let (mut ws, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("connect");

    send_json(
        &mut ws,
        serde_json::json!({"type":"auth","token": valid_token()}),
    )
    .await;
    let _ready = next_json(&mut ws).await;

    send_json(
        &mut ws,
        serde_json::json!({
            "type":"stream_start",
            "session_id":"s1",
            "source_lang":"tr",
            "target_langs":["en","de"],
            "audio_format":{"codec":"opus","sample_rate":16000,"channels":1},
            "stt_source":"local"
        }),
    )
    .await;
    let ack = next_json(&mut ws).await;
    assert_eq!(ack["type"], "stream_ack");

    // transcript → subtitle: kaynak metin aynen korunur, STT çağrılmaz.
    send_json(
        &mut ws,
        serde_json::json!({
            "type":"transcript",
            "session_id":"s1",
            "text":"bugün çok iyi bir gün"
        }),
    )
    .await;
    let subtitle = next_json(&mut ws).await;
    assert_eq!(subtitle["type"], "subtitle");
    assert_eq!(subtitle["source"]["text"], "bugün çok iyi bir gün");
    assert_eq!(subtitle["source"]["is_final"], true);
    assert_eq!(subtitle["translations"].as_array().unwrap().len(), 2);

    // Local session'da bulut STT fallback'i de çalışır — sequence paylaşılır.
    ws.send(Message::Binary(audio_frame(&[1, 2, 3, 4]).into()))
        .await
        .expect("send binary");
    let second = next_json(&mut ws).await;
    assert_eq!(second["type"], "subtitle");
    assert_eq!(second["sequence"], 1);
}

/// Cloud (varsayılan) session'dan gelen transcript 4007 ile reddedilir —
/// mod, plan gating'i baypas etmemek için yayın başında bildirilmek zorunda.
#[tokio::test]
async fn transcript_on_cloud_session_rejected() {
    let addr = spawn_server().await;
    let url = format!("ws://{addr}/ws/stream");
    let (mut ws, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("connect");

    send_json(
        &mut ws,
        serde_json::json!({"type":"auth","token": valid_token()}),
    )
    .await;
    let _ready = next_json(&mut ws).await;

    send_json(
        &mut ws,
        serde_json::json!({
            "type":"stream_start",
            "session_id":"s1",
            "source_lang":"tr",
            "target_langs":["en"],
            "audio_format":{"codec":"opus","sample_rate":16000,"channels":1}
        }),
    )
    .await;
    let _ack = next_json(&mut ws).await;

    send_json(
        &mut ws,
        serde_json::json!({"type":"transcript","session_id":"s1","text":"merhaba"}),
    )
    .await;
    let err = next_json(&mut ws).await;
    assert_eq!(err["type"], "error");
    assert_eq!(err["code"], 4007);
    assert_eq!(err["message"], "stt_source_not_local");
}

/// `stt.local.max_chars` sınırını aşan transcript 4007 ile reddedilir;
/// session açık kalır, normal uzunlukta satır işlenmeye devam eder.
#[tokio::test]
async fn overlong_transcript_rejected_but_session_survives() {
    let addr = spawn_server().await;
    let url = format!("ws://{addr}/ws/stream");
    let (mut ws, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("connect");

    send_json(
        &mut ws,
        serde_json::json!({"type":"auth","token": valid_token()}),
    )
    .await;
    let _ready = next_json(&mut ws).await;

    send_json(
        &mut ws,
        serde_json::json!({
            "type":"stream_start",
            "session_id":"s1",
            "source_lang":"tr",
            "target_langs":["en"],
            "audio_format":{"codec":"opus","sample_rate":16000,"channels":1},
            "stt_source":"local"
        }),
    )
    .await;
    let _ack = next_json(&mut ws).await;

    // Varsayılan sınır 500 karakter → 600 karakterlik satır reddedilir.
    send_json(
        &mut ws,
        serde_json::json!({"type":"transcript","session_id":"s1","text":"a".repeat(600)}),
    )
    .await;
    let err = next_json(&mut ws).await;
    assert_eq!(err["type"], "error");
    assert_eq!(err["code"], 4007);
    assert_eq!(err["message"], "transcript_too_long");

    // Session kapanmadı — normal satır hâlâ işleniyor.
    send_json(
        &mut ws,
        serde_json::json!({"type":"transcript","session_id":"s1","text":"merhaba"}),
    )
    .await;
    let subtitle = next_json(&mut ws).await;
    assert_eq!(subtitle["type"], "subtitle");
    assert_eq!(subtitle["source"]["text"], "merhaba");
}

#[tokio::test]
async fn transcript_before_stream_start_errors() {
    let addr = spawn_server().await;
    let url = format!("ws://{addr}/ws/stream");
    let (mut ws, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("connect");

    send_json(
        &mut ws,
        serde_json::json!({"type":"auth","token": valid_token()}),
    )
    .await;
    let _ready = next_json(&mut ws).await;

    send_json(
        &mut ws,
        serde_json::json!({"type":"transcript","session_id":"s1","text":"merhaba"}),
    )
    .await;
    let err = next_json(&mut ws).await;
    assert_eq!(err["type"], "error");
    assert_eq!(err["code"], 4002);
}

#[tokio::test]
async fn audio_before_stream_start_errors() {
    let addr = spawn_server().await;
    let url = format!("ws://{addr}/ws/stream");
    let (mut ws, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("connect");

    send_json(
        &mut ws,
        serde_json::json!({"type":"auth","token": valid_token()}),
    )
    .await;
    let _ready = next_json(&mut ws).await;

    // stream_start göndermeden binary frame → 4002
    ws.send(Message::Binary(audio_frame(&[1, 2, 3]).into()))
        .await
        .expect("send binary");
    let err = next_json(&mut ws).await;
    assert_eq!(err["type"], "error");
    assert_eq!(err["code"], 4002);
}
