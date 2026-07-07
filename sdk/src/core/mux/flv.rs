//! Serializes relay `RtmpMessage`s (already-parsed RTMP media messages) into
//! an FLV byte stream — the exact format ffmpeg's `-f flv` demuxer expects on
//! stdin. RTMP media message payloads and FLV tag bodies are byte-for-byte
//! identical by design (both are Adobe/FLV-era formats), so this is pure
//! container framing: no audio/video re-encoding, no payload transformation.
//!
//! Only video and script/metadata (`onMetaData`) tags are written — audio is
//! deliberately dropped here because the dub target's audio comes from a
//! second ffmpeg input (the dub audio scheduler), not the original stream.

use super::super::rtmp::{RtmpMessage, MSG_TYPE_AMF0_DATA, MSG_TYPE_VIDEO};

const FLV_HEADER: [u8; 9] = [
    0x46, 0x4C, 0x56, // "FLV"
    0x01, // version
    0x01, // flags: video present, no audio
    0x00, 0x00, 0x00, 0x09, // data offset (header size)
];
const FLV_TAG_HEADER_LEN: u32 = 11;

/// Streaming `RtmpMessage` → FLV tag serializer. One instance per ffmpeg
/// stdin / dub-target connection — rebases timestamps to start near zero
/// (same principle as `rtmp::relay::forward_loop`: a fresh ffmpeg run has no
/// use for the ingest's absolute timestamp).
pub struct FlvWriter {
    base_ts: Option<u32>,
    wrote_header: bool,
}

impl FlvWriter {
    pub fn new() -> Self {
        FlvWriter {
            base_ts: None,
            wrote_header: false,
        }
    }

    /// Appends the FLV-framed form of `msg` to `out`. No-ops for message
    /// types other than video/script (audio is intentionally dropped — see
    /// module docs) — the FLV header is only written on the first tag that
    /// actually gets forwarded, not eagerly.
    pub fn write_tag(&mut self, out: &mut Vec<u8>, msg: &RtmpMessage) {
        if msg.msg_type_id != MSG_TYPE_VIDEO && msg.msg_type_id != MSG_TYPE_AMF0_DATA {
            return;
        }
        if !self.wrote_header {
            out.extend_from_slice(&FLV_HEADER);
            out.extend_from_slice(&0u32.to_be_bytes()); // PreviousTagSize0
            self.wrote_header = true;
        }

        let base = *self.base_ts.get_or_insert(msg.timestamp);
        let ts = msg.timestamp.wrapping_sub(base);
        let data_size = msg.payload.len() as u32;

        out.push(msg.msg_type_id);
        out.extend_from_slice(&data_size.to_be_bytes()[1..4]); // DataSize, u24 big-endian
        out.extend_from_slice(&ts.to_be_bytes()[1..4]); // Timestamp lower 24 bits
        out.push((ts >> 24) as u8); // TimestampExtended (upper 8 bits)
        out.extend_from_slice(&[0, 0, 0]); // StreamID, always 0
        out.extend_from_slice(&msg.payload);

        let prev_tag_size = FLV_TAG_HEADER_LEN + data_size;
        out.extend_from_slice(&prev_tag_size.to_be_bytes());
    }
}

