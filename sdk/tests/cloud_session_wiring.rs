//! End-to-end: SDK `cloud_session` wiring ↔ real backend.
//!
//! Drives the piece `relay_wiring.rs`/`cloud_e2e.rs` didn't cover: fetching
//! the broadcaster's saved language settings, then actually pumping mic
//! audio (via `MockCapture`) through VAD/framing/`CloudClient` end-to-end and
//! reading subtitles back — the loop `run_desktop_shell` now spawns.

use std::net::SocketAddr;
use std::sync::Arc;

use streamdubber_backend::{
    app,
    auth::jwt::{Claims, Jwt},
    config::ServiceConfig,
    gateway::{stt::MockStt, translate::MockTranslate, Gateway},
    state::AppState,
};
use streamdubber_sdk::core::capture::{CaptureConfig, MockCapture};
use streamdubber_sdk::core::cloud_session::{run_cloud_session, CloudSessionError};

const SECRET: &[u8] = b"sdk-cloud-session-secret";

async fn spawn_backend() -> SocketAddr {
    let service_config = Arc::new(ServiceConfig::with_defaults());
    let mut gateway = Gateway::new(Arc::clone(&service_config));
    gateway.register_stt(Arc::new(MockStt::new("deepgram")));
    gateway.register_translate(Arc::new(MockTranslate::new("deepl")));

    let state = AppState::skeleton(Jwt::new(SECRET), Arc::clone(&service_config), gateway);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, app::router(state)).await.expect("serve");
    });
    addr
}

fn device_token(sub: &str) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs() as i64;
    let jwt = Jwt::new(SECRET);
    jwt.issue(&Claims {
        sub: sub.to_string(),
        role: "broadcaster".into(),
        plan: "pro".into(),
        exp: now + 3600,
    })
    .expect("issue token")
}

fn silence(n: usize) -> Vec<i16> {
    vec![0; n]
}

fn loud(n: usize) -> Vec<i16> {
    (0..n).map(|i| if i % 2 == 0 { 5000 } else { -5000 }).collect()
}

#[tokio::test]
async fn cloud_session_streams_mic_audio_and_counts_subtitles() {
    let addr = spawn_backend().await;
    let base_url = format!("http://{addr}");
    let token = device_token("u-cloud-session");
    let http = reqwest::Client::new();

    // Panel side: save the language settings the SDK will fetch.
    let put = http
        .put(format!("{base_url}/api/translate/languages"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"source_lang": "tr", "target_langs": ["en"]}))
        .send()
        .await
        .expect("put languages");
    assert!(put.status().is_success(), "status: {}", put.status());

    // Leading silence is dropped by VAD; only the speech frame is sent —
    // no trailing silence, so the hangover window can't send a second frame
    // and make the subtitle count ambiguous.
    let frames = vec![silence(320), loud(320)];
    let capture = MockCapture::new(CaptureConfig::default(), frames);

    let outcome = run_cloud_session(&http, &base_url, &token, Box::new(capture))
        .await
        .expect("cloud session");
    assert_eq!(outcome.subtitles_received, 1);
}

#[tokio::test]
async fn cloud_session_rejects_missing_target_languages() {
    let addr = spawn_backend().await;
    let base_url = format!("http://{addr}");
    let token = device_token("u-no-languages");
    let http = reqwest::Client::new();

    // No PUT — the store defaults to empty target_langs.
    let capture = MockCapture::new(CaptureConfig::default(), vec![loud(320)]);

    let err = run_cloud_session(&http, &base_url, &token, Box::new(capture))
        .await
        .unwrap_err();
    assert!(matches!(err, CloudSessionError::NoTargetLanguages));
}
