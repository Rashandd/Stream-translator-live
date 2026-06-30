//! Voice Activity Detection (VAD) — local, lightweight.
//!
//! Runs on the broadcaster machine so silent audio is never sent to the cloud
//! (lowers STT cost ~30-40%, see roadmap). Phase 1 uses a simple RMS energy
//! gate with a hangover window so trailing speech isn't clipped.
//!
//! Platform-independent on purpose: VAD belongs in `core/`, not `audio/`.

/// VAD tuning parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VadConfig {
    /// RMS amplitude threshold (0..32767). Frames at/above this are "voiced".
    pub rms_threshold: u16,
    /// Frames to keep forwarding after speech ends (avoids clipping tails).
    pub hangover_frames: u32,
}

impl Default for VadConfig {
    fn default() -> Self {
        // ~500 amplitude is a conservative speech floor for 16-bit PCM;
        // 10 frames ≈ 200 ms hangover at 20 ms/frame.
        VadConfig {
            rms_threshold: 500,
            hangover_frames: 10,
        }
    }
}

/// Root-mean-square amplitude of a PCM s16 frame.
pub fn frame_rms(samples: &[i16]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f64 = samples.iter().map(|&s| (s as f64) * (s as f64)).sum();
    (sum_sq / samples.len() as f64).sqrt()
}

/// Stateful gate: decides whether each frame should be forwarded to the cloud.
#[derive(Debug, Clone)]
pub struct VadGate {
    config: VadConfig,
    hangover_remaining: u32,
}

impl VadGate {
    pub fn new(config: VadConfig) -> Self {
        VadGate {
            config,
            hangover_remaining: 0,
        }
    }

    /// True if the frame's energy is at/above the speech threshold.
    pub fn is_voiced(&self, samples: &[i16]) -> bool {
        frame_rms(samples) >= self.config.rms_threshold as f64
    }

    /// Processes one frame and returns whether it should be forwarded.
    ///
    /// Voiced frames pass and arm the hangover; subsequent silent frames pass
    /// until the hangover is exhausted, then are dropped.
    pub fn forward(&mut self, samples: &[i16]) -> bool {
        if self.is_voiced(samples) {
            self.hangover_remaining = self.config.hangover_frames;
            true
        } else if self.hangover_remaining > 0 {
            self.hangover_remaining -= 1;
            true
        } else {
            false
        }
    }
}

impl Default for VadGate {
    fn default() -> Self {
        VadGate::new(VadConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn silence(n: usize) -> Vec<i16> {
        vec![0; n]
    }

    /// A loud frame: alternating +/- amplitude well above threshold.
    fn loud(n: usize, amp: i16) -> Vec<i16> {
        (0..n)
            .map(|i| if i % 2 == 0 { amp } else { -amp })
            .collect()
    }

    #[test]
    fn rms_of_silence_is_zero() {
        assert_eq!(frame_rms(&silence(320)), 0.0);
    }

    #[test]
    fn rms_of_constant_amplitude_equals_amplitude() {
        let rms = frame_rms(&loud(320, 1000));
        assert!((rms - 1000.0).abs() < 1.0, "rms={rms}");
    }

    #[test]
    fn silence_is_not_voiced() {
        let gate = VadGate::default();
        assert!(!gate.is_voiced(&silence(320)));
    }

    #[test]
    fn loud_frame_is_voiced() {
        let gate = VadGate::default();
        assert!(gate.is_voiced(&loud(320, 2000)));
    }

    #[test]
    fn drops_leading_silence() {
        let mut gate = VadGate::new(VadConfig {
            rms_threshold: 500,
            hangover_frames: 0,
        });
        assert!(!gate.forward(&silence(320)));
    }

    #[test]
    fn forwards_speech_then_hangover_then_drops() {
        let mut gate = VadGate::new(VadConfig {
            rms_threshold: 500,
            hangover_frames: 2,
        });
        // Speech → forwarded, arms 2-frame hangover.
        assert!(gate.forward(&loud(320, 3000)));
        // Two trailing silent frames ride the hangover.
        assert!(gate.forward(&silence(320)));
        assert!(gate.forward(&silence(320)));
        // Hangover exhausted → silence dropped.
        assert!(!gate.forward(&silence(320)));
    }
}
