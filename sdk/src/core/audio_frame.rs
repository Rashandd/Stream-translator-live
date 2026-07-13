//! SDK-side binary audio frame builder for the backend WebSocket protocol.

/// Fixed binary audio frame header length.
pub const HEADER_LEN: usize = 8;

/// StreamDubber audio frame signature: `SDAU`.
pub const MAGIC: [u8; 4] = [0x53, 0x44, 0x41, 0x55];

/// Maximum payload accepted by the backend protocol.
pub const MAX_PAYLOAD_LEN: usize = 4096;

/// SDK audio codec values understood by the backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioCodec {
    Opus,
    PcmS16Le,
}

impl AudioCodec {
    pub fn wire_byte(self) -> u8 {
        match self {
            AudioCodec::Opus => 0x01,
            AudioCodec::PcmS16Le => 0x02,
        }
    }

    pub fn as_protocol_name(self) -> &'static str {
        match self {
            AudioCodec::Opus => "opus",
            AudioCodec::PcmS16Le => "pcm_s16le",
        }
    }
}

/// Audio frame header flag bitleri (header byte[5], eski `reserved` byte).
///
/// `0x00` = işaret yok → tam geriye uyumlu (backend eski `reserved: 0x00`
/// frame'leri aynen parse eder). Utterance birleştirme (Bölge A) için bit0
/// kullanılır; A2 VAD cümle sonunu görünce son frame'i `UTTERANCE_END` ile
/// işaretler, backend (A3) o noktada tamponu flush eder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FrameFlags(u8);

impl FrameFlags {
    /// İşaret yok — geriye uyumlu `0x00`.
    pub const NONE: FrameFlags = FrameFlags(0x00);
    /// bit0 — bu frame bir utterance'ın (cümlenin) son frame'i.
    pub const UTTERANCE_END: FrameFlags = FrameFlags(0x01);

    /// Ham wire byte (header byte[5]).
    pub const fn bits(self) -> u8 {
        self.0
    }
}

/// Audio frame construction errors.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AudioFrameError {
    #[error("audio payload too large: {actual} bytes, max {max}")]
    PayloadTooLarge { actual: usize, max: usize },
}

/// Builds a backend-compatible binary audio frame.
///
/// `flags` header byte[5]'e yazılır; [`FrameFlags::NONE`] geriye uyumlu `0x00`
/// üretir (eski davranış). Utterance sonu işareti için [`FrameFlags::UTTERANCE_END`].
pub fn build_audio_frame(
    codec: AudioCodec,
    flags: FrameFlags,
    payload: &[u8],
) -> Result<Vec<u8>, AudioFrameError> {
    if payload.len() > MAX_PAYLOAD_LEN {
        return Err(AudioFrameError::PayloadTooLarge {
            actual: payload.len(),
            max: MAX_PAYLOAD_LEN,
        });
    }

    let mut frame = Vec::with_capacity(HEADER_LEN + payload.len());
    frame.extend_from_slice(&MAGIC);
    frame.push(codec.wire_byte());
    frame.push(flags.bits());
    frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    frame.extend_from_slice(payload);
    Ok(frame)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_opus_frame_with_backend_header() {
        let frame = build_audio_frame(AudioCodec::Opus, FrameFlags::NONE, &[1, 2, 3]).expect("frame");

        assert_eq!(&frame[0..4], &MAGIC);
        assert_eq!(frame[4], 0x01);
        assert_eq!(frame[5], 0x00); // NONE → geriye uyumlu reserved 0x00
        assert_eq!(&frame[6..8], &[0x00, 0x03]);
        assert_eq!(&frame[8..], &[1, 2, 3]);
    }

    #[test]
    fn sets_utterance_end_flag_bit() {
        let frame =
            build_audio_frame(AudioCodec::PcmS16Le, FrameFlags::UTTERANCE_END, &[9]).expect("frame");
        assert_eq!(frame[5], 0x01); // header byte[5] = flags, bit0 set
        // Diğer header alanları etkilenmez.
        assert_eq!(frame[4], 0x02);
        assert_eq!(&frame[6..8], &[0x00, 0x01]);
        assert_eq!(&frame[8..], &[9]);
    }

    #[test]
    fn maps_pcm_codec_to_protocol_values() {
        assert_eq!(AudioCodec::PcmS16Le.wire_byte(), 0x02);
        assert_eq!(AudioCodec::PcmS16Le.as_protocol_name(), "pcm_s16le");
    }

    #[test]
    fn rejects_payloads_larger_than_backend_limit() {
        let payload = vec![0; MAX_PAYLOAD_LEN + 1];

        assert_eq!(
            build_audio_frame(AudioCodec::Opus, FrameFlags::NONE, &payload),
            Err(AudioFrameError::PayloadTooLarge {
                actual: MAX_PAYLOAD_LEN + 1,
                max: MAX_PAYLOAD_LEN
            })
        );
    }
}
