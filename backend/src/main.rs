//! StreamDubber backend giriş noktası.

use std::net::SocketAddr;
use std::process::ExitCode;
use std::sync::Arc;

use axum::Router;
use streamdubber_backend::{
    app,
    auth::jwt::Jwt,
    config::{Config, ServiceConfig},
    db,
    gateway::{stt::MockStt, translate::MockTranslate, Gateway},
    secrets::{names, EnvSecretStore, SecretStore},
    state::AppState,
    tls::{self, TlsSettings},
};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!(error = %err, "fatal startup error");
            ExitCode::FAILURE
        }
    }
}

/// Konfigürasyonu yükler, state'i kurar, dinleyiciyi bağlar ve sunucuyu çalıştırır.
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Container healthcheck modu: /health'e bağlan, çık. (distroless'ta curl yok.)
    if std::env::args().any(|a| a == "--health-check") {
        return health_check();
    }

    let config = Config::from_env()?;

    // Secret kaynağı — Faz 1: env. Üretim: Vault (infra sağlanınca).
    let secrets = EnvSecretStore;

    // DATABASE_URL verilmişse bağlan + migration uygula.
    if let Some(url) = &config.database_url {
        let pool = db::connect(url).await?;
        db::run_migrations(&pool).await?;
        tracing::info!("database migrations applied");
    } else {
        tracing::warn!("DATABASE_URL not set — running without database");
    }

    let state = build_state(&secrets).await?;
    let router = app::router(state);

    match TlsSettings::from_env() {
        Some(settings) => serve_tls(config.bind_addr, router, settings).await,
        None => {
            tracing::warn!("TLS not configured — serving plain HTTP (expect TLS-terminating proxy)");
            serve_plain(config.bind_addr, router).await
        }
    }
}

/// Paylaşılan state'i kurar ve sağlayıcıları kaydeder.
///
/// JWT secret ve sağlayıcı key'leri [`SecretStore`]'dan çekilir — koda gömülmez.
/// Faz 1 iskeleti: mock STT/çeviri. Gerçek Deepgram (1.7) ve DeepL (1.8)
/// entegrasyonu aynı trait'leri implemente edip burada kaydedilecek.
async fn build_state(secrets: &dyn SecretStore) -> Result<AppState, Box<dyn std::error::Error>> {
    let jwt_secret = secrets.get(names::JWT_SECRET).await?;

    let service_config = Arc::new(ServiceConfig::with_defaults());

    let mut gateway = Gateway::new(Arc::clone(&service_config));
    gateway.register_stt(Arc::new(MockStt::new("deepgram")));
    gateway.register_stt(Arc::new(MockStt::new("assemblyai")));
    gateway.register_translate(Arc::new(MockTranslate::new("deepl")));
    gateway.register_translate(Arc::new(MockTranslate::new("google")));

    Ok(AppState {
        jwt: Arc::new(Jwt::new(jwt_secret.as_bytes())),
        gateway: Arc::new(gateway),
        service_config,
    })
}

/// Düz HTTP servis (TLS terminasyonu dışarıda).
async fn serve_plain(addr: SocketAddr, router: Router) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "backend listening (plain)");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// TLS 1.3 (+ opsiyonel mTLS) servis.
async fn serve_tls(
    addr: SocketAddr,
    router: Router,
    settings: TlsSettings,
) -> Result<(), Box<dyn std::error::Error>> {
    let server_config = tls::build_server_config(&settings)?;
    let rustls_config = axum_server::tls_rustls::RustlsConfig::from_config(server_config);

    let handle = axum_server::Handle::new();
    let shutdown_handle = handle.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        shutdown_handle.graceful_shutdown(Some(std::time::Duration::from_secs(5)));
    });

    tracing::info!(%addr, mtls = settings.mutual(), "backend listening (TLS 1.3)");
    axum_server::bind_rustls(addr, rustls_config)
        .handle(handle)
        .serve(router.into_make_service())
        .await?;
    Ok(())
}

/// Container healthcheck — `/health`'e basit bir HTTP GET atıp `200`'ü doğrular.
/// std-only (ek bağımlılık yok), distroless runtime'da çalışır.
fn health_check() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    let addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let port = addr.rsplit(':').next().unwrap_or("8080");
    let target = format!("127.0.0.1:{port}");

    let mut stream = TcpStream::connect(&target)?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    stream.write_all(b"GET /health HTTP/1.0\r\nHost: localhost\r\n\r\n")?;

    let mut response = String::new();
    stream.read_to_string(&mut response)?;

    if response.lines().next().is_some_and(|line| line.contains("200")) {
        Ok(())
    } else {
        Err(format!("health check failed: {:?}", response.lines().next()).into())
    }
}

/// Ctrl-C ile düzgün kapanma.
async fn shutdown_signal() {
    if let Err(err) = tokio::signal::ctrl_c().await {
        tracing::error!(error = %err, "failed to listen for shutdown signal");
    }
}
