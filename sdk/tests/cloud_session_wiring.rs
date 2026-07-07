//! End-to-end: SDK `cloud_session` wiring ↔ real backend.
//!
//! Drives the piece `relay_wiring.rs`/`cloud_e2e.rs` didn't cover: fetching
//! the broadcaster's saved language settings, then actually pumping mic
//! audio (via `MockCapture`) through VAD/framing/`CloudClient` end-to-end and
//! reading subtitles back — the loop `run_desktop_shell` now spawns.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use streamdubber_backend::{
    app,
    auth::jwt::{Claims, Jwt},
    config::ServiceConfig,
    gateway::{stt::MockStt, translate::MockTranslate, tts::MockTts, Gateway},
    state::AppState,
    voice_models::{LanguageMode, VoiceModel, VoiceModelStatus, VoiceMode, VoiceSource},
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

/// Same as `spawn_backend`, plus a registered mock TTS provider and an
/// approved+active voice model for `user_id`/`lang` — the minimum needed for
/// `pipeline::StreamSession::subtitle_for` to actually populate
/// `Subtitle.dubbed_audio` (see `pipeline::tests::voice_models_with_active`
/// for the same setup, backend-side).
async fn spawn_backend_with_active_voice_model(user_id: &str, lang: &str) -> SocketAddr {
    let service_config = Arc::new(ServiceConfig::with_defaults());
    service_config.set("tts.provider", "elevenlabs");
    let mut gateway = Gateway::new(Arc::clone(&service_config));
    gateway.register_stt(Arc::new(MockStt::new("deepgram")));
    gateway.register_translate(Arc::new(MockTranslate::new("deepl")));
    gateway.register_tts(Arc::new(MockTts::new("elevenlabs")));

    let state = AppState::skeleton(Jwt::new(SECRET), Arc::clone(&service_config), gateway);
    state.voice_models.insert(VoiceModel {
        id: "m1".to_string(),
        user_id: user_id.to_string(),
        name: "test voice".to_string(),
        provider_voice_id: "voice-abc".to_string(),
        status: VoiceModelStatus::PendingReview,
        created_at: 0,
        language_mode: LanguageMode::SingleToMulti,
        source_languages: vec!["tr".to_string()],
        target_languages: Vec::new(),
        mode: VoiceMode::Dubbing,
        source: VoiceSource::UserCloned,
        has_stored_sample: false,
    });
    state.voice_models.set_status("m1", VoiceModelStatus::Approved);
    state.voice_models.set_active_for_language("m1", lang).expect("activate model");

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

    let outcome = run_cloud_session(&http, &base_url, &token, Box::new(capture), HashMap::new())
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

    let err = run_cloud_session(&http, &base_url, &token, Box::new(capture), HashMap::new())
        .await
        .unwrap_err();
    assert!(matches!(err, CloudSessionError::NoTargetLanguages));
}

#[tokio::test]
async fn cloud_session_routes_dub_audio_to_the_matching_language_sink() {
    let user_id = "u-dub-routing";
    let target_lang = "en";
    let addr = spawn_backend_with_active_voice_model(user_id, target_lang).await;
    let base_url = format!("http://{addr}");
    let token = device_token(user_id);
    let http = reqwest::Client::new();

    let put = http
        .put(format!("{base_url}/api/translate/languages"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"source_lang": "tr", "target_langs": [target_lang]}))
        .send()
        .await
        .expect("put languages");
    assert!(put.status().is_success(), "status: {}", put.status());

    let frames = vec![silence(320), loud(320)];
    let capture = MockCapture::new(CaptureConfig::default(), frames);

    let (dub_tx, mut dub_rx) = tokio::sync::mpsc::channel::<Vec<i16>>(4);
    let mut sinks = HashMap::new();
    sinks.insert(target_lang.to_string(), dub_tx);

    let outcome = run_cloud_session(&http, &base_url, &token, Box::new(capture), sinks)
        .await
        .expect("cloud session");
    assert_eq!(outcome.subtitles_received, 1);

    let samples = dub_rx
        .try_recv()
        .expect("dub audio should have been routed to the 'en' sink");
    assert!(!samples.is_empty(), "decoded PCM samples should be non-empty");
}

#[tokio::test]
async fn cloud_session_ignores_dub_audio_for_a_language_with_no_sink() {
    let user_id = "u-dub-no-sink";
    let target_lang = "en";
    let addr = spawn_backend_with_active_voice_model(user_id, target_lang).await;
    let base_url = format!("http://{addr}");
    let token = device_token(user_id);
    let http = reqwest::Client::new();

    let put = http
        .put(format!("{base_url}/api/translate/languages"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"source_lang": "tr", "target_langs": [target_lang]}))
        .send()
        .await
        .expect("put languages");
    assert!(put.status().is_success());

    let frames = vec![silence(320), loud(320)];
    let capture = MockCapture::new(CaptureConfig::default(), frames);

    // No sinks registered at all — dub audio exists (a real active model is
    // configured) but there's nowhere to route it; this must not error.
    let outcome = run_cloud_session(&http, &base_url, &token, Box::new(capture), HashMap::new())
        .await
        .expect("cloud session should not fail just because no dub sink is registered");
    assert_eq!(outcome.subtitles_received, 1);
}
