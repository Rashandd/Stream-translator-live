//! Cloud WebSocket client — sends ONLY audio to the backend, receives subtitles.
//!
//! Transport is TLS 1.3 (`wss://`). Per `docs/security.md` / CLAUDE.md the audio
//! stream is NOT additionally AES-encrypted — TLS transit is sufficient, a second
//! layer is wasted CPU. Video never leaves the machine; only ~50-128 Kbps audio.
//!
//! mTLS (client certificate) is required against production per security policy;
//! the custom TLS connector wiring is a TODO below (needs the SDK client cert).

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, Utf8Bytes},
    MaybeTlsStream, WebSocketStream,
};

use super::audio_frame::AudioCodec;
use super::stream::{StreamConfig, SttSource};

/// Audio format block in `stream_start`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioFormat {
    pub codec: String,
    pub sample_rate: u32,
    pub channels: u8,
}

/// SDK → backend messages.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Auth {
        token: String,
    },
    StreamStart {
        session_id: String,
        source_lang: String,
        target_langs: Vec<String>,
        audio_format: AudioFormat,
        stt_source: SttSource,
    },
    StreamStop {
        session_id: String,
    },
    /// Local STT mode (phase 2.6b): the SDK transcribed the audio itself and
    /// sends finished text instead of audio frames — the backend skips STT.
    Transcript {
        session_id: String,
        text: String,
        is_final: bool,
    },
    Ping {
        ts: i64,
    },
}

/// One translation line.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Translation {
    pub lang: String,
    pub text: String,
}

/// Source-language portion of a subtitle.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SubtitleSource {
    pub lang: String,
    pub text: String,
    pub is_final: bool,
}

/// backend → SDK messages.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    SessionReady {
        session_id: String,
        user_id: String,
        plan: String,
        remaining_seconds: i64,
    },
    StreamAck {
        session_id: String,
        status: String,
    },
    Subtitle {
        session_id: String,
        sequence: u64,
        source: SubtitleSource,
        translations: Vec<Translation>,
        ts_start_ms: i64,
        ts_end_ms: i64,
    },
    Pong {
        ts: i64,
    },
    UsageUpdate {
        used_seconds: i64,
        remaining_seconds: i64,
    },
    Error {
        code: u16,
        message: String,
        #[serde(default)]
        detail: Option<String>,
    },
}

/// Cloud client errors.
#[derive(Debug, thiserror::Error)]
pub enum CloudError {
    #[error("connection failed: {0}")]
    Connect(String),
    #[error("websocket error: {0}")]
    Ws(String),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("connection closed by server")]
    Closed,
    #[error("unexpected message, expected {expected}")]
    Unexpected { expected: &'static str },
    #[error("server error {code}: {message}")]
    Server { code: u16, message: String },
}

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Connected cloud session. Sends only audio; receives subtitles/usage.
pub struct CloudClient {
    socket: Socket,
}

impl CloudClient {
    /// Connects to the backend WebSocket endpoint (`ws://` or `wss://`).
    ///
    /// TODO(mTLS): for `wss://` against production, supply a rustls connector
    /// carrying the SDK client certificate (`connect_async_tls_with_config`).
    pub async fn connect(url: &str) -> Result<Self, CloudError> {
        let (socket, _resp) = connect_async(url)
            .await
            .map_err(|e| CloudError::Connect(e.to_string()))?;
        Ok(CloudClient { socket })
    }

    /// Sends `auth` and waits for `session_ready` (or surfaces server error).
    pub async fn authenticate(&mut self, token: &str) -> Result<ServerMessage, CloudError> {
        self.send(&ClientMessage::Auth {
            token: token.to_string(),
        })
        .await?;
        let msg = self.next_message().await?.ok_or(CloudError::Closed)?;
        match msg {
            ServerMessage::SessionReady { .. } => Ok(msg),
            ServerMessage::Error { code, message, .. } => Err(CloudError::Server { code, message }),
            _ => Err(CloudError::Unexpected {
                expected: "session_ready",
            }),
        }
    }

    /// Sends `stream_start` and waits for `stream_ack`.
    pub async fn start_stream(&mut self, cfg: &StreamConfig) -> Result<(), CloudError> {
        self.send(&ClientMessage::StreamStart {
            session_id: cfg.session_id.clone(),
            source_lang: cfg.source_lang.clone(),
            target_langs: cfg.target_langs.clone(),
            audio_format: AudioFormat {
                codec: codec_name(cfg.codec).to_string(),
                sample_rate: cfg.sample_rate,
                channels: cfg.channels,
            },
            stt_source: cfg.stt_source,
        })
        .await?;
        match self.next_message().await?.ok_or(CloudError::Closed)? {
            ServerMessage::StreamAck { .. } => Ok(()),
            ServerMessage::Error { code, message, .. } => Err(CloudError::Server { code, message }),
            _ => Err(CloudError::Unexpected {
                expected: "stream_ack",
            }),
        }
    }

