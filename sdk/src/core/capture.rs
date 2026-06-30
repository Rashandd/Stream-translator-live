//! Platform-independent audio capture abstraction.
//!
//! The real device I/O lives under `audio/` (WASAPI/CoreAudio/PipeWire) behind
//! `cfg(target_os)`. This module defines the platform-neutral contract those
//! adapters fulfil, plus a [`MockCapture`] used by tests and demos.
//!
//! Phase 1 captures 16 kHz mono PCM (s16le) — enough for STT, 48 kHz is wasteful.

use super::stream::{CHANNELS, SAMPLE_RATE_HZ};

/// Default capture frame size in samples (20 ms at 16 kHz mono).
pub const DEFAULT_FRAME_SAMPLES: usize = 320;

/// Capture configuration handed to a platform adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureConfig {
    pub sample_rate: u32,
    pub channels: u8,
    /// Samples per captured frame (per channel).
    pub frame_samples: usize,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        CaptureConfig {
            sample_rate: SAMPLE_RATE_HZ,
            channels: CHANNELS,
            frame_samples: DEFAULT_FRAME_SAMPLES,
        }
    }
}

/// Audio capture errors.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CaptureError {
    /// Adapter exists for this platform but device binding is not written yet.
    #[error("audio capture not implemented: {0}")]
    NotImplemented(&'static str),
    /// Platform has no capture adapter (disabled skeleton).
    #[error("audio capture unsupported on this platform")]
    Unsupported,
    /// Runtime device error.
    #[error("audio device error: {0}")]
    Device(String),
}

/// A pull-based source of PCM frames (i16 mono samples).
///
/// `Send` so capture can run on a dedicated thread/task. Each `next_frame`
/// returns one frame of `config().frame_samples` samples, or `None` at end.
pub trait CaptureSource: Send {
    /// The format this source produces.
    fn config(&self) -> CaptureConfig;

    /// Pulls the next PCM frame, or `None` when the source is exhausted.
    fn next_frame(&mut self) -> Result<Option<Vec<i16>>, CaptureError>;
}

/// In-memory capture source for tests and offline demos.
///
/// Yields the queued frames in order, then `None`.
pub struct MockCapture {
    config: CaptureConfig,
    frames: std::collections::VecDeque<Vec<i16>>,
}

impl MockCapture {
    /// Builds a mock source from a list of PCM frames.
    pub fn new(config: CaptureConfig, frames: Vec<Vec<i16>>) -> Self {
        MockCapture {
            config,
            frames: frames.into(),
        }
    }
}

impl CaptureSource for MockCapture {
    fn config(&self) -> CaptureConfig {
        self.config
    }

    fn next_frame(&mut self) -> Result<Option<Vec<i16>>, CaptureError> {
        Ok(self.frames.pop_front())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_phase_one_16k_mono() {
        let cfg = CaptureConfig::default();
        assert_eq!(cfg.sample_rate, 16_000);
        assert_eq!(cfg.channels, 1);
        assert_eq!(cfg.frame_samples, DEFAULT_FRAME_SAMPLES);
    }

    #[test]
    fn mock_capture_yields_frames_then_none() {
        let mut cap = MockCapture::new(CaptureConfig::default(), vec![vec![1, 2], vec![3, 4]]);
        assert_eq!(cap.next_frame().unwrap(), Some(vec![1, 2]));
        assert_eq!(cap.next_frame().unwrap(), Some(vec![3, 4]));
        assert_eq!(cap.next_frame().unwrap(), None);
    }
}
