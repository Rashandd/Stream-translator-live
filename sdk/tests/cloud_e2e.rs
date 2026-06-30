//! End-to-end: SDK CloudClient ↔ real backend WebSocket server.
//!
//! Spawns the backend in-process and drives the full Phase-1 audio path through
//! the SDK client: connect → auth → stream_start → audio frame → subtitle.

use std::net::SocketAddr;
use std::sync::Arc;

use streamdubber_backend::{
    app,
    auth::jwt::{Claims, Jwt},
    config::ServiceConfig,
    gateway::{stt::MockStt, translate::MockTranslate, Gateway},
    state::AppState,
};
use streamdubber_sdk::core::{
    audio_frame::{build_audio_frame, AudioCodec},
    cloud::{CloudClient, ServerMessage},
    stream::StreamConfig,
};
use tokio::net::TcpListener;

const SECRET: &[u8] = b"sdk-e2e-secret";

async fn spawn_backend() -> SocketAddr {
    let service_config = Arc::new(ServiceConfig::with_defaults());
    let mut gateway = Gateway::new(Arc::clone(&service_config));
    gateway.register_stt(Arc::new(MockStt::new("deepgram")));
    gateway.register_translate(Arc::new(MockTranslate::new("deepl")));

    let state = AppState {
        jwt: Arc::new(Jwt::new(SECRET)),
        gateway: Arc::new(gateway),
        service_config,
    };

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, app::router(state))
            .await
            .expect("serve");
    });
    addr
}

fn token() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs() as i64;
    let jwt = Jwt::new(SECRET);
    jwt.issue(&Claims {
        sub: "u-e2e".into(),
        role: "broadcaster".into(),
        plan: "free".into(),
        exp: now + 3600,
    })
    .expect("issue")
}

#[tokio::test]
async fn sdk_client_runs_full_audio_path() {
    let addr = spawn_backend().await;
    let url = format!("ws://{addr}/ws/stream");

    let mut client = CloudClient::connect(&url).await.expect("connect");

    // auth → session_ready
    let ready = client.authenticate(&token()).await.expect("auth");
    assert!(matches!(ready, ServerMessage::SessionReady { .. }));

    // stream_start → stream_ack
    let cfg = StreamConfig::new(
        "u-e2e",
        "tr",
        vec!["en".to_string(), "de".to_string()],
        AudioCodec::PcmS16Le,
    );
    client.start_stream(&cfg).await.expect("stream_start");

    // audio frame → subtitle
    let frame = build_audio_frame(AudioCodec::PcmS16Le, &[1, 2, 3, 4]).expect("frame");
    client.send_audio(&frame).await.expect("send_audio");

    let msg = client.next_message().await.expect("recv").expect("not closed");
    match msg {
        ServerMessage::Subtitle {
            source,
            translations,
            ..
        } => {
            assert_eq!(source.lang, "tr");
            assert_eq!(translations.len(), 2);
        }
        other => panic!("expected subtitle, got {other:?}"),
    }
}

#[tokio::test]
async fn sdk_client_surfaces_invalid_token() {
    let addr = spawn_backend().await;
    let url = format!("ws://{addr}/ws/stream");
    let mut client = CloudClient::connect(&url).await.expect("connect");

    let err = client.authenticate("garbage").await.unwrap_err();
    // Backend rejects with protocol code 4001.
    assert!(matches!(
        err,
        streamdubber_sdk::core::cloud::CloudError::Server { code: 4001, .. }
    ));
}
