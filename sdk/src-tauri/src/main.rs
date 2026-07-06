use streamdubber_sdk::DesktopShellConfig;

/// Headless entry point (no real Tauri window yet — see `run_desktop_shell`
/// doc comment). Reads what a future Tauri command/UI would otherwise pass
/// in as parameters from env vars so the binary is runnable standalone.
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cfg = DesktopShellConfig {
        backend_base_url: std::env::var("STREAMDUBBER_BACKEND_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string()),
        device_token: std::env::var("STREAMDUBBER_DEVICE_TOKEN").unwrap_or_default(),
        listen_addr: std::env::var("STREAMDUBBER_RELAY_LISTEN_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:19350".to_string()),
    };

    // stt_control: None — whisper.cpp bind ("2.6b" roadmap kalanı) henüz
    // yok; poll loop mekaniği hazır ve test edilmiş (core::stt_control::
    // run_polling_loop), gerçek motor lanınca burada Some(...) olur.
    if let Err(err) = streamdubber_sdk::run_desktop_shell(cfg, None).await {
        tracing::error!(%err, "desktop shell exited with error");
        std::process::exit(1);
    }
}
