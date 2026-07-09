// Prevents an additional console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use commands::AppState;
use streamdubber_sdk::core::local_store::LocalTokenStore;
use tauri::Manager;
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

/// Checks the configured updater endpoint once at startup and, if a newer
/// signed build is available, downloads and installs it in the background.
///
/// Deliberately does **not** call `app.restart()` — a broadcaster may be
/// mid-stream when the app launches a previous session's leftover process,
/// or may keep the app running for hours. Forcing a restart would kill an
/// active relay/cloud session; the update takes effect the next time the
/// broadcaster closes and reopens the app themselves (see `docs/sdk-distribution.md`).
async fn check_for_update(app: &tauri::AppHandle) {
    let updater = match app.updater() {
        Ok(updater) => updater,
        Err(err) => {
            tracing::warn!(%err, "updater not available (endpoint/pubkey not configured?)");
            return;
        }
    };

    let update = match updater.check().await {
        Ok(Some(update)) => update,
        Ok(None) => {
            tracing::info!("sdk is up to date");
            return;
        }
        Err(err) => {
            tracing::warn!(%err, "update check failed");
            return;
        }
    };

    tracing::info!(version = %update.version, "sdk update available, downloading");
    match update.download_and_install(|_chunk_len, _total_len| {}, || {}).await {
        Ok(()) => tracing::info!("sdk update installed — takes effect on next restart"),
        Err(err) => tracing::warn!(%err, "sdk update download/install failed"),
    }
}
