//! Tauri commands backing `sdk/ui` — pairing, live status, unpair.
//!
//! Pairing reuses the backend endpoint that already exists for this exact
//! purpose (`POST /api/pairing/claim`, see `backend/src/pairing/mod.rs`) —
//! no new backend surface needed here, only a caller for it.

use std::sync::atomic::{AtomicBool, Ordering};

use streamdubber_sdk::core::local_store::{LocalSession, LocalTokenStore};
use streamdubber_sdk::core::pairing::{PairingCode, PairingRequest};
use streamdubber_sdk::{DesktopShellConfig, ShellStatus};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::watch;

/// Local OBS-facing RTMP listener address. Not exposed in the UI yet — the
/// happy path never needs it changed; power users can still override via
/// `STREAMDUBBER_RELAY_LISTEN_ADDR` if `start()` picks it up in a future pass.
const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:19350";

/// Snapshot the frontend renders — pairing state plus whatever the running
/// desktop shell reports about itself.
#[derive(Debug, Clone, serde::Serialize)]
pub struct StatusDto {
    pub paired: bool,
    pub backend_url: Option<String>,
    pub relay_listening: bool,
    pub mic_capture_ok: Option<bool>,
    pub targets_synced: usize,
}

/// Managed Tauri state: the encrypted local session store plus the live
/// status of the (at most one) running desktop shell.
pub struct AppState {
    store: LocalTokenStore,
    status_tx: watch::Sender<ShellStatus>,
    started: AtomicBool,
}

impl AppState {
    pub fn new(store: LocalTokenStore) -> Self {
        let (status_tx, _rx) = watch::channel(ShellStatus::default());
        AppState {
            store,
            status_tx,
            started: AtomicBool::new(false),
        }
    }

    /// Resumes a previously paired session on launch, if one is persisted —
    /// device tokens last 30 days, so broadcasters shouldn't have to re-pair
    /// every time they open the app.
    pub fn resume_if_paired(&self, app: AppHandle) {
        if let Some(session) = self.store.load() {
            self.start(app, session);
        }
    }

    /// Spawns `run_desktop_shell` in the background and forwards its status
    /// to the frontend as `status-changed` events. Idempotent: at most one
    /// shell runs per process (a second pairing while already running is a
    /// no-op here — full restart-on-repair is future work, same limitation
    /// `run_desktop_shell` itself already documents for stop/restart).
    fn start(&self, app: AppHandle, session: LocalSession) {
        if self.started.swap(true, Ordering::SeqCst) {
            return;
        }

        let mut status_rx = self.status_tx.subscribe();
        let event_backend_url = session.backend_url.clone();
        tauri::async_runtime::spawn(async move {
            while status_rx.changed().await.is_ok() {
                let shell = status_rx.borrow_and_update().clone();
                let dto = StatusDto {
                    paired: true,
                    backend_url: Some(event_backend_url.clone()),
                    relay_listening: shell.relay_listening,
                    mic_capture_ok: shell.mic_capture_ok,
                    targets_synced: shell.targets_synced,
                };
                let _ = app.emit("status-changed", dto);
            }
        });

        let cfg = DesktopShellConfig {
            backend_base_url: session.backend_url,
            device_token: session.token,
            listen_addr: DEFAULT_LISTEN_ADDR.to_string(),
            sdk_session_id: session.sdk_session_id,
            heartbeat_secret: session.heartbeat_secret,
        };
        let status_tx = self.status_tx.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(err) = streamdubber_sdk::run_desktop_shell(cfg, None, Some(status_tx)).await
            {
                tracing::error!(%err, "desktop shell exited with error");
            }
        });
    }
}

/// `get_status` — combines pairing state (from disk) with the running
/// shell's live status (from the watch channel).
#[tauri::command]
pub fn get_status(state: State<AppState>) -> StatusDto {
    let session = state.store.load();
    let shell = state.status_tx.borrow().clone();
    StatusDto {
        paired: session.is_some(),
        backend_url: session.map(|s| s.backend_url),
        relay_listening: shell.relay_listening,
        mic_capture_ok: shell.mic_capture_ok,
        targets_synced: shell.targets_synced,
    }
}

/// `submit_pairing_code` — validates the code, redeems it against the
/// existing `/api/pairing/claim` endpoint, persists the resulting device
/// token encrypted, and starts the desktop shell.
#[tauri::command]
pub async fn submit_pairing_code(
    app: AppHandle,
    state: State<'_, AppState>,
    backend_url: String,
    code: String,
    device_name: String,
) -> Result<(), String> {
    let parsed = PairingCode::parse(&code).map_err(|e| e.to_string())?;
    let backend_url = backend_url.trim().trim_end_matches('/').to_string();
    if backend_url.is_empty() {
        return Err("backend URL is required".to_string());
    }
    let request = PairingRequest::new(&parsed, device_name);

    let http = reqwest::Client::new();
    let response = http
        .post(format!("{backend_url}/api/pairing/claim"))
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("could not reach the backend: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("pairing failed ({})", response.status()));
    }

    #[derive(serde::Deserialize)]
    struct ClaimResponse {
        token: String,
        sdk_session_id: String,
        heartbeat_secret: String,
    }
    let claimed: ClaimResponse = response
        .json()
        .await
        .map_err(|e| format!("unexpected response from backend: {e}"))?;

    let session = LocalSession {
        backend_url,
        token: claimed.token,
        sdk_session_id: claimed.sdk_session_id,
        heartbeat_secret: claimed.heartbeat_secret,
    };
    state.store.save(&session).map_err(|e| e.to_string())?;

    state.start(app, session);
    Ok(())
}

/// `unpair` — clears the persisted session so the UI falls back to the
/// pairing screen. Does not stop an already-running relay/cloud session in
/// this process (same documented limitation as `run_desktop_shell`'s
/// stop/restart control) — takes full effect on next launch.
#[tauri::command]
pub fn unpair(state: State<AppState>) -> Result<(), String> {
    state.store.clear().map_err(|e| e.to_string())
}
