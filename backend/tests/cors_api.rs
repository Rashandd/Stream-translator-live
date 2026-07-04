//! Dashboard CORS uçtan-uca testleri.
//!
//! Frontend (Vercel) ile API (VPS) ayrı origin'lerde — tarayıcı preflight'ı
//! `cors.allowed_origins` config'ine göre yanıtlanır. Sözleşme:
//! - İzinli origin → preflight 200 + `access-control-allow-origin` yankısı.
//! - Listede olmayan origin → CORS başlığı YOK (tarayıcı engeller).
//! - Liste config'ten restart'sız güncellenir.
//! - Ext/waitlist aileleri kendi Any-origin CORS'unu korur (çift header yok).

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use streamdubber_backend::{
    app,
    auth::jwt::Jwt,
    config::ServiceConfig,
    gateway::{stt::MockStt, translate::MockTranslate, Gateway},
    state::AppState,
};

async fn spawn_server() -> (SocketAddr, Arc<ServiceConfig>) {
    let service_config = Arc::new(ServiceConfig::with_defaults());
    let mut gateway = Gateway::new(Arc::clone(&service_config));
    gateway.register_stt(Arc::new(MockStt::new("deepgram")));
    gateway.register_translate(Arc::new(MockTranslate::new("deepl")));

    let state = AppState::skeleton(
        Jwt::new(b"cors-test-secret"),
        Arc::clone(&service_config),
        gateway,
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        axum::serve(listener, app::router(state)).await.expect("serve");
    });
    (addr, service_config)
}

async fn raw(addr: SocketAddr, request: &str) -> String {
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    stream.write_all(request.as_bytes()).await.expect("write");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.expect("read");
    String::from_utf8_lossy(&buf).to_lowercase()
}

fn preflight(path: &str, origin: &str) -> String {
    format!(
        "OPTIONS {path} HTTP/1.1\r\nHost: t\r\nOrigin: {origin}\r\nAccess-Control-Request-Method: POST\r\nAccess-Control-Request-Headers: authorization,content-type\r\nConnection: close\r\n\r\n"
    )
}

#[tokio::test]
async fn allowed_origin_passes_preflight_on_dashboard_routes() {
    let (addr, _cfg) = spawn_server().await;

    let response = raw(addr, &preflight("/api/auth/login", "http://localhost:3000")).await;
    assert!(
        response.contains("access-control-allow-origin: http://localhost:3000"),
        "response: {response}"
    );
    assert!(response.contains("access-control-allow-methods"), "response: {response}");

    // Admin ailesi de aynı katmanın arkasında.
    let response = raw(addr, &preflight("/admin/secrets", "http://localhost:3000")).await;
    assert!(
        response.contains("access-control-allow-origin: http://localhost:3000"),
        "response: {response}"
    );
}

#[tokio::test]
async fn disallowed_origin_gets_no_cors_headers() {
    let (addr, _cfg) = spawn_server().await;
    let response = raw(addr, &preflight("/api/auth/login", "https://evil.example")).await;
    assert!(
        !response.contains("access-control-allow-origin"),
        "unexpected CORS grant: {response}"
    );
}

#[tokio::test]
async fn origins_update_from_config_without_restart() {
    let (addr, cfg) = spawn_server().await;

    let response = raw(addr, &preflight("/api/auth/login", "https://strovak.com")).await;
    assert!(!response.contains("access-control-allow-origin"), "response: {response}");

    cfg.set(
        "cors.allowed_origins",
        "http://localhost:3000,https://strovak.com",
    );

    let response = raw(addr, &preflight("/api/auth/login", "https://strovak.com")).await;
    assert!(
        response.contains("access-control-allow-origin: https://strovak.com"),
        "response: {response}"
    );
}

#[tokio::test]
async fn waitlist_keeps_any_origin_without_duplicate_headers() {
    let (addr, _cfg) = spawn_server().await;
    let response = raw(
        addr,
        "OPTIONS /api/waitlist HTTP/1.1\r\nHost: t\r\nOrigin: https://random.example\r\nAccess-Control-Request-Method: POST\r\nAccess-Control-Request-Headers: content-type\r\nConnection: close\r\n\r\n",
    )
    .await;
    assert!(
        response.contains("access-control-allow-origin: *"),
        "response: {response}"
    );
    // Çift CORS katmanı yok — header tam bir kez.
    assert_eq!(
        response.matches("access-control-allow-origin").count(),
        1,
        "duplicate CORS headers: {response}"
    );
}
