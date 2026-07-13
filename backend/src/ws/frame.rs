//! Binary ses frame parser — SDK → backend ses paketleri.
//!
//! Header 8 byte sabittir (bkz. `docs/websocket-protocol.md`):
//! ```text
//! [0..3]  magic: 0x53 0x44 0x41 0x55  ("SDAU")
//! [4]     codec: 0x01=opus, 0x02=pcm_s16le
//! [5]     flags: bit0=utterance_end; 0x00 = işaret yok (eski "reserved", geriye uyumlu)
//! [6..7]  payload_len: uint16 big-endian
//! [8..]   payload (ses verisi)
//! ```

/// Frame header'ın sabit boyutu (byte).
pub const HEADER_LEN: usize = 8;

/// Sihirli imza: "SDAU".
pub const MAGIC: [u8; 4] = [0x53, 0x44, 0x41, 0x55];

/// Header byte[5] flag biti: bu frame bir utterance'ın (cümlenin) son frame'i.
/// SDK VAD cümle sonunu görünce set eder (Bölge A2); backend tamponu bu noktada
/// flush eder (Bölge A3). Bilinmeyen üst bitler yok sayılır (ileri uyumluluk).
pub const FLAG_UTTERANCE_END: u8 = 0x01;

/// Bir frame'in toplam maksimum boyutu (header + payload).
/// Protokol payload için Opus frame'e yetecek ~4096 byte üst sınır koyar.
pub const MAX_FRAME_LEN: usize = HEADER_LEN + 4096;

/// Ses codec'i.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    Opus,
    PcmS16Le,
}

impl Codec {
    fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x01 => Some(Codec::Opus),
            0x02 => Some(Codec::PcmS16Le),
            _ => None,
        }
    }
}

/// Parse edilmiş ses frame'i. Payload, girdi buffer'ına ödünç referanstır.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioFrame<'a> {
    pub codec: Codec,
    /// Header byte[5] bit0 — bu frame bir utterance'ın son frame'i (Bölge A2/A3).
    pub utterance_end: bool,
    pub payload: &'a [u8],
}

/// Frame parse hataları. Her biri protokol error koduyla eşleşir.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FrameError {
    #[error("frame too short: {0} bytes, need at least {HEADER_LEN}")]
    TooShort(usize),
    #[error("bad magic bytes")]
    BadMagic,
    #[error("unknown codec byte: {0:#04x}")]
    UnknownCodec(u8),
    #[error("declared payload_len {declared} != actual {actual}")]
    LengthMismatch { declared: usize, actual: usize },
    #[error("frame too large: {0} bytes, max {MAX_FRAME_LEN}")]
    TooLarge(usize),
}

