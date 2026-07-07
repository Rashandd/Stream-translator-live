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

/// Live status the desktop shell reports back to the Tauri UI layer (or any
/// other caller). `mic_capture_ok` stays `None` until a capture attempt has
/// actually happened — OBS has to connect and publish before the mic-capture
/// task spawns (see the tail of [`run_desktop_shell`]).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct ShellStatus {
    pub targets_synced: usize,
    pub relay_listening: bool,
    pub mic_capture_ok: Option<bool>,
}

/// Applies `f` to a copy of the current status and publishes it, if a
/// watcher is attached. No-op when `status` is `None`, so callers that don't
/// care about live status (tests, the pre-GUI headless binary) pay nothing.
fn update_status(
    status: &Option<tokio::sync::watch::Sender<ShellStatus>>,
    f: impl FnOnce(&mut ShellStatus),
) {
    if let Some(tx) = status {
        let mut next = tx.borrow().clone();
        f(&mut next);
        let _ = tx.send(next);
    }
}

/// Entry point used by the Tauri desktop shell.
///
/// Wires the pieces the roadmap calls "SDK desktop relay wiring": pull the
/// broadcaster's relay targets from the backend, open the localhost RTMP
/// listener OBS publishes into, and hand the accepted connection to the
/// relay engine. `status` is an optional live-status sink (the Tauri layer
/// wires one in to drive the pairing/status UI); it stays `None` in tests
/// and the plain headless path.
pub async fn run_desktop_shell(
    cfg: DesktopShellConfig,
    stt_control: Option<SttControlHandle>,
    status: Option<tokio::sync::watch::Sender<ShellStatus>>,
) -> Result<(), DesktopShellError> {
    let http = reqwest::Client::new();
    let targets = fetch_relay_targets(&http, &cfg.backend_base_url, &cfg.device_token).await?;
    tracing::info!(targets = targets.len(), addr = %cfg.listen_addr, "relay targets synced");
    update_status(&status, |s| s.targets_synced = targets.len());

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
    update_status(&status, |s| s.relay_listening = true);

    let engine = accept_and_relay(&listener, targets).await?;
    tracing::info!("relay engine started");

    // Mic → cloud → subtitles, running alongside the relay. Best-effort: a
    // capture-open failure (e.g. Linux/macOS Phase-1 skeletons) or a session
    // error only logs — it must never take the RTMP relay down with it.
    // Not joined, same fire-and-forget posture as the stt_control loop above:
    // lifecycle sync with OBS connect/disconnect is future work (RelayEngine
    // doesn't expose a "stopped" signal to key off yet).
    //
    // `dub_audio_sinks` is how cloud-synthesized dub audio (task 2.3, device
    // mux) reaches the relay's dub targets: fetched from the engine before
    // it's moved into `std::future::pending` below, and forwarded into the
    // cloud session, which routes each `Subtitle.dubbed_audio` track to the
    // matching language's sink (see `cloud_session::route_dub_audio`).
    let dub_audio_sinks = engine.dub_audio_sinks();
    {
        let http = http.clone();
        let backend_base_url = cfg.backend_base_url.clone();
        let device_token = cfg.device_token.clone();
        let status = status.clone();
        tokio::spawn(async move {
            match audio::open_capture(CaptureConfig::default()) {
                Ok(capture) => {
                    update_status(&status, |s| s.mic_capture_ok = Some(true));
                    match run_cloud_session(
                        &http,
                        &backend_base_url,
                        &device_token,
                        capture,
                        dub_audio_sinks,
                    )
                    .await
                    {
                        Ok(outcome) => {
                            tracing::info!(subtitles = outcome.subtitles_received, "cloud session ended")
                        }
                        Err(err) => tracing::warn!(%err, "cloud session ended with an error"),
                    }
                }
                Err(err) => {
                    update_status(&status, |s| s.mic_capture_ok = Some(false));
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
