//! Linux PipeWire capture adapter skeleton.

use super::{AudioAdapterInfo, AudioBackend};
use crate::core::capture::{CaptureConfig, CaptureError, CaptureSource};

pub fn adapter_info() -> AudioAdapterInfo {
    AudioAdapterInfo {
        backend: AudioBackend::PipeWire,
        enabled: false,
    }
}

/// Skeleton — disabled in Phase 1, activated in Phase 4 (Linux).
pub fn open(_config: CaptureConfig) -> Result<Box<dyn CaptureSource>, CaptureError> {
    Err(CaptureError::NotImplemented(
        "PipeWire adapter is a Phase 1 skeleton (Linux activation in Phase 4)",
    ))
}
