//! Ses akışı pipeline'ı — tek bir stream session'ının orchestration'ı.
//!
//! Akış (bkz. `docs/websocket-protocol.md` "Backend İçi Akış"):
//! ```text
//! binary frame → STT (gateway) → çeviri (paralel) → subtitle mesajı
//! ```
//! VAD lokalde olduğundan backend sessiz frame almaz; gelen her frame işlenir.

use std::sync::Arc;

use crate::gateway::Gateway;
use crate::ws::protocol::{ServerMessage, SubtitleSource};

/// Aktif bir stream session'ının durumu.
pub struct StreamSession {
    session_id: String,
    source_lang: String,
    target_langs: Vec<String>,
    sequence: u64,
    gateway: Arc<Gateway>,
}

impl StreamSession {
    /// `stream_start` üzerine yeni bir session kurar.
    pub fn new(
        session_id: impl Into<String>,
        source_lang: impl Into<String>,
        target_langs: Vec<String>,
        gateway: Arc<Gateway>,
    ) -> Self {
        StreamSession {
            session_id: session_id.into(),
            source_lang: source_lang.into(),
            target_langs,
            sequence: 0,
            gateway,
        }
    }

    /// Bir ses parçasını işler ve (varsa) bir `subtitle` mesajı üretir.
    ///
    /// STT başarısızsa `None` döner (frame atlanır, akış bloklanmaz).
    pub async fn process_audio(&mut self, audio: &[u8]) -> Option<ServerMessage> {
        let transcript = match self.gateway.transcribe(audio, &self.source_lang).await {
            Ok(t) => t,
            Err(err) => {
                tracing::debug!(error = %err, "stt failed for frame, skipping");
                return None;
            }
        };

        let translations = self
            .gateway
            .translate_all(&transcript.text, &self.source_lang, &self.target_langs)
            .await;

        let ts = chrono::Utc::now().timestamp_millis();
        let seq = self.next_sequence();

        Some(ServerMessage::Subtitle {
            session_id: self.session_id.clone(),
            sequence: seq,
            source: SubtitleSource {
                lang: self.source_lang.clone(),
                text: transcript.text,
                is_final: transcript.is_final,
            },
            translations,
            ts_start_ms: ts,
            ts_end_ms: ts,
        })
    }

    /// Bir sonraki sıra numarasını döndürür (her altyazıda artar).
    fn next_sequence(&mut self) -> u64 {
        let seq = self.sequence;
        self.sequence += 1;
        seq
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ServiceConfig;
    use crate::gateway::{stt::MockStt, translate::MockTranslate};

    fn test_gateway() -> Arc<Gateway> {
        let config = Arc::new(ServiceConfig::with_defaults());
        let mut gw = Gateway::new(config);
        gw.register_stt(Arc::new(MockStt::new("deepgram")));
        gw.register_translate(Arc::new(MockTranslate::new("deepl")));
        Arc::new(gw)
    }

    #[tokio::test]
    async fn produces_subtitle_with_translations() {
        let mut session = StreamSession::new(
            "s1",
            "tr",
            vec!["en".to_string(), "de".to_string()],
            test_gateway(),
        );
        let msg = session.process_audio(&[1, 2, 3]).await.expect("subtitle");
        match msg {
            ServerMessage::Subtitle {
                sequence,
                source,
                translations,
                ..
            } => {
                assert_eq!(sequence, 0);
                assert_eq!(source.lang, "tr");
                assert_eq!(translations.len(), 2);
            }
            other => panic!("beklenmeyen mesaj: {other:?}"),
        }
    }

    #[tokio::test]
    async fn sequence_increments_per_subtitle() {
        let mut session =
            StreamSession::new("s1", "tr", vec!["en".to_string()], test_gateway());
        let first = session.process_audio(&[1]).await.expect("first");
        let second = session.process_audio(&[2]).await.expect("second");
        let seq = |m: &ServerMessage| match m {
            ServerMessage::Subtitle { sequence, .. } => *sequence,
            _ => panic!("subtitle bekleniyordu"),
        };
        assert_eq!(seq(&first), 0);
        assert_eq!(seq(&second), 1);
    }

    #[tokio::test]
    async fn empty_audio_skipped() {
        let mut session =
            StreamSession::new("s1", "tr", vec!["en".to_string()], test_gateway());
        // MockStt boş payload'ta hata verir → subtitle yok.
        assert!(session.process_audio(&[]).await.is_none());
    }
}
