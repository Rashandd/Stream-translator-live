//! StreamDubber backend giriş noktası.

use std::process::ExitCode;

use streamdubber_backend::{app, config::Config};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!(error = %err, "fatal startup error");
            ExitCode::FAILURE
        }
    }
}

/// Konfigürasyonu yükler, dinleyiciyi bağlar ve sunucuyu çalıştırır.
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "backend listening");

    axum::serve(listener, app::router())
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// Ctrl-C ile düzgün kapanma.
async fn shutdown_signal() {
    if let Err(err) = tokio::signal::ctrl_c().await {
        tracing::error!(error = %err, "failed to listen for shutdown signal");
    }
}
