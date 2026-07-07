//! Local video + audio output planning, and (Phase 2, task 2.3) the real
//! "cihazda mux" engine: for relay targets with a `dub_language`, the
//! original audio is replaced with cloud-synthesized dubbed audio before
//! publishing to the real platform (YouTube/Twitch/Kick) — video is never
//! altered or re-encoded (`-c:v copy`). See `docs/multistream.md` for the
//! device-vs-cloud mux decision; only device mux is implemented here.
//!
//! Submodules: [`flv`] serializes relay `RtmpMessage`s into an FLV byte
//! stream (ffmpeg's stdin input). More land alongside it as the engine is
//! built out: an audio scheduler (bursty dubbed audio → continuous PCM,
//! ffmpeg's second input over a local TCP socket) and an ffmpeg child
//! process wrapper. `rtmp::relay` wires these together into `run_dub_target`,
//! the dub-target counterpart of `run_target`.

pub mod audio_scheduler;
pub mod ffmpeg_process;
pub mod flv;

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
