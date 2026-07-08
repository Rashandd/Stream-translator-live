//! End-to-end: SDK relay wiring ↔ real backend (V11 "SDK desktop relay wiring").
//!
//! Spawns the backend in-process, drives the two pieces the roadmap calls
//! out as missing: fetching relay targets over HTTP with a real device JWT,
//! and accepting a real localhost connection into the RTMP relay engine.

use std::net::SocketAddr;
use std::sync::Arc;

use streamdubber_backend::{
    app,
    auth::jwt::{Claims, Jwt},
    config::ServiceConfig,
    gateway::{stt::MockStt, translate::MockTranslate, Gateway},
    state::AppState,
};
use streamdubber_sdk::core::relay_service::{accept_and_relay, bind_ingest_listener};
use streamdubber_sdk::core::relay_sync::fetch_relay_targets;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

const SECRET: &[u8] = b"sdk-relay-wiring-secret";

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
    jwt.issue(&Claims {
        sub: "u-relay-wiring".into(),
        role: "broadcaster".into(),
        plan: "pro".into(),
        exp: now + 3600,
    })
    .expect("issue token")
}

#[tokio::test]
async fn fetch_relay_targets_matches_what_was_put_on_the_backend() {
    let addr = spawn_backend().await;
    let base_url = format!("http://{addr}");
    let token = device_token();
    let http = reqwest::Client::new();

    // Panel tarafı: hedefleri backend'e yazar. (youtube burada YASAK — bkz.
    // backend/src/relay/mod.rs: YouTube native OAuth akışını kullanır, relay
    // hedefi olamaz — bu yüzden test twitch kullanıyor.)
    let put_resp = http
        .put(format!("{base_url}/api/relay/targets"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "targets": [
                {"platform": "twitch", "rtmp_url": "rtmp://a.rtmp.twitch.tv/live2", "stream_key": "tw-secret-key"},
            ]
        }))
        .send()
        .await
        .expect("put targets");
    assert!(put_resp.status().is_success(), "status: {}", put_resp.status());

    // SDK tarafı: aynı cihaz token'ıyla hedefleri çeker.
    let targets = fetch_relay_targets(&http, &base_url, &token).await.expect("fetch");
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].label, "twitch");
    assert_eq!(targets[0].host, "a.rtmp.twitch.tv");
    assert_eq!(targets[0].app, "live2");
    assert_eq!(targets[0].stream_key, "tw-secret-key");
}

#[tokio::test]
async fn fetch_relay_targets_rejects_a_bad_token() {
    let addr = spawn_backend().await;
    let base_url = format!("http://{addr}");
    let http = reqwest::Client::new();

    let err = fetch_relay_targets(&http, &base_url, "garbage-token").await.unwrap_err();
    match err {
        streamdubber_sdk::core::relay_sync::RelaySyncError::Server(status) => {
            assert_eq!(status.as_u16(), 401);
        }
        other => panic!("expected a 401 Server error, got {other:?}"),
    }
}

#[tokio::test]
async fn local_listener_accepts_a_real_connection_and_reaches_the_relay_engine() {
    let listener = bind_ingest_listener("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");

    let obs = tokio::spawn(async move {
        let mut sock = TcpStream::connect(addr).await.expect("connect");
        // Incomplete handshake on purpose (see relay_service unit test for
        // why): proves the real socket reached RelayEngine::start without
        // needing to reimplement the full RTMP publish command sequence.
        sock.write_all(&[3]).await.expect("write c0");
        sock.shutdown().await.expect("shutdown");
    });

    let result = accept_and_relay(&listener, Vec::new()).await;
    obs.await.expect("obs task");
    assert!(result.is_err(), "expected the incomplete handshake to fail past the real listener");
}
