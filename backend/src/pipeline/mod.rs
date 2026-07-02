//! Ses akışı pipeline'ı — tek bir stream session'ının orchestration'ı.
//!
//! Akış (bkz. `docs/websocket-protocol.md` "Backend İçi Akış"):
//! ```text
//! binary frame → kullanım limiti kontrolü → STT (gateway) → çeviri (paralel) → subtitle mesajı
//! ```
//! VAD lokalde olduğundan backend sessiz frame almaz; gelen her frame işlenir.

use std::sync::Arc;
use std::time::Instant;

use crate::gateway::Gateway;
use crate::logging::{LogLevel, Logger};
use crate::usage::UsageTracker;
use crate::ws::protocol::{ErrorCode, ServerMessage, SubtitleSource};

/// Aktif bir stream session'ının durumu.
pub struct StreamSession {
    session_id: String,
    source_lang: String,
    target_langs: Vec<String>,
    sequence: u64,
    gateway: Arc<Gateway>,
    /// Overlay hub'ında bu session'ın altyazılarının yayınlandığı token
    /// (bkz. `overlay` modülü, `ws::protocol::ServerMessage::StreamAck`).
    overlay_token: String,
    user_id: String,
    /// Planın aylık saat limiti (`-1` = sınırsız). Bkz. `plans::PlanDefinition`.
    monthly_hours: i64,
    usage: Arc<UsageTracker>,
    /// Kullanım süresini duvar saatiyle ölçmek için son tick zamanı.
    last_tick: Instant,
    /// Merkezi log deposu (1.12) — pipeline/gateway olayları buraya yazılır.
    logger: Arc<Logger>,
}

impl StreamSession {
    /// `stream_start` üzerine yeni bir session kurar.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session_id: impl Into<String>,
        source_lang: impl Into<String>,
        target_langs: Vec<String>,
        gateway: Arc<Gateway>,
        overlay_token: impl Into<String>,
        user_id: impl Into<String>,
        monthly_hours: i64,
        usage: Arc<UsageTracker>,
        logger: Arc<Logger>,
    ) -> Self {
        StreamSession {
            session_id: session_id.into(),
            source_lang: source_lang.into(),
            target_langs,
            sequence: 0,
            gateway,
            overlay_token: overlay_token.into(),
            user_id: user_id.into(),
            monthly_hours,
            usage,
            last_tick: Instant::now(),
            logger,
        }
    }

    /// Bu session'ın overlay capability token'ı.
    pub fn overlay_token(&self) -> &str {
        &self.overlay_token
    }

    /// Bir ses parçasını işler ve bir mesaj üretir:
    /// - plan limiti dolmuşsa `Error(PlanLimitReached)` (STT/çeviri hiç çağrılmaz),
    /// - STT başarısızsa `None` (frame atlanır, akış bloklanmaz),
    /// - aksi halde `Subtitle`.
    pub async fn process_audio(&mut self, audio: &[u8]) -> Option<ServerMessage> {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_tick).as_secs_f64();
        self.last_tick = now;

        let status = self.usage.record(&self.user_id, elapsed, self.monthly_hours);
        if status.limit_reached {
            self.logger.log(
                LogLevel::Warn,
                "pipeline",
                "plan limit reached, session closing",
                Some(&self.user_id),
                Some(&self.session_id),
            );
            return Some(ServerMessage::error(
                ErrorCode::PlanLimitReached,
                "plan_limit_reached",
            ));
        }

        let transcript = match self.gateway.transcribe(audio, &self.source_lang).await {
            Ok(t) => t,
            Err(err) => {
                tracing::debug!(error = %err, "stt failed for frame, skipping");
                self.logger.log(
                    LogLevel::Warn,
                    "gateway",
                    &format!("stt failed for frame, skipping: {err}"),
                    Some(&self.user_id),
                    Some(&self.session_id),
                );
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

    fn test_logger() -> Arc<Logger> {
        Arc::new(Logger::in_memory(Arc::new(ServiceConfig::with_defaults())))
    }

    fn session(target_langs: Vec<String>, monthly_hours: i64) -> StreamSession {
        StreamSession::new(
            "s1",
            "tr",
            target_langs,
            test_gateway(),
            "tok-1",
            "user-1",
            monthly_hours,
            Arc::new(UsageTracker::new()),
            test_logger(),
        )
    }

    #[tokio::test]
    async fn produces_subtitle_with_translations() {
        let mut session = session(vec!["en".to_string(), "de".to_string()], -1);
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
        let mut session = session(vec!["en".to_string()], -1);
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
        let mut session = session(vec!["en".to_string()], -1);
        // MockStt boş payload'ta hata verir → subtitle yok.
        assert!(session.process_audio(&[]).await.is_none());
    }

    #[tokio::test]
    async fn plan_limit_reached_short_circuits_before_stt() {
        let usage = Arc::new(UsageTracker::new());
        // Limiti önceden doldur: monthly_hours=0 → ilk tick'te bile limit dolu sayılır.
        usage.record("user-1", 1.0, 0);
        let logger = test_logger();
        let mut session = StreamSession::new(
            "s1",
            "tr",
            vec!["en".to_string()],
            test_gateway(),
            "tok-1",
            "user-1",
            0,
            usage,
            Arc::clone(&logger),
        );
        let msg = session.process_audio(&[1, 2, 3]).await.expect("error message");
        assert!(matches!(
            msg,
            ServerMessage::Error { code: 4003, .. }
        ));

        // Merkezi log deposuna warn kaydı düşer (1.12) — kullanıcı+session bağlı.
        let logs = logger.query(&crate::logging::LogQuery {
            level: Some(LogLevel::Warn),
            service: Some("pipeline".into()),
            ..Default::default()
        });
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].user_id.as_deref(), Some("user-1"));
        assert_eq!(logs[0].session_id.as_deref(), Some("s1"));
    }

    #[tokio::test]
    async fn stt_failure_is_logged_to_central_store() {
        let logger = test_logger();
        let mut session = StreamSession::new(
            "s1",
            "tr",
            vec!["en".to_string()],
            test_gateway(),
            "tok-1",
            "user-1",
            -1,
            Arc::new(UsageTracker::new()),
            Arc::clone(&logger),
        );
        // MockStt boş payload'ta hata verir → gateway hatası loglanır.
        assert!(session.process_audio(&[]).await.is_none());

        let logs = logger.query(&crate::logging::LogQuery {
            service: Some("gateway".into()),
            level: Some(LogLevel::Warn),
            ..Default::default()
        });
        assert_eq!(logs.len(), 1);
        assert!(logs[0].message.contains("stt failed"));
    }
}
