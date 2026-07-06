//! End-to-end: SDK STT control-plane ↔ real backend (phase 2.6b).
//!
//! Spawns the backend in-process and drives the full panel→SDK loop:
//! panel queues a benchmark command, the SDK polls it, runs the (mock)
//! engine against server-policy thresholds, reports the result, and the
//! policy endpoint reflects it — which is exactly what unlocks the
//! "On your device" mode in the dashboard card.

use std::net::SocketAddr;
use std::sync::Arc;

use streamdubber_backend::{
    app,
    auth::jwt::{Claims, Jwt},
    config::ServiceConfig,
    gateway::{stt::MockStt, translate::MockTranslate, Gateway},
    state::AppState,
};
use streamdubber_sdk::core::local_stt::MockEngine;
use streamdubber_sdk::core::stt_control::{
    fetch_policy, handle_pending_commands, poll_commands, BenchmarkSample, HardwareInfo, SttMode,
};

const SECRET: &[u8] = b"sdk-stt-control-wiring-secret";

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

fn device_token() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs() as i64;
    let jwt = Jwt::new(SECRET);
    let claims = Claims {
        sub: "11111111-2222-3333-4444-555555555555".into(),
        role: "broadcaster".into(),
        plan: "free".into(),
        exp: now + 3600,
    };
    jwt.issue(&claims).expect("issue token")
}

fn sample() -> BenchmarkSample {
    // 3 × 1 sn sessiz segment (16 kHz mono) — mock motor için yeterli.
    BenchmarkSample {
        segments: vec![vec![0; 16_000], vec![0; 16_000], vec![0; 16_000]],
        sample_rate_hz: 16_000,
    }
}

#[tokio::test]
async fn full_panel_to_sdk_benchmark_loop() {
    let addr = spawn_backend().await;
    let base = format!("http://{addr}");
    let token = device_token();
    let http = reqwest::Client::new();

    // Başlangıç: policy'de benchmark yok, mod cloud.
    let policy = fetch_policy(&http, &base, &token).await.expect("policy");
    assert!(policy.local_allowed);
    assert_eq!(policy.mode, SttMode::Cloud);
    assert!(policy.benchmark.is_none());

    // Panel benchmark komutu bırakır (dashboard "Test my hardware" butonu).
    let res = http
        .post(format!("{base}/api/sdk/commands"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"command": "benchmark"}))
        .send()
        .await
        .expect("push command");
    assert_eq!(res.status(), 202);

    // SDK yoklar, mock motorla koşar, raporlar.
    let mut engine = MockEngine::instant("merhaba dünya");
    let handled = handle_pending_commands(
        &http,
        &base,
        &token,
        &mut engine,
        &sample(),
        &HardwareInfo {
            gpu_name: Some("RTX 3060".into()),
            vram_mb: Some(12_288),
        },
    )
    .await
    .expect("handle commands");
    assert_eq!(handled, 1);

    // Policy artık geçen benchmark'ı yansıtır — panel kartı kilidi açar.
    let policy = fetch_policy(&http, &base, &token).await.expect("policy");
    let benchmark = policy.benchmark.expect("benchmark stored");
    assert!(benchmark.passed, "mock motor policy eşiklerini geçmeli");
    assert_eq!(benchmark.gpu_name.as_deref(), Some("RTX 3060"));
    assert!(benchmark.reported_at > 0);

    // Kuyruk tek seferlik: ikinci tur işlenecek komut bulamaz.
    let handled = handle_pending_commands(
        &http,
        &base,
        &token,
        &mut engine,
        &sample(),
        &HardwareInfo::default(),
    )
    .await
    .expect("second poll");
    assert_eq!(handled, 0);
    assert!(poll_commands(&http, &base, &token).await.expect("poll").is_empty());
}

#[tokio::test]
async fn bad_token_is_rejected() {
    let addr = spawn_backend().await;
    let base = format!("http://{addr}");
    let http = reqwest::Client::new();

    let err = fetch_policy(&http, &base, "garbage-token").await.unwrap_err();
    assert!(err.to_string().contains("401"), "beklenen 401, alınan: {err}");
}
