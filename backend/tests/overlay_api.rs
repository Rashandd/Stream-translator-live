//! Overlay WebSocket uçtan-uca testi (Faz 1.9) — bir SDK stream'inde üretilen
//! altyazının, `stream_ack.overlay_token`'ı üzerinden overlay istemcisine
//! (OBS Browser Source rolünde) gerçek zamanlı ulaştığını doğrular.

use std::net::SocketAddr;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use streamdubber_backend::{
    app,
    auth::jwt::{Claims, Jwt},
    billing::Billing,
    config::ServiceConfig,
    gateway::{stt::MockStt, translate::MockTranslate, Gateway},
    integrations::Integrations,
    overlay::OverlayHub,
    plans::PlanRegistry,
    state::AppState,
    usage::UsageTracker,
};

const SECRET: &[u8] = b"overlay-api-test-secret";

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
        secrets: Arc::new(streamdubber_backend::secrets::ManagedSecrets::env()),
    };

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        axum::serve(listener, app::router(state)).await.expect("serve");
    });
    addr
}

fn valid_token() -> String {
    let jwt = Jwt::new(SECRET);
    let claims = Claims {
        sub: "22222222-2222-2222-2222-222222222222".into(),
        role: "broadcaster".into(),
        plan: "free".into(),
        exp: chrono::Utc::now().timestamp() + 3600,
    };
    jwt.issue(&claims).expect("issue token")
}

fn audio_frame(payload: &[u8]) -> Vec<u8> {
    let len = payload.len() as u16;
    let mut buf = vec![0x53, 0x44, 0x41, 0x55, 0x01, 0x00];
    buf.extend_from_slice(&len.to_be_bytes());
    buf.extend_from_slice(payload);
    buf
}

type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

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

#[tokio::test]
async fn overlay_client_receives_subtitles_published_during_stream() {
    let addr = spawn_server().await;

    // SDK bağlantısı: auth → stream_start (overlay_token alınır).
    let sdk_url = format!("ws://{addr}/ws/stream");
    let (mut sdk, _) = tokio_tungstenite::connect_async(sdk_url).await.expect("connect sdk");
    send_json(&mut sdk, serde_json::json!({"type":"auth","token": valid_token()})).await;
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
    let ack = next_json(&mut sdk).await;
    let overlay_token = ack["overlay_token"].as_str().expect("overlay_token present").to_string();
    assert!(!overlay_token.is_empty());

    // Overlay istemcisi (OBS Browser Source rolünde): auth gerektirmez, sadece token.
    let overlay_url = format!("ws://{addr}/overlay/{overlay_token}");
    let (mut overlay_ws, _) = tokio_tungstenite::connect_async(overlay_url).await.expect("connect overlay");

    // Overlay abone olduktan sonra SDK ses gönderir → subtitle hem SDK'ya hem overlay'e gider.
    // Bağlantının kurulma zamanlamasını tolere etmek için kısa bir bekleme.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    sdk.send(Message::Binary(audio_frame(&[1, 2, 3]).into())).await.expect("send binary");

    let sdk_subtitle = next_json(&mut sdk).await;
    assert_eq!(sdk_subtitle["type"], "subtitle");

    let overlay_subtitle = next_json(&mut overlay_ws).await;
    assert_eq!(overlay_subtitle["type"], "subtitle");
    assert_eq!(overlay_subtitle["source"]["lang"], "tr");
    assert_eq!(overlay_subtitle, sdk_subtitle);
}

#[tokio::test]
async fn overlay_with_unknown_token_receives_nothing_but_connects() {
    let addr = spawn_server().await;
    let overlay_url = format!("ws://{addr}/overlay/no-such-token");
    let (mut overlay_ws, _) = tokio_tungstenite::connect_async(overlay_url).await.expect("connect overlay");

    // Kısa bir süre içinde hiç mesaj gelmemeli (kimse bu token'a yayın yapmıyor).
    let result = tokio::time::timeout(std::time::Duration::from_millis(200), overlay_ws.next()).await;
    assert!(result.is_err(), "unknown token should not receive any message");
}
