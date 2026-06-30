//! Platform-independent stream session configuration.

use super::audio_frame::AudioCodec;

/// STT-oriented audio format for phase 1.
pub const SAMPLE_RATE_HZ: u32 = 16_000;
pub const CHANNELS: u8 = 1;

/// SDK stream settings sent before binary audio frames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamConfig {
    pub session_id: String,
    pub source_lang: String,
    pub target_langs: Vec<String>,
    pub codec: AudioCodec,
    pub sample_rate: u32,
    pub channels: u8,
}

impl StreamConfig {
    pub fn new(
        session_id: impl Into<String>,
        source_lang: impl Into<String>,
        target_langs: Vec<String>,
        codec: AudioCodec,
    ) -> Self {
        StreamConfig {
            session_id: session_id.into(),
            source_lang: source_lang.into(),
            target_langs,
            codec,
            sample_rate: SAMPLE_RATE_HZ,
            channels: CHANNELS,
        }
    }

    pub fn validate(&self) -> Result<(), StreamConfigError> {
        if self.session_id.trim().is_empty() {
            return Err(StreamConfigError::MissingSessionId);
        }
        if self.source_lang.trim().is_empty() {
            return Err(StreamConfigError::MissingSourceLang);
        }
        if self.target_langs.is_empty() {
            return Err(StreamConfigError::MissingTargets);
        }
        if self.sample_rate != SAMPLE_RATE_HZ {
            return Err(StreamConfigError::UnsupportedSampleRate(self.sample_rate));
        }
        if self.channels != CHANNELS {
            return Err(StreamConfigError::UnsupportedChannels(self.channels));
        }
        Ok(())
    }
}

/// Stream config validation errors.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum StreamConfigError {
    #[error("session_id is required")]
    MissingSessionId,
    #[error("source_lang is required")]
    MissingSourceLang,
    #[error("at least one target language is required")]
    MissingTargets,
    #[error("unsupported sample rate: {0}")]
    UnsupportedSampleRate(u32),
    #[error("unsupported channel count: {0}")]
    UnsupportedChannels(u8),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_phase_one_stream_config_passes() {
        let cfg = StreamConfig::new("s1", "tr", vec!["en".to_string()], AudioCodec::Opus);

        assert_eq!(cfg.sample_rate, SAMPLE_RATE_HZ);
        assert_eq!(cfg.channels, CHANNELS);
        assert_eq!(cfg.codec.as_protocol_name(), "opus");
        assert_eq!(cfg.validate(), Ok(()));
    }

    #[test]
    fn rejects_missing_target_languages() {
        let cfg = StreamConfig::new("s1", "tr", Vec::new(), AudioCodec::Opus);

        assert_eq!(cfg.validate(), Err(StreamConfigError::MissingTargets));
    }

    #[test]
    fn rejects_non_stt_sample_rate() {
        let mut cfg = StreamConfig::new("s1", "tr", vec!["en".to_string()], AudioCodec::Opus);
        cfg.sample_rate = 48_000;

        assert_eq!(
            cfg.validate(),
            Err(StreamConfigError::UnsupportedSampleRate(48_000))
        );
    }
}
