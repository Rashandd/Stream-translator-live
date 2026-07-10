// Prevents an additional console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use commands::AppState;
use streamdubber_sdk::core::local_store::LocalTokenStore;
use tauri::{Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

/// Real Tauri desktop shell entry point. `sdk/ui` shows a pairing form until
/// a device token is persisted (see `core::local_store`), then a status view;
/// `run_desktop_shell` itself is unchanged — this just gives it a UI and a
/// place to get its config from instead of env vars.
fn main() {
    tracing_subscriber::fmt::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::submit_pairing_code,
            commands::unpair,
            commands::restart_app,
        ])
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir().join("streamdubber-sdk"));
            let state = AppState::new(LocalTokenStore::at(data_dir));
            state.resume_if_paired(app.handle().clone());
            app.manage(state);

            let updater_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                check_for_update(&updater_handle).await;
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running StreamDubber SDK");
}

/// `sdk/ui` renders one of these as a small banner (`update-status` event).
/// Mirrors `commands::StatusDto`'s "DTO the frontend listens for" pattern.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum UpdateStatus {
    Checking,
    UpToDate,
    Available { version: String },
    Downloading,
    Ready,
    Failed { message: String },
}

/// Pure mapping from a `check()` outcome to the status the UI shows — kept
/// free of `tauri_plugin_updater` types so it can be unit tested without a
/// running `AppHandle`. `Ok(Some(version))` mirrors "an update exists"
/// collapsed down to the one field the UI needs.
fn classify_check_result(result: Result<Option<String>, String>) -> UpdateStatus {
    match result {
        Ok(Some(version)) => UpdateStatus::Available { version },
        Ok(None) => UpdateStatus::UpToDate,
        Err(message) => UpdateStatus::Failed { message },
    }
}

/// Checks the configured updater endpoint once at startup and, if a newer
/// signed build is available, downloads it in the background and reports
/// progress via `update-status` events (`sdk/ui/main.js` renders a banner).
///
/// Deliberately does **not** call `app.restart()` itself — a broadcaster may
/// be mid-stream when the app launches a previous session's leftover
/// process, or may keep the app running for hours. Forcing a restart would
/// kill an active relay/cloud session; once `Ready` is reported the UI offers
/// a "Restart now" button (`commands::restart_app`) so the broadcaster picks
/// the moment (see `docs/sdk-distribution.md`).
async fn check_for_update(app: &tauri::AppHandle) {
    let _ = app.emit("update-status", UpdateStatus::Checking);

    let updater = match app.updater() {
        Ok(updater) => updater,
        Err(err) => {
            // pubkey boşken (bkz. C1 — anahtar üretimi henüz yapılmadı) bu
            // beklenen bir durum; sessizce vazgeç, UI'da hata gösterme.
            tracing::warn!(%err, "updater not available (endpoint/pubkey not configured?)");
            return;
        }
    };

    let check_result = updater.check().await;
    let status = classify_check_result(
        check_result
            .as_ref()
            .map(|opt| opt.as_ref().map(|u| u.version.clone()))
            .map_err(|e| e.to_string()),
    );
    let _ = app.emit("update-status", status.clone());

    let Ok(Some(update)) = check_result else {
        if let Err(err) = check_result {
            tracing::warn!(%err, "update check failed");
        } else {
            tracing::info!("sdk is up to date");
        }
        return;
    };

    tracing::info!(version = %update.version, "sdk update available, downloading");
    let _ = app.emit("update-status", UpdateStatus::Downloading);
    match update.download_and_install(|_chunk_len, _total_len| {}, || {}).await {
        Ok(()) => {
            tracing::info!("sdk update installed — takes effect on restart");
            let _ = app.emit("update-status", UpdateStatus::Ready);
        }
        Err(err) => {
            tracing::warn!(%err, "sdk update download/install failed");
            let _ = app.emit("update-status", UpdateStatus::Failed { message: err.to_string() });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_check_result_maps_each_outcome() {
        assert_eq!(classify_check_result(Ok(None)), UpdateStatus::UpToDate);
        assert_eq!(
            classify_check_result(Ok(Some("1.2.3".to_string()))),
            UpdateStatus::Available { version: "1.2.3".to_string() }
        );
        assert_eq!(
            classify_check_result(Err("network error".to_string())),
            UpdateStatus::Failed { message: "network error".to_string() }
        );
    }
}
