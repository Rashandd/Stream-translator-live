//! Windows WASAPI capture adapter.

use super::{AudioAdapterInfo, AudioBackend};
use crate::core::capture::{CaptureConfig, CaptureError, CaptureSource};

pub fn adapter_info() -> AudioAdapterInfo {
    AudioAdapterInfo {
        backend: AudioBackend::Wasapi,
        enabled: true,
    }
}

/// Opens a WASAPI capture stream.
///
/// TODO(1.3): bind the WASAPI shared-mode loopback/mic device and resample to
/// 16 kHz mono. Must be implemented and tested on Windows.
pub fn open(_config: CaptureConfig) -> Result<Box<dyn CaptureSource>, CaptureError> {
    Err(CaptureError::NotImplemented(
        "WASAPI device binding pending — implement on Windows",
    ))
}
