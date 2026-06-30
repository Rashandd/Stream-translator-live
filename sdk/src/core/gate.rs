//! Capture → VAD → frame pipeline (SDK side, local).
//!
//! Pulls PCM frames from a [`CaptureSource`], drops silence via [`VadGate`], and
//! packages voiced frames into backend-compatible binary audio frames. Silent
//! audio never leaves the machine — only speech is sent to the cloud.

use super::audio_frame::{build_audio_frame, AudioCodec, AudioFrameError};
use super::capture::{CaptureError, CaptureSource};
use super::preprocess::Preprocessor;
use super::vad::VadGate;

/// Errors surfaced while pulling and packaging audio.
#[derive(Debug, thiserror::Error)]
pub enum GateError {
    #[error(transparent)]
    Capture(#[from] CaptureError),
    #[error(transparent)]
    Frame(#[from] AudioFrameError),
}

/// Counters for observability (e.g. dashboard latency/usage panels).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GateStats {
    pub frames_seen: u64,
    pub frames_forwarded: u64,
    pub frames_dropped_silence: u64,
}

/// Drives a capture source through optional preprocessing + VAD and produces
/// backend audio frames.
pub struct CapturePipeline<S: CaptureSource> {
    source: S,
    preprocessor: Option<Preprocessor>,
    vad: VadGate,
    codec: AudioCodec,
    stats: GateStats,
}

impl<S: CaptureSource> CapturePipeline<S> {
    /// Builds a pipeline without preprocessing. Phase 1 sends PCM s16le upstream
    /// (Opus encoding later).
    pub fn new(source: S, vad: VadGate) -> Self {
        CapturePipeline {
            source,
            preprocessor: None,
            vad,
            codec: AudioCodec::PcmS16Le,
            stats: GateStats::default(),
        }
    }

    /// Inserts a local preprocessing (noise cleanup) step before VAD.
    pub fn with_preprocessor(mut self, preprocessor: Preprocessor) -> Self {
        self.preprocessor = Some(preprocessor);
        self
    }

    /// Cumulative counters.
    pub fn stats(&self) -> GateStats {
        self.stats
    }

    /// Returns the next *speech* frame as a backend binary frame.
    ///
    /// Silent frames are consumed and skipped (not returned). Returns `None`
    /// only when the capture source is exhausted.
    pub fn next_frame(&mut self) -> Result<Option<Vec<u8>>, GateError> {
        while let Some(raw) = self.source.next_frame()? {
            self.stats.frames_seen += 1;
            let samples = match self.preprocessor.as_mut() {
                Some(p) => p.process(&raw),
                None => raw,
            };
            if self.vad.forward(&samples) {
                self.stats.frames_forwarded += 1;
                let payload = pcm_to_le_bytes(&samples);
                let frame = build_audio_frame(self.codec, &payload)?;
                return Ok(Some(frame));
            }
            self.stats.frames_dropped_silence += 1;
        }
        Ok(None)
    }
}

/// Encodes i16 samples as little-endian PCM bytes.
fn pcm_to_le_bytes(samples: &[i16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for &s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::audio_frame::{HEADER_LEN, MAGIC};
    use crate::core::capture::{CaptureConfig, MockCapture};
    use crate::core::vad::VadConfig;

    fn loud(n: usize) -> Vec<i16> {
        (0..n).map(|i| if i % 2 == 0 { 5000 } else { -5000 }).collect()
    }

    fn no_hangover() -> VadGate {
        VadGate::new(VadConfig {
            rms_threshold: 500,
            hangover_frames: 0,
        })
    }

    #[test]
    fn drops_silence_and_forwards_speech() {
        let frames = vec![vec![0i16; 320], loud(320), vec![0i16; 320]];
        let source = MockCapture::new(CaptureConfig::default(), frames);
        let mut pipeline = CapturePipeline::new(source, no_hangover());

        // Only the speech frame comes out.
        let frame = pipeline.next_frame().unwrap().expect("speech frame");
        assert_eq!(&frame[0..4], &MAGIC);
        assert_eq!(frame.len(), HEADER_LEN + 320 * 2);

        // Source exhausted after the trailing silence is skipped.
        assert_eq!(pipeline.next_frame().unwrap(), None);

        let stats = pipeline.stats();
        assert_eq!(stats.frames_seen, 3);
        assert_eq!(stats.frames_forwarded, 1);
        assert_eq!(stats.frames_dropped_silence, 2);
    }

    #[test]
    fn all_silence_yields_no_frames() {
        let frames = vec![vec![0i16; 320], vec![0i16; 320]];
        let source = MockCapture::new(CaptureConfig::default(), frames);
        let mut pipeline = CapturePipeline::new(source, no_hangover());
        assert_eq!(pipeline.next_frame().unwrap(), None);
        assert_eq!(pipeline.stats().frames_forwarded, 0);
        assert_eq!(pipeline.stats().frames_dropped_silence, 2);
    }

    #[test]
    fn preprocessor_step_is_applied_before_vad() {
        use crate::core::preprocess::{PreprocessConfig, Preprocessor};
        // A constant-bias frame: after DC removal it becomes silence and is dropped.
        let frames = vec![vec![1000i16; 320]];
        let source = MockCapture::new(CaptureConfig::default(), frames);
        let pre = Preprocessor::new(PreprocessConfig {
            dc_removal: true,
            pre_emphasis: 0.0,
        });
        let mut pipeline = CapturePipeline::new(source, no_hangover()).with_preprocessor(pre);
        // Constant bias -> DC-removed to zero -> VAD treats as silence -> no frame.
        assert_eq!(pipeline.next_frame().unwrap(), None);
        assert_eq!(pipeline.stats().frames_dropped_silence, 1);
    }

    #[test]
    fn payload_is_little_endian_pcm() {
        let frames = vec![vec![0x0102i16, 0x0304]];
        // Force-forward by using a zero threshold so the frame counts as voiced.
        let vad = VadGate::new(VadConfig {
            rms_threshold: 0,
            hangover_frames: 0,
        });
        let source = MockCapture::new(CaptureConfig::default(), frames);
        let mut pipeline = CapturePipeline::new(source, vad);
        let frame = pipeline.next_frame().unwrap().expect("frame");
        // payload starts after the 8-byte header: 0x0102 -> [0x02,0x01], 0x0304 -> [0x04,0x03]
        assert_eq!(&frame[HEADER_LEN..], &[0x02, 0x01, 0x04, 0x03]);
    }
}