    /// Sends one binary audio frame (already SDAU-framed).
    pub async fn send_audio(&mut self, frame: &[u8]) -> Result<(), CloudError> {
        self.socket
            .send(Message::Binary(frame.to_vec().into()))
            .await
            .map_err(|e| CloudError::Ws(e.to_string()))
    }

    /// Sends one locally-transcribed line (local STT mode, phase 2.6b).
    /// Requires the stream to have been started with `SttSource::Local`;
    /// the backend rejects it otherwise (error 4007).
    pub async fn send_transcript(
        &mut self,
        session_id: &str,
        text: &str,
        is_final: bool,
    ) -> Result<(), CloudError> {
        self.send(&ClientMessage::Transcript {
            session_id: session_id.to_string(),
            text: text.to_string(),
            is_final,
        })
        .await
    }

    /// Sends `stream_stop`.
    pub async fn stop_stream(&mut self, session_id: &str) -> Result<(), CloudError> {
        self.send(&ClientMessage::StreamStop {
            session_id: session_id.to_string(),
        })
        .await
    }

    /// Reads the next text control message (skips ping/pong frames).
    pub async fn next_message(&mut self) -> Result<Option<ServerMessage>, CloudError> {
        while let Some(frame) = self.socket.next().await {
            let frame = frame.map_err(|e| CloudError::Ws(e.to_string()))?;
            match frame {
                Message::Text(text) => {
                    let msg = serde_json::from_str(text.as_str())?;
                    return Ok(Some(msg));
                }
                Message::Close(_) => return Ok(None),
                _ => continue,
            }
        }
        Ok(None)
    }

    async fn send(&mut self, msg: &ClientMessage) -> Result<(), CloudError> {
        let json = serde_json::to_string(msg)?;
        self.socket
            .send(Message::Text(Utf8Bytes::from(json)))
            .await
            .map_err(|e| CloudError::Ws(e.to_string()))
    }
}

fn codec_name(codec: AudioCodec) -> &'static str {
    codec.as_protocol_name()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_auth_with_type_tag() {
        let json = serde_json::to_value(ClientMessage::Auth {
            token: "t".into(),
        })
        .unwrap();
        assert_eq!(json["type"], "auth");
        assert_eq!(json["token"], "t");
    }

    #[test]
    fn serializes_stream_start_audio_format() {
        let msg = ClientMessage::StreamStart {
            session_id: "s1".into(),
            source_lang: "tr".into(),
            target_langs: vec!["en".into()],
            audio_format: AudioFormat {
                codec: "pcm_s16le".into(),
                sample_rate: 16000,
                channels: 1,
            },
            stt_source: SttSource::Cloud,
        };
        let json = serde_json::to_value(msg).unwrap();
        assert_eq!(json["type"], "stream_start");
        assert_eq!(json["audio_format"]["sample_rate"], 16000);
        assert_eq!(json["stt_source"], "cloud");
    }

    #[test]
    fn serializes_stream_start_local_stt_source() {
        let msg = ClientMessage::StreamStart {
            session_id: "s1".into(),
            source_lang: "tr".into(),
            target_langs: vec!["en".into()],
            audio_format: AudioFormat {
                codec: "opus".into(),
                sample_rate: 16000,
                channels: 1,
            },
            stt_source: SttSource::Local,
        };
        let json = serde_json::to_value(msg).unwrap();
        assert_eq!(json["stt_source"], "local");
    }

    #[test]
    fn serializes_transcript_with_type_tag() {
        let msg = ClientMessage::Transcript {
            session_id: "s1".into(),
            text: "merhaba dünya".into(),
            is_final: true,
        };
        let json = serde_json::to_value(msg).unwrap();
        assert_eq!(json["type"], "transcript");
        assert_eq!(json["text"], "merhaba dünya");
        assert_eq!(json["is_final"], true);
    }

    #[test]
    fn deserializes_subtitle() {
        let raw = r#"{
            "type":"subtitle","session_id":"s1","sequence":3,
            "source":{"lang":"tr","text":"merhaba","is_final":true},
            "translations":[{"lang":"en","text":"hello"}],
            "ts_start_ms":1,"ts_end_ms":2
        }"#;
        let msg: ServerMessage = serde_json::from_str(raw).unwrap();
        match msg {
            ServerMessage::Subtitle {
                sequence,
                translations,
                ..
            } => {
                assert_eq!(sequence, 3);
                assert_eq!(translations[0].lang, "en");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn deserializes_error_without_detail() {
        let raw = r#"{"type":"error","code":4001,"message":"invalid_token"}"#;
        let msg: ServerMessage = serde_json::from_str(raw).unwrap();
        assert!(matches!(msg, ServerMessage::Error { code: 4001, .. }));
    }
}
