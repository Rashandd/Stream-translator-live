//! StreamDubber backend giriş noktası.

use std::process::ExitCode;
use std::sync::Arc;

use streamdubber_backend::{
    app,
    auth::jwt::Jwt,
    config::{Config, ServiceConfig},
    db,
    gateway::{stt::MockStt, translate::MockTranslate, Gateway},
    state::AppState,
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
    let config = Config::from_env()?;

    // DATABASE_URL verilmişse bağlan + migration uygula.
    if let Some(url) = &config.database_url {
        let pool = db::connect(url).await?;
        db::run_migrations(&pool).await?;
        tracing::info!("database migrations applied");
    } else {
        tracing::warn!("DATABASE_URL not set — running without database");
    }

    let state = build_state(&config);

    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "backend listening");

    axum::serve(listener, app::router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// Paylaşılan state'i kurar ve sağlayıcıları kaydeder.
///
/// Faz 1 iskeleti: mock STT/çeviri sağlayıcıları. Gerçek Deepgram (1.7) ve
/// DeepL (1.8) entegrasyonu aynı trait'leri implemente edip burada kaydedilecek.
fn build_state(config: &Config) -> AppState {
    let service_config = Arc::new(ServiceConfig::with_defaults());

    let mut gateway = Gateway::new(Arc::clone(&service_config));
    gateway.register_stt(Arc::new(MockStt::new("deepgram")));
    gateway.register_stt(Arc::new(MockStt::new("assemblyai")));
    gateway.register_translate(Arc::new(MockTranslate::new("deepl")));
    gateway.register_translate(Arc::new(MockTranslate::new("google")));

    AppState {
        jwt: Arc::new(Jwt::new(config.jwt_secret.as_bytes())),
        gateway: Arc::new(gateway),
        service_config,
    }
}

/// Ctrl-C ile düzgün kapanma.
async fn shutdown_signal() {
    if let Err(err) = tokio::signal::ctrl_c().await {
        tracing::error!(error = %err, "failed to listen for shutdown signal");
    }
}
