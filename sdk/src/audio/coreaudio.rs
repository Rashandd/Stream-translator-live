//! macOS CoreAudio capture adapter skeleton.

use super::{AudioAdapterInfo, AudioBackend};
use crate::core::capture::{CaptureConfig, CaptureError, CaptureSource};

pub fn adapter_info() -> AudioAdapterInfo {
    AudioAdapterInfo {
        backend: AudioBackend::CoreAudio,
        enabled: false,
    }
}

/// Skeleton — disabled in Phase 1, activated in Phase 4 (macOS).
pub fn open(_config: CaptureConfig) -> Result<Box<dyn CaptureSource>, CaptureError> {
    Err(CaptureError::NotImplemented(
        "CoreAudio adapter is a Phase 1 skeleton (macOS activation in Phase 4)",
    ))
}
