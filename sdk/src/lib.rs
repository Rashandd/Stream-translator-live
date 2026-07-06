//! StreamDubber local SDK core.
//!
//! Tauri is the desktop shell. Audio capture, VAD, frame packaging and cloud
//! protocol code live in platform-independent modules unless explicitly placed
//! under `audio/`.

pub mod audio;
pub mod core;
pub mod integrations;

use core::capture::CaptureConfig;
use core::cloud_session::run_cloud_session;
use core::local_stt::LocalSttEngine;
use core::relay_service::{accept_and_relay, bind_ingest_listener, RelayServiceError};
use core::relay_sync::{fetch_relay_targets, RelaySyncError};
use core::stt_control::{run_polling_loop, BenchmarkSample};

/// Parameters the desktop shell needs to start the local relay: which
/// backend to sync targets from, the paired device's JWT, and which local
/// address OBS should publish into.
#[derive(Debug, Clone)]
pub struct DesktopShellConfig {
    pub backend_base_url: String,
    pub device_token: String,
    pub listen_addr: String,
}

/// Local-STT control-plane wiring (phase 2.6b) — `None` until a real
/// [`LocalSttEngine`] exists (whisper.cpp bind, roadmap 2.6b) to inject.
/// The polling loop itself is already implemented and tested against
/// `MockEngine` (see `core::stt_control::run_polling_loop`); passing a mock
/// in here would make the tray report fabricated hardware-test results to
/// real broadcasters, so `main.rs` stays on `None` until the real engine
/// lands.
pub struct SttControlHandle {
    pub engine: Box<dyn LocalSttEngine>,
    pub sample: BenchmarkSample,
    pub poll_interval: std::time::Duration,
}

#[derive(Debug, thiserror::Error)]
pub enum DesktopShellError {
    #[error("failed to sync relay targets: {0}")]
    Sync(#[from] RelaySyncError),
    #[error("relay service error: {0}")]
    Relay(#[from] RelayServiceError),
}

/// Entry point used by the Tauri desktop shell.
///
/// Wires the pieces the roadmap calls "SDK desktop relay wiring": pull the
/// broadcaster's relay targets from the backend, open the localhost RTMP
/// listener OBS publishes into, and hand the accepted connection to the
/// relay engine. No real Tauri window/commands are attached yet — this
/// milestone keeps the shell headless and testable; the GUI layer is a
/// separate follow-up (see roadmap V11 notes).
pub async fn run_desktop_shell(
    cfg: DesktopShellConfig,
    stt_control: Option<SttControlHandle>,
) -> Result<(), DesktopShellError> {
    let http = reqwest::Client::new();
    let targets = fetch_relay_targets(&http, &cfg.backend_base_url, &cfg.device_token).await?;
    tracing::info!(targets = targets.len(), addr = %cfg.listen_addr, "relay targets synced");

    match stt_control {
        Some(handle) => {
            tokio::spawn(run_polling_loop(
                http.clone(),
                cfg.backend_base_url.clone(),
                cfg.device_token.clone(),
                handle.engine,
                handle.sample,
                handle.poll_interval,
            ));
            tracing::info!("local stt control poll loop started");
        }
        None => tracing::info!("local stt control disabled — no local engine configured yet"),
    }

    let listener = bind_ingest_listener(&cfg.listen_addr)
        .await
        .map_err(RelayServiceError::from)?;
    tracing::info!(addr = %cfg.listen_addr, "waiting for OBS to publish");

    let engine = accept_and_relay(&listener, targets).await?;
    tracing::info!("relay engine started");

    // Mic → cloud → subtitles, running alongside the relay. Best-effort: a
    // capture-open failure (e.g. Linux/macOS Phase-1 skeletons) or a session
    // error only logs — it must never take the RTMP relay down with it.
    // Not joined, same fire-and-forget posture as the stt_control loop above:
    // lifecycle sync with OBS connect/disconnect is future work (RelayEngine
    // doesn't expose a "stopped" signal to key off yet).
    {
        let http = http.clone();
        let backend_base_url = cfg.backend_base_url.clone();
        let device_token = cfg.device_token.clone();
        tokio::spawn(async move {
            match audio::open_capture(CaptureConfig::default()) {
                Ok(capture) => {
                    match run_cloud_session(&http, &backend_base_url, &device_token, capture).await {
                        Ok(outcome) => {
                            tracing::info!(subtitles = outcome.subtitles_received, "cloud session ended")
                        }
                        Err(err) => tracing::warn!(%err, "cloud session ended with an error"),
                    }
                }
                Err(err) => {
                    tracing::warn!(%err, "microphone capture unavailable — subtitles/dubbing disabled")
                }
            }
        });
    }

    // Keep the process alive while the relay runs. Stop/restart control is
    // future Tauri command work — this milestone only proves the wiring.
    std::future::pending::<()>().await;
    drop(engine);
    Ok(())
}