impl Default for FlvWriter {
    fn default() -> Self {
        FlvWriter::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::rtmp::MSG_TYPE_AUDIO;

    fn video(timestamp: u32, payload: &[u8]) -> RtmpMessage {
        RtmpMessage {
            msg_type_id: MSG_TYPE_VIDEO,
            timestamp,
            msg_stream_id: 1,
            payload: payload.to_vec(),
        }
    }

    fn audio(timestamp: u32, payload: &[u8]) -> RtmpMessage {
        RtmpMessage {
            msg_type_id: MSG_TYPE_AUDIO,
            timestamp,
            msg_stream_id: 1,
            payload: payload.to_vec(),
        }
    }

    /// One parsed FLV tag, plus how many bytes it (and its trailing
    /// PreviousTagSize) occupied — lets tests walk multiple tags in one buffer.
    struct ParsedTag {
        tag_type: u8,
        timestamp: u32,
        data: Vec<u8>,
        prev_tag_size: u32,
        consumed: usize,
    }

    fn parse_tag(buf: &[u8]) -> ParsedTag {
        let tag_type = buf[0];
        let data_size = u32::from_be_bytes([0, buf[1], buf[2], buf[3]]);
        let ts_lower = u32::from_be_bytes([0, buf[4], buf[5], buf[6]]);
        let ts_upper = buf[7] as u32;
        let timestamp = (ts_upper << 24) | ts_lower;
        // bytes 8..11 are StreamID (always 0, not asserted here)
        let data_start = 11;
        let data_end = data_start + data_size as usize;
        let data = buf[data_start..data_end].to_vec();
        let prev_tag_size = u32::from_be_bytes(buf[data_end..data_end + 4].try_into().unwrap());
        ParsedTag {
            tag_type,
            timestamp,
            data,
            prev_tag_size,
            consumed: data_end + 4,
        }
    }

    #[test]
    fn writes_flv_header_once_before_the_first_tag() {
        let mut writer = FlvWriter::new();
        let mut out = Vec::new();
        writer.write_tag(&mut out, &video(1000, &[1, 2, 3]));
        writer.write_tag(&mut out, &video(1020, &[4, 5]));

        assert_eq!(&out[0..9], &FLV_HEADER);
        assert_eq!(&out[9..13], &0u32.to_be_bytes()); // PreviousTagSize0

        let tag1 = parse_tag(&out[13..]);
        assert_eq!(tag1.data, vec![1, 2, 3]);
        // Header bytes must not reappear before the second tag.
        let tag2 = parse_tag(&out[13 + tag1.consumed..]);
        assert_eq!(tag2.data, vec![4, 5]);
    }

    #[test]
    fn audio_tags_are_dropped_entirely() {
        let mut writer = FlvWriter::new();
        let mut out = Vec::new();
        writer.write_tag(&mut out, &audio(1000, &[9, 9, 9]));
        writer.write_tag(&mut out, &audio(1020, &[9, 9]));

        assert!(out.is_empty(), "audio-only input should never write the FLV header either");
    }

    #[test]
    fn video_and_script_tags_are_kept_audio_is_skipped() {
        let mut writer = FlvWriter::new();
        let mut out = Vec::new();
        let script = RtmpMessage {
            msg_type_id: MSG_TYPE_AMF0_DATA,
            timestamp: 0,
            msg_stream_id: 1,
            payload: vec![7, 7],
        };
        writer.write_tag(&mut out, &script);
        writer.write_tag(&mut out, &audio(5, &[9, 9, 9])); // dropped, no bytes appended
        writer.write_tag(&mut out, &video(10, &[1, 2, 3, 4]));

        let tag1 = parse_tag(&out[13..]);
        assert_eq!(tag1.tag_type, MSG_TYPE_AMF0_DATA);
        assert_eq!(tag1.data, vec![7, 7]);

        let tag2 = parse_tag(&out[13 + tag1.consumed..]);
        assert_eq!(tag2.tag_type, MSG_TYPE_VIDEO);
        assert_eq!(tag2.data, vec![1, 2, 3, 4]);
    }

    #[test]
    fn rebases_timestamps_to_start_near_zero() {
        let mut writer = FlvWriter::new();
        let mut out = Vec::new();
        writer.write_tag(&mut out, &video(1000, &[1]));
        writer.write_tag(&mut out, &video(1040, &[2]));

        let tag1 = parse_tag(&out[13..]);
        assert_eq!(tag1.timestamp, 0);
        let tag2 = parse_tag(&out[13 + tag1.consumed..]);
        assert_eq!(tag2.timestamp, 40);
    }

    #[test]
    fn previous_tag_size_matches_header_plus_payload() {
        let mut writer = FlvWriter::new();
        let mut out = Vec::new();
        writer.write_tag(&mut out, &video(0, &[1, 2, 3, 4, 5]));

        let tag = parse_tag(&out[13..]);
        assert_eq!(tag.prev_tag_size, FLV_TAG_HEADER_LEN + 5);
    }
}
