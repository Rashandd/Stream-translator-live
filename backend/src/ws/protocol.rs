//! SDK ↔ Backend kontrol mesajları (Text/JSON frame'ler).
//!
//! Bkz. `docs/websocket-protocol.md`. Tüm mesajlar `type` alanıyla etiketlenir.

use serde::{Deserialize, Serialize};

/// Ses formatı tanımı (`stream_start` içinde).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioFormat {
    pub codec: String,
    pub sample_rate: u32,
    pub channels: u8,
}

/// SDK → Backend yönündeki mesajlar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    },
    StreamStop {
        session_id: String,
    },
    Ping {
        ts: i64,
    },
}

/// Bir altyazının kaynak-dil bölümü.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubtitleSource {
    pub lang: String,
    pub text: String,
    pub is_final: bool,
}

/// Tek bir çeviri satırı.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Translation {
    pub lang: String,
    pub text: String,
}

/// Backend → SDK yönündeki mesajlar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
        /// Tahmin edilemez overlay capability token'ı — OBS Browser Source
        /// URL'sinde kullanılır (bkz. `overlay` modülü). JWT gerektirmez.
        overlay_token: String,
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
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
}

/// Protokol error kodları (bkz. spec tablosu).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    InvalidToken = 4001,
    AudioBeforeStart = 4002,
    PlanLimitReached = 4003,
    UnsupportedCodec = 4004,
    SttProviderError = 4005,
    TranslateProviderError = 4006,
    InternalError = 5000,
}

impl ServerMessage {
    /// Verilen kod ve mesajla bir `Error` mesajı kurar.
    pub fn error(code: ErrorCode, message: impl Into<String>) -> Self {
        ServerMessage::Error {
            code: code as u16,
            message: message.into(),
            detail: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_auth() {
        let json = r#"{"type":"auth","token":"abc.def.ghi"}"#;
        let msg: ClientMessage = serde_json::from_str(json).expect("parse auth");
        assert_eq!(
            msg,
            ClientMessage::Auth {
                token: "abc.def.ghi".to_string()
            }
        );
    }

    #[test]
    fn deserializes_stream_start() {
        let json = r#"{
            "type":"stream_start",
            "session_id":"s1",
            "source_lang":"tr",
            "target_langs":["en","de"],
            "audio_format":{"codec":"opus","sample_rate":16000,"channels":1}
        }"#;
        let msg: ClientMessage = serde_json::from_str(json).expect("parse stream_start");
        match msg {
            ClientMessage::StreamStart {
                session_id,
                source_lang,
                target_langs,
                audio_format,
            } => {
                assert_eq!(session_id, "s1");
                assert_eq!(source_lang, "tr");
                assert_eq!(target_langs, vec!["en", "de"]);
                assert_eq!(audio_format.sample_rate, 16000);
            }
            other => panic!("beklenmeyen mesaj: {other:?}"),
        }
    }

    #[test]
    fn serializes_session_ready_with_type_tag() {
        let msg = ServerMessage::SessionReady {
            session_id: "s1".into(),
            user_id: "u1".into(),
            plan: "free".into(),
            remaining_seconds: 18000,
        };
        let json = serde_json::to_value(&msg).expect("serialize");
        assert_eq!(json["type"], "session_ready");
        assert_eq!(json["remaining_seconds"], 18000);
    }

    #[test]
    fn error_helper_sets_code_and_omits_null_detail() {
        let msg = ServerMessage::error(ErrorCode::InvalidToken, "invalid_token");
        let json = serde_json::to_value(&msg).expect("serialize");
        assert_eq!(json["type"], "error");
        assert_eq!(json["code"], 4001);
        assert_eq!(json["message"], "invalid_token");
        assert!(json.get("detail").is_none(), "null detail atlanmalı");
    }

    #[test]
    fn ping_pong_roundtrip() {
        let ping = r#"{"type":"ping","ts":1719600000000}"#;
        let msg: ClientMessage = serde_json::from_str(ping).expect("parse ping");
        assert_eq!(msg, ClientMessage::Ping { ts: 1719600000000 });
    }
}
