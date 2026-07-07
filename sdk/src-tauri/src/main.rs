// Prevents an additional console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use commands::AppState;
use streamdubber_sdk::core::local_store::LocalTokenStore;
use tauri::Manager;

/// Real Tauri desktop shell entry point. `sdk/ui` shows a pairing form until
/// a device token is persisted (see `core::local_store`), then a status view;
/// `run_desktop_shell` itself is unchanged — this just gives it a UI and a
/// place to get its config from instead of env vars.
fn main() {
    tracing_subscriber::fmt::init();

    tauri::Builder::default()
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
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running StreamDubber SDK");
}
