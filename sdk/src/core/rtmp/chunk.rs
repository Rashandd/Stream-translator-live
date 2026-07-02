//! RTMP chunk stream — demuxes the wire format into whole `RtmpMessage`s and
//! muxes outgoing messages back into chunks.
//!
//! The reader implements the full basic-header (1/2/3-byte) and message
//! -header (fmt 0/1/2/3) matrix, including extended timestamps and chunk
//! -stream interleaving, since real encoders (OBS) rely on all of it. The
//! writer only ever emits fmt0 (full header) for the first chunk of a
//! message and fmt3 for continuations — simpler and always spec-legal, just
//! not maximally bandwidth-efficient (acceptable: our own outbound link is
//! the broadcaster's already-paid-for upload).

use std::collections::HashMap;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const DEFAULT_CHUNK_SIZE: usize = 128;
const EXTENDED_TIMESTAMP_MARKER: u32 = 0x00FF_FFFF;

pub const MSG_TYPE_SET_CHUNK_SIZE: u8 = 1;

#[derive(Debug, thiserror::Error)]
pub enum ChunkError {
    #[error("io error in chunk stream: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid chunk fmt: {0}")]
    InvalidFmt(u8),
    #[error("chunk stream id {0} has no cached header for a type-3 chunk")]
    UncachedContinuation(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtmpMessage {
    pub msg_type_id: u8,
    pub timestamp: u32,
    pub msg_stream_id: u32,
    pub payload: Vec<u8>,
}

#[derive(Debug, Default)]
struct ChunkStreamState {
    timestamp: u32,
    timestamp_delta: u32,
    message_length: usize,
    message_type_id: u8,
    msg_stream_id: u32,
    extended_timestamp: bool,
    partial: Vec<u8>,
    has_header: bool,
}

/// Stateful chunk demuxer. Holds no I/O handle itself — callers pass the
/// stream in on every read, which keeps this usable with any transport
/// (real `TcpStream`, `tokio::io::duplex` in tests, ...).
pub struct ChunkReader {
    chunk_size: usize,
    states: HashMap<u32, ChunkStreamState>,
}

impl ChunkReader {
    pub fn new() -> Self {
        ChunkReader {
            chunk_size: DEFAULT_CHUNK_SIZE,
            states: HashMap::new(),
        }
    }

    /// Reads chunks (across however many csids are interleaved) until one
    /// full message is assembled, returning it. "Set Chunk Size" protocol
    /// control messages are applied internally and never surfaced.
    pub async fn read_message<S>(&mut self, io: &mut S) -> Result<RtmpMessage, ChunkError>
    where
        S: AsyncRead + Unpin,
    {
        loop {
            let (fmt, csid) = read_basic_header(io).await?;
            let parsed = match fmt {
                0 => {
                    let (timestamp, length, type_id, stream_id, extended) =
                        read_fmt0_header(io).await?;
                    ParsedHeader::Full {
                        timestamp,
                        length,
                        type_id,
                        stream_id,
                        extended,
                    }
                }
                1 => {
                    let (delta, length, type_id, extended) = read_fmt1_header(io).await?;
                    ParsedHeader::Delta {
                        delta,
                        length,
                        type_id,
                        extended,
                    }
                }
                2 => {
                    let (delta, extended) = read_fmt2_header(io).await?;
                    ParsedHeader::TimestampOnly { delta, extended }
                }
                3 => ParsedHeader::Continuation,
                other => return Err(ChunkError::InvalidFmt(other)),
            };

            let needs_ext = matches!(parsed, ParsedHeader::Continuation)
                && self.states.get(&csid).is_some_and(|s| s.extended_timestamp);
            let fmt3_ext_delta = if needs_ext {
                let mut ext = [0u8; 4];
                io.read_exact(&mut ext).await?;
                Some(u32::from_be_bytes(ext))
            } else {
                None
            };

            let state = self.states.entry(csid).or_default();
            apply_header(state, parsed, fmt3_ext_delta, csid)?;

            let remaining = state.message_length - state.partial.len();
            let to_read = remaining.min(self.chunk_size);
            let mut buf = vec![0u8; to_read];
            io.read_exact(&mut buf).await?;

            let state = self.states.entry(csid).or_default();
            state.partial.extend_from_slice(&buf);

            if state.partial.len() < state.message_length {
                continue;
            }

            let msg_type_id = state.message_type_id;
            let timestamp = state.timestamp;
            let msg_stream_id = state.msg_stream_id;
            let payload = std::mem::take(&mut state.partial);

            if msg_type_id == MSG_TYPE_SET_CHUNK_SIZE {
                if let Some(&[a, b, c, d]) = payload.get(0..4) {
                    let new_size = u32::from_be_bytes([a, b, c, d]) as usize;
                    if new_size > 0 {
                        self.chunk_size = new_size;
                    }
                }
                continue;
            }

            return Ok(RtmpMessage {
                msg_type_id,
                timestamp,
                msg_stream_id,
                payload,
            });
        }
    }
}

impl Default for ChunkReader {
    fn default() -> Self {
        Self::new()
    }
}

enum ParsedHeader {
    Full {
        timestamp: u32,
        length: usize,
        type_id: u8,
        stream_id: u32,
        extended: bool,
    },
    Delta {
        delta: u32,
        length: usize,
        type_id: u8,
        extended: bool,
    },
    TimestampOnly {
        delta: u32,
        extended: bool,
    },
    Continuation,
}

fn apply_header(
    state: &mut ChunkStreamState,
    parsed: ParsedHeader,
    fmt3_ext_delta: Option<u32>,
    csid: u32,
) -> Result<(), ChunkError> {
    match parsed {
        ParsedHeader::Full {
            timestamp,
            length,
            type_id,
            stream_id,
            extended,
        } => {
            state.timestamp = timestamp;
            state.timestamp_delta = 0;
            state.message_length = length;
            state.message_type_id = type_id;
            state.msg_stream_id = stream_id;
            state.extended_timestamp = extended;
            state.partial.clear();
            state.has_header = true;
        }
        ParsedHeader::Delta {
            delta,
            length,
            type_id,
            extended,
        } => {
            if !state.has_header {
                return Err(ChunkError::UncachedContinuation(csid));
            }
            state.timestamp = state.timestamp.wrapping_add(delta);
            state.timestamp_delta = delta;
            state.message_length = length;
            state.message_type_id = type_id;
            state.extended_timestamp = extended;
            state.partial.clear();
        }
        ParsedHeader::TimestampOnly { delta, extended } => {
            if !state.has_header {
                return Err(ChunkError::UncachedContinuation(csid));
            }
            state.timestamp = state.timestamp.wrapping_add(delta);
            state.timestamp_delta = delta;
            state.extended_timestamp = extended;
            state.partial.clear();
        }
        ParsedHeader::Continuation => {
            if !state.has_header {
                return Err(ChunkError::UncachedContinuation(csid));
            }
            let is_new_message = state.partial.len() >= state.message_length;
            if is_new_message {
                let delta = fmt3_ext_delta.unwrap_or(state.timestamp_delta);
                state.timestamp = state.timestamp.wrapping_add(delta);
                state.partial.clear();
            }
        }
    }
    Ok(())
}

async fn read_basic_header<S: AsyncRead + Unpin>(io: &mut S) -> Result<(u8, u32), ChunkError> {
    let mut b0 = [0u8; 1];
    io.read_exact(&mut b0).await?;
    let fmt = b0[0] >> 6;
    let csid_low = b0[0] & 0x3F;
    let csid = match csid_low {
        0 => {
            let mut b = [0u8; 1];
            io.read_exact(&mut b).await?;
            64 + b[0] as u32
        }
        1 => {
            let mut b = [0u8; 2];
            io.read_exact(&mut b).await?;
            64 + b[0] as u32 + (b[1] as u32) * 256
        }
        n => n as u32,
    };
    Ok((fmt, csid))
}

async fn read_ext_if_marked<S: AsyncRead + Unpin>(
    io: &mut S,
    value: u32,
) -> Result<(u32, bool), ChunkError> {
    if value == EXTENDED_TIMESTAMP_MARKER {
        let mut ext = [0u8; 4];
        io.read_exact(&mut ext).await?;
        Ok((u32::from_be_bytes(ext), true))
    } else {
        Ok((value, false))
    }
}

async fn read_fmt0_header<S: AsyncRead + Unpin>(
    io: &mut S,
) -> Result<(u32 /*timestamp*/, usize, u8, u32, bool), ChunkError> {
    let mut hdr = [0u8; 11];
    io.read_exact(&mut hdr).await?;
    let (ts, extended) = read_ext_if_marked(io, u24_be(&hdr[0..3])).await?;
    let length = u24_be(&hdr[3..6]) as usize;
    let type_id = hdr[6];
    let stream_id = u32::from_le_bytes([hdr[7], hdr[8], hdr[9], hdr[10]]);
    Ok((ts, length, type_id, stream_id, extended))
}

async fn read_fmt1_header<S: AsyncRead + Unpin>(
    io: &mut S,
) -> Result<(u32 /*delta*/, usize, u8, bool), ChunkError> {
    let mut hdr = [0u8; 7];
    io.read_exact(&mut hdr).await?;
    let (delta, extended) = read_ext_if_marked(io, u24_be(&hdr[0..3])).await?;
    let length = u24_be(&hdr[3..6]) as usize;
    let type_id = hdr[6];
    Ok((delta, length, type_id, extended))
}

async fn read_fmt2_header<S: AsyncRead + Unpin>(
    io: &mut S,
) -> Result<(u32 /*delta*/, bool), ChunkError> {
    let mut hdr = [0u8; 3];
    io.read_exact(&mut hdr).await?;
    read_ext_if_marked(io, u24_be(&hdr[0..3])).await
}

fn u24_be(bytes: &[u8]) -> u32 {
    (bytes[0] as u32) << 16 | (bytes[1] as u32) << 8 | bytes[2] as u32
}

fn u24_bytes(value: u32) -> [u8; 3] {
    let v = value & 0x00FF_FFFF;
    [(v >> 16) as u8, (v >> 8) as u8, v as u8]
}

async fn write_basic_header<W: AsyncWrite + Unpin>(
    io: &mut W,
    fmt: u8,
    csid: u32,
) -> Result<(), ChunkError> {
    if csid < 64 {
        io.write_all(&[(fmt << 6) | (csid as u8)]).await?;
    } else if csid < 320 {
        io.write_all(&[fmt << 6, (csid - 64) as u8]).await?;
    } else {
        let v = csid - 64;
        io.write_all(&[(fmt << 6) | 1, (v & 0xFF) as u8, (v >> 8) as u8])
            .await?;
    }
    Ok(())
}

/// Writes `msg` as one fmt0 chunk followed by as many fmt3 continuation
/// chunks as needed to fit `chunk_size`-byte payload pieces.
pub async fn write_message<W>(
    io: &mut W,
    chunk_size: usize,
    csid: u32,
    msg: &RtmpMessage,
) -> Result<(), ChunkError>
where
    W: AsyncWrite + Unpin,
{
    let mut offset = 0usize;
    loop {
        if offset == 0 {
            write_basic_header(io, 0, csid).await?;
            let mut hdr = [0u8; 11];
            hdr[0..3].copy_from_slice(&u24_bytes(msg.timestamp));
            hdr[3..6].copy_from_slice(&u24_bytes(msg.payload.len() as u32));
            hdr[6] = msg.msg_type_id;
            hdr[7..11].copy_from_slice(&msg.msg_stream_id.to_le_bytes());
            io.write_all(&hdr).await?;
        } else {
            write_basic_header(io, 3, csid).await?;
        }
        let end = (offset + chunk_size).min(msg.payload.len());
        io.write_all(&msg.payload[offset..end]).await?;
        offset = end;
        if offset >= msg.payload.len() {
            break;
        }
    }
    io.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn writes_and_reads_single_chunk_message() {
        let (mut client_io, mut server_io) = tokio::io::duplex(4096);
        let msg = RtmpMessage {
            msg_type_id: 8,
            timestamp: 42,
            msg_stream_id: 1,
            payload: vec![1, 2, 3, 4],
        };
        write_message(&mut client_io, DEFAULT_CHUNK_SIZE, 5, &msg)
            .await
            .expect("write");

        let mut reader = ChunkReader::new();
        let got = reader.read_message(&mut server_io).await.expect("read");
        assert_eq!(got, msg);
    }

    #[tokio::test]
    async fn splits_and_reassembles_message_larger_than_chunk_size() {
        let (mut client_io, mut server_io) = tokio::io::duplex(8192);
        let payload: Vec<u8> = (0..300u32).map(|i| (i % 256) as u8).collect();
        let msg = RtmpMessage {
            msg_type_id: 9,
            timestamp: 7,
            msg_stream_id: 1,
            payload: payload.clone(),
        };
        write_message(&mut client_io, 128, 4, &msg)
            .await
            .expect("write");

        let mut reader = ChunkReader::new();
        let got = reader.read_message(&mut server_io).await.expect("read");
        assert_eq!(got.payload, payload);
        assert_eq!(got.msg_type_id, 9);
        assert_eq!(got.timestamp, 7);
    }

    #[tokio::test]
    async fn set_chunk_size_message_is_applied_and_not_surfaced() {
        let (mut client_io, mut server_io) = tokio::io::duplex(8192);

        // Send a "Set Chunk Size" control message (csid 2, stream 0) raising
        // the chunk size, then a media message bigger than the OLD default
        // but smaller than the NEW size — it must arrive in one chunk.
        let set_chunk = RtmpMessage {
            msg_type_id: MSG_TYPE_SET_CHUNK_SIZE,
            timestamp: 0,
            msg_stream_id: 0,
            payload: 1024u32.to_be_bytes().to_vec(),
        };
        write_message(&mut client_io, DEFAULT_CHUNK_SIZE, 2, &set_chunk)
            .await
            .expect("write set chunk size");

        let payload: Vec<u8> = (0..500u32).map(|i| (i % 256) as u8).collect();
        let media = RtmpMessage {
            msg_type_id: 9,
            timestamp: 1,
            msg_stream_id: 1,
            payload: payload.clone(),
        };
        write_message(&mut client_io, 1024, 4, &media)
            .await
            .expect("write media");

        let mut reader = ChunkReader::new();
        let got = reader.read_message(&mut server_io).await.expect("read");
        assert_eq!(got.payload, payload);
    }

    #[tokio::test]
    async fn interleaved_csids_are_demultiplexed_independently() {
        let (mut client_io, mut server_io) = tokio::io::duplex(8192);
        let audio = RtmpMessage {
            msg_type_id: 8,
            timestamp: 10,
            msg_stream_id: 1,
            payload: vec![9, 9, 9],
        };
        let video = RtmpMessage {
            msg_type_id: 9,
            timestamp: 11,
            msg_stream_id: 1,
            payload: vec![7, 7, 7, 7],
        };
        write_message(&mut client_io, DEFAULT_CHUNK_SIZE, 5, &audio)
            .await
            .expect("write audio");
        write_message(&mut client_io, DEFAULT_CHUNK_SIZE, 4, &video)
            .await
            .expect("write video");

        let mut reader = ChunkReader::new();
        let first = reader.read_message(&mut server_io).await.expect("read 1");
        let second = reader.read_message(&mut server_io).await.expect("read 2");
        assert_eq!(first, audio);
        assert_eq!(second, video);
    }

    #[test]
    fn u24_roundtrips() {
        assert_eq!(u24_be(&u24_bytes(0x00ABCDEF & 0x00FF_FFFF)), 0x00ABCDEF);
        assert_eq!(u24_be(&u24_bytes(0)), 0);
    }
}
