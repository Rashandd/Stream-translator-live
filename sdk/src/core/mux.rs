//! Local video + audio output planning (Mod A, Phase 1).
//!
//! IMPORTANT: in Phase 1 the SDK does NOT mux/relay video. Video stays local and
//! is published by the broadcaster's own OBS / Restream / Castr. The SDK only:
//!   - sends audio to the cloud (subtitles come back),
//!   - feeds subtitles to OBS via a browser/text source overlay.
//!
//! Real local muxing of *cloned audio* into the outgoing stream is Phase 2
//! (cloned audio + SDK RTMP relay). This module captures the Phase-1 output
//! *decision* so downstream code is explicit about where each track goes.

/// Where the audio track comes from for the outgoing stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioTrack {
    /// Broadcaster's original microphone audio (Phase 1).
    Original,
    /// Cloud-cloned audio merged locally (Phase 2+).
    Cloned,
}

/// How subtitles reach the viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubtitleDelivery {
    /// OBS browser/text source overlay (Phase 1 default).
    ObsOverlay,
    /// Platform-native CC (e.g. YouTube) — separate integration.
    NativeCc,
}

/// The resolved output plan for a stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputPlan {
    pub audio: AudioTrack,
    pub subtitles: SubtitleDelivery,
    /// True when video is muxed/relayed locally by the SDK (Phase 2 Mod B).
    pub local_video_relay: bool,
}

impl OutputPlan {
    /// Phase 1 (Mod A): original audio, OBS overlay subtitles, no local relay.
    pub fn phase_one() -> Self {
        OutputPlan {
            audio: AudioTrack::Original,
            subtitles: SubtitleDelivery::ObsOverlay,
            local_video_relay: false,
        }
    }
}

impl Default for OutputPlan {
    fn default() -> Self {
        OutputPlan::phase_one()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_one_keeps_original_audio_and_no_relay() {
        let plan = OutputPlan::phase_one();
        assert_eq!(plan.audio, AudioTrack::Original);
        assert_eq!(plan.subtitles, SubtitleDelivery::ObsOverlay);
        assert!(!plan.local_video_relay);
    }
}
