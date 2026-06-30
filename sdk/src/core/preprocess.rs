//! Lightweight local audio preprocessing (noise reduction / cleanup).
//!
//! Runs on the broadcaster machine before VAD and framing. Phase 1 keeps it
//! cheap and dependency-free:
//! - DC offset removal (removes constant bias from cheap mics)
//! - pre-emphasis high-pass (attenuates low-frequency rumble/hum)
//!
//! Heavier denoising (RNNoise/spectral gating) can replace this behind the same
//! `Preprocessor` interface later without touching the capture pipeline.

/// Preprocessing parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreprocessConfig {
    /// Remove the DC (mean) component of each frame.
    pub dc_removal: bool,
    /// Pre-emphasis coefficient (0.0 = off, ~0.97 typical). High-pass-like.
    pub pre_emphasis: f32,
}

impl Default for PreprocessConfig {
    fn default() -> Self {
        PreprocessConfig {
            dc_removal: true,
            pre_emphasis: 0.97,
        }
    }
}

/// Stateful preprocessor (keeps last sample across frames for continuity).
#[derive(Debug, Clone)]
pub struct Preprocessor {
    config: PreprocessConfig,
    last_sample: f32,
}

impl Preprocessor {
    pub fn new(config: PreprocessConfig) -> Self {
        Preprocessor {
            config,
            last_sample: 0.0,
        }
    }

    /// Cleans one PCM s16 frame in place-of a new buffer.
    pub fn process(&mut self, samples: &[i16]) -> Vec<i16> {
        if samples.is_empty() {
            return Vec::new();
        }

        let mut buf: Vec<f32> = samples.iter().map(|&s| s as f32).collect();

        if self.config.dc_removal {
            let mean = buf.iter().sum::<f32>() / buf.len() as f32;
            for s in &mut buf {
                *s -= mean;
            }
        }

        if self.config.pre_emphasis > 0.0 {
            let coeff = self.config.pre_emphasis;
            let mut prev = self.last_sample;
            for s in &mut buf {
                let current = *s;
                *s = current - coeff * prev;
                prev = current;
            }
            self.last_sample = prev;
        }

        buf.into_iter().map(clamp_to_i16).collect()
    }
}

impl Default for Preprocessor {
    fn default() -> Self {
        Preprocessor::new(PreprocessConfig::default())
    }
}

/// Saturating conversion from f32 back to i16.
fn clamp_to_i16(v: f32) -> i16 {
    v.round().clamp(i16::MIN as f32, i16::MAX as f32) as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_frame_stays_empty() {
        let mut p = Preprocessor::default();
        assert!(p.process(&[]).is_empty());
    }

    #[test]
    fn dc_removal_centers_constant_bias() {
        // Pure DC bias, no pre-emphasis: mean should drop to ~0.
        let mut p = Preprocessor::new(PreprocessConfig {
            dc_removal: true,
            pre_emphasis: 0.0,
        });
        let out = p.process(&[1000, 1000, 1000, 1000]);
        assert!(out.iter().all(|&s| s == 0), "out={out:?}");
    }

    #[test]
    fn silence_stays_silence() {
        let mut p = Preprocessor::default();
        let out = p.process(&[0, 0, 0, 0]);
        assert!(out.iter().all(|&s| s == 0));
    }

    #[test]
    fn output_length_matches_input() {
        let mut p = Preprocessor::default();
        let out = p.process(&[100, -200, 300, -400, 500]);
        assert_eq!(out.len(), 5);
    }

    #[test]
    fn does_not_overflow_on_extremes() {
        let mut p = Preprocessor::new(PreprocessConfig {
            dc_removal: false,
            pre_emphasis: 0.97,
        });
        // Alternating extremes would exceed i16 after pre-emphasis without clamping.
        let out = p.process(&[i16::MAX, i16::MIN, i16::MAX, i16::MIN]);
        assert_eq!(out.len(), 4);
        // No panic + values are valid i16 by construction.
    }
}
