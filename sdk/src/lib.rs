//! StreamDubber local SDK core.
//!
//! Tauri is the desktop shell. Audio capture, VAD, frame packaging and cloud
//! protocol code live in platform-independent modules unless explicitly placed
//! under `audio/`.

pub mod audio;
pub mod core;
pub mod integrations;

/// Entry point used by the Tauri desktop shell.
///
/// The first SDK milestone only wires the Rust architecture. The real Tauri
/// runtime and UI commands will be attached after the core/audio boundaries are
/// stable.
pub fn run_desktop_shell() {}
