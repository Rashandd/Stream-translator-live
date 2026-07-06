//! End-to-end: periodic STT-control poll loop (phase 2.6b — tray wiring).
//!
//! `run_polling_loop` is what the desktop shell spawns once a real
//! `LocalSttEngine` exists. `stt_control_wiring.rs` already covers a single
//! manual `handle_pending_commands` call; this proves the *loop* actually
//! ticks repeatedly against a live backend — a command queued after the
//! loop has already started still gets picked up on a later tick — and that
//! a backend that's unreachable on the first tick doesn't kill the task.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use streamdubber_backend::{
    app,
    auth::jwt::{Claims, Jwt},
    config::ServiceConfig,
    gateway::{stt::MockStt, translate::MockTranslate, Gateway},
    state::AppState,
};
use streamdubber_sdk::core::local_stt::MockEngine;
use streamdubber_sdk::core::stt_control::{fetch_policy, run_polling_loop, BenchmarkSample};

const SECRET: &[u8] = b"sdk-stt-control-loop-secret";

async fn spawn_backend() -> SocketAddr {
    let service_config = Arc::new(ServiceConfig::with_defaults());
    let mut gateway = Gateway::new(Arc::clone(&service_config));
    gateway.register_stt(Arc::new(MockStt::new("deepgram")));
    gateway.register_translate(Arc::new(MockTranslate::new("deepl")));

    let state = AppState::skeleton(Jwt::new(SECRET), Arc::clone(&service_config), gateway);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
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
        sub: "22222222-3333-4444-5555-666666666666".into(),
        role: "broadcaster".into(),
        plan: "free".into(),
        exp: now + 3600,
    };
    jwt.issue(&claims).expect("issue token")
}

fn sample() -> BenchmarkSample {
    BenchmarkSample {
        segments: vec![vec![0; 16_000]],
        sample_rate_hz: 16_000,
    }
}

#[tokio::test]
async fn loop_picks_up_a_command_queued_after_it_starts() {
    let addr = spawn_backend().await;
    let base = format!("http://{addr}");
    let token = device_token();
    let http = reqwest::Client::new();

    let handle = tokio::spawn(run_polling_loop(
        http.clone(),
        base.clone(),
        token.clone(),
        Box::new(MockEngine::instant("merhaba")),
        sample(),
        Duration::from_millis(20),
    ));

    // Loop çoktan birkaç tur atmış olmalı — henüz işlenecek komut yok.
    tokio::time::sleep(Duration::from_millis(60)).await;

    // Panel komutu şimdi bırakıyor — loop zaten çalışırken.
    let res = http
        .post(format!("{base}/api/sdk/commands"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"command": "benchmark"}))
        .send()
        .await
        .expect("push command");
    assert_eq!(res.status(), 202);

    // Sonraki tick'lerden birinde işlenmeli.
    tokio::time::sleep(Duration::from_millis(200)).await;
    handle.abort();

    let policy = fetch_policy(&http, &base, &token).await.expect("policy");
    let benchmark = policy
        .benchmark
        .expect("loop bir sonraki tick'te komutu işlemeliydi");
    assert!(benchmark.passed);
}

#[tokio::test]
async fn loop_survives_backend_being_unreachable() {
    let http = reqwest::Client::new();
    // 127.0.0.1:1 hiçbir şey dinlemiyor — bağlantı hemen reddedilir.
    let handle = tokio::spawn(run_polling_loop(
        http,
        "http://127.0.0.1:1".to_string(),
        "irrelevant-token".to_string(),
        Box::new(MockEngine::instant("x")),
        sample(),
        Duration::from_millis(10),
    ));

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(!handle.is_finished(), "bağlantı hatası loop'u öldürmemeli");
    handle.abort();
}