/// Bir binary frame'i parse eder.
///
/// Header byte[5] `flags`'tir: `bit0` utterance sonu işareti, bilinmeyen üst
/// bitler yok sayılır (ileri uyumluluk — eski `reserved: 0x00` frame'leri
/// `utterance_end=false` olarak parse edilir). Header'da bildirilen
/// `payload_len`, gerçek payload uzunluğuyla birebir eşleşmelidir.
pub fn parse(buf: &[u8]) -> Result<AudioFrame<'_>, FrameError> {
    if buf.len() > MAX_FRAME_LEN {
        return Err(FrameError::TooLarge(buf.len()));
    }
    if buf.len() < HEADER_LEN {
        return Err(FrameError::TooShort(buf.len()));
    }
    if buf[0..4] != MAGIC {
        return Err(FrameError::BadMagic);
    }
    let codec = Codec::from_byte(buf[4]).ok_or(FrameError::UnknownCodec(buf[4]))?;
    let utterance_end = buf[5] & FLAG_UTTERANCE_END != 0;

    let declared = u16::from_be_bytes([buf[6], buf[7]]) as usize;
    let payload = &buf[HEADER_LEN..];
    if declared != payload.len() {
        return Err(FrameError::LengthMismatch {
            declared,
            actual: payload.len(),
        });
    }

    Ok(AudioFrame {
        codec,
        utterance_end,
        payload,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verilen payload + codec ile geçerli bir frame buffer'ı kurar (flags=0x00).
    fn build(codec: u8, payload: &[u8]) -> Vec<u8> {
        build_flags(codec, 0x00, payload)
    }

    /// `build` gibi, ama header byte[5] flags değerini açıkça alır.
    fn build_flags(codec: u8, flags: u8, payload: &[u8]) -> Vec<u8> {
        let len = payload.len() as u16;
        let mut buf = Vec::with_capacity(HEADER_LEN + payload.len());
        buf.extend_from_slice(&MAGIC);
        buf.push(codec);
        buf.push(flags);
        buf.extend_from_slice(&len.to_be_bytes());
        buf.extend_from_slice(payload);
        buf
    }

    #[test]
    fn parses_opus_frame() {
        let buf = build(0x01, &[1, 2, 3, 4]);
        let frame = parse(&buf).expect("valid opus frame");
        assert_eq!(frame.codec, Codec::Opus);
        assert_eq!(frame.payload, &[1, 2, 3, 4]);
        assert!(!frame.utterance_end); // flags=0x00 → işaret yok
    }

    #[test]
    fn parses_pcm_frame() {
        let buf = build(0x02, &[9, 9]);
        let frame = parse(&buf).expect("valid pcm frame");
        assert_eq!(frame.codec, Codec::PcmS16Le);
        assert_eq!(frame.payload, &[9, 9]);
    }

    #[test]
    fn parses_empty_payload() {
        let buf = build(0x01, &[]);
        let frame = parse(&buf).expect("valid empty-payload frame");
        assert_eq!(frame.codec, Codec::Opus);
        assert!(frame.payload.is_empty());
    }

    #[test]
    fn parses_utterance_end_flag() {
        let buf = build_flags(0x01, FLAG_UTTERANCE_END, &[7]);
        let frame = parse(&buf).expect("valid flagged frame");
        assert!(frame.utterance_end);
        assert_eq!(frame.payload, &[7]);
    }

    #[test]
    fn ignores_unknown_flag_bits() {
        // Üst bitler (bit0 dışı) ileri uyumluluk için yok sayılır: 0xFE'de
        // bit0 kapalı → utterance_end false, parse yine de başarılı.
        let buf = build_flags(0x01, 0xFE, &[7]);
        let frame = parse(&buf).expect("unknown flag bits ignored");
        assert!(!frame.utterance_end);
        assert_eq!(frame.payload, &[7]);
    }

    #[test]
    fn rejects_too_short() {
        assert_eq!(parse(&[0x53, 0x44, 0x41]), Err(FrameError::TooShort(3)));
    }

    #[test]
    fn rejects_bad_magic() {
        let mut buf = build(0x01, &[1]);
        buf[0] = 0x00;
        assert_eq!(parse(&buf), Err(FrameError::BadMagic));
    }

    #[test]
    fn rejects_unknown_codec() {
        let buf = build(0x09, &[1]);
        assert_eq!(parse(&buf), Err(FrameError::UnknownCodec(0x09)));
    }

    #[test]
    fn rejects_length_mismatch() {
        let mut buf = build(0x01, &[1, 2, 3, 4]);
        // declared len'i 4 → 2 yap, ama payload hâlâ 4 byte
        buf[6] = 0x00;
        buf[7] = 0x02;
        assert_eq!(
            parse(&buf),
            Err(FrameError::LengthMismatch {
                declared: 2,
                actual: 4
            })
        );
    }

    #[test]
    fn rejects_too_large() {
        let buf = vec![0u8; MAX_FRAME_LEN + 1];
        assert_eq!(parse(&buf), Err(FrameError::TooLarge(MAX_FRAME_LEN + 1)));
    }
}
