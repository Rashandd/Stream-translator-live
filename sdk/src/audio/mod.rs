//! Platform-specific audio capture boundary.
//!
//! `cfg(target_os)` is intentionally kept inside this module tree.

use crate::core::capture::{CaptureConfig, CaptureError, CaptureSource};

#[cfg(target_os = "macos")]
pub mod coreaudio;
#[cfg(target_os = "linux")]
pub mod pipewire;
#[cfg(target_os = "windows")]
pub mod wasapi;

/// Thin descriptor returned by platform audio adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioAdapterInfo {
    pub backend: AudioBackend,
    pub enabled: bool,
}

/// Supported platform audio backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioBackend {
    Wasapi,
    CoreAudio,
    PipeWire,
    Unsupported,
}

/// Returns the audio backend selected for the current build target.
pub fn platform_backend() -> AudioAdapterInfo {
    AudioAdapterInfo {
        backend: selected_backend(),
        enabled: matches!(selected_backend(), AudioBackend::Wasapi),
    }
}

#[cfg(target_os = "windows")]
fn selected_backend() -> AudioBackend {
    AudioBackend::Wasapi
}

#[cfg(target_os = "macos")]
fn selected_backend() -> AudioBackend {
    AudioBackend::CoreAudio
}

#[cfg(target_os = "linux")]
fn selected_backend() -> AudioBackend {
    AudioBackend::PipeWire
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn selected_backend() -> AudioBackend {
    AudioBackend::Unsupported
}

/// Opens a platform capture source for the current build target.
///
/// Windows (WASAPI) is the primary target; macOS/Linux adapters are disabled
/// skeletons in Phase 1 and return [`CaptureError::NotImplemented`]. The actual
/// device binding is the only platform-specific part — everything downstream
/// (VAD, framing, cloud transport) is platform-independent core.
pub fn open_capture(config: CaptureConfig) -> Result<Box<dyn CaptureSource>, CaptureError> {
    #[cfg(target_os = "windows")]
    {
        wasapi::open(config)
    }
    #[cfg(target_os = "macos")]
    {
        coreaudio::open(config)
    }
    #[cfg(target_os = "linux")]
    {
        pipewire::open(config)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = config;
        Err(CaptureError::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_backend_is_known_for_supported_targets() {
        let info = platform_backend();

        assert!(matches!(
            info.backend,
            AudioBackend::Wasapi
                | AudioBackend::CoreAudio
                | AudioBackend::PipeWire
                | AudioBackend::Unsupported
        ));
    }
}
