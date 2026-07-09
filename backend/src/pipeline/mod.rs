//! Ses akışı pipeline'ı — tek bir stream session'ının orchestration'ı.
//!
//! Akış (bkz. `docs/websocket-protocol.md` "Backend İçi Akış"):
//! ```text
//! cloud: binary frame → kullanım limiti → STT (gateway) → çeviri (paralel) → subtitle
//! local: transcript mesajı → kullanım limiti → (STT atlanır) → çeviri → subtitle
//! ```
//! VAD lokalde olduğundan backend sessiz frame almaz; gelen her frame işlenir.
//! İki yol da aynı `Subtitle` çıktısını üretir (Faz 2.6b).

use std::sync::Arc;
use std::time::Instant;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

use crate::gateway::Gateway;
use crate::logging::{LogLevel, Logger};
use crate::usage::UsageTracker;
use crate::voice_models::VoiceModelStore;
use crate::ws::protocol::{
    DubbedAudioTrack, ErrorCode, ServerMessage, SttSource, SubtitleSource, Translation, TtsSource,
};

/// ElevenLabs `synthesize` çağrısına açıkça verdiğimiz sabit çıktı formatının
/// (bkz. `gateway::elevenlabs` — `pcm_24000`: 24000 Hz × 16 bit × 1 kanal) bit
/// hızı. TTS kota tüketimi (Faz 2.2) gerçek ses süresini bu sabitten hesaplar
/// (`bytes*8/bitrate`) — tahmini bir karakter/saniye oranı değil, sağlayıcıya
/// EXPLICIT istenen formattan türetilen kesin değer. Farklı bir cloud
/// sağlayıcı eklenirse bu sabit sağlayıcı başına bir değere taşınmalı
/// (şimdilik tek cloud sağlayıcı ElevenLabs olduğu için global sabit yeterli).
const ASSUMED_TTS_BITRATE_BPS: f64 = 384_000.0;

/// Ham ses baytından saniye hesaplar (`bytes*8/bitrate`).
fn tts_audio_seconds(audio_len_bytes: usize) -> f64 {
    (audio_len_bytes as f64 * 8.0) / ASSUMED_TTS_BITRATE_BPS
}

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
    /// `stream_start.stt_source` — `Local` ise SDK `transcript` mesajı
    /// gönderebilir (Faz 2.6b), `Cloud` session'da transcript reddedilir.
    stt_source: SttSource,
    /// `stream_start.tts_source` (Faz 2.2b): `Cloud` ise backend hedef dil
    /// başına onaylı+aktif bir ses modeli varsa gerçekten sentezler (Faz
    /// 2.2); `Local` ise SDK kendi motoruyla `translations` metnini
    /// sentezler, backend hiç ses üretmez.
    tts_source: TtsSource,
    /// Admin'in (kullanıcı, hedef dil) başına aktif kıldığı onaylı ses
    /// modelleri — cloud TTS dispatch'i (Faz 2.2) bunu okur.
    voice_models: Arc<VoiceModelStore>,
    /// Backend'in kendi işlediği (indirimsiz) STT saatinin kredi ağırlığı
    /// (`stt_policy::cloud_credit_weight`, roadmap "Fiyatlandırma — Kredi
    /// Sistemi"). Varsayılan `1.0` — belirtilmezse eski sabit davranış.
    stt_credit_weight: f64,
    /// Aynı şey TTS/ses klonlama için (`tts_policy::cloud_credit_weight`).
    tts_credit_weight: f64,
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
        stt_source: SttSource,
        tts_source: TtsSource,
        voice_models: Arc<VoiceModelStore>,
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
            stt_source,
            tts_source,
            voice_models,
            stt_credit_weight: 1.0,
            tts_credit_weight: 1.0,
        }
    }

    /// Backend-side (indirimsiz) STT/TTS kredi ağırlıklarını ayarlar —
    /// admin'in `stt.cloud.credit_weight`/`tts.cloud.credit_weight`
    /// config'inden okunan değerler (`ws::handle_control` StreamStart'ta
    /// çözülür). Çağrılmazsa ikisi de `1.0` (eski sabit davranış).
    pub fn with_credit_weights(mut self, stt: f64, tts: f64) -> Self {
        self.stt_credit_weight = stt;
        self.tts_credit_weight = tts;
        self
    }

    /// Bu session'ın overlay capability token'ı.
    pub fn overlay_token(&self) -> &str {
        &self.overlay_token
    }

    /// Bu session'ın STT kaynağı (`stream_start.stt_source`).
    pub fn stt_source(&self) -> SttSource {
        self.stt_source
    }

    /// Bu session'ın TTS kaynağı (`stream_start.tts_source`, Faz 2.2b).
    pub fn tts_source(&self) -> TtsSource {
        self.tts_source
    }

    /// Bir ses parçasını işler ve bir mesaj üretir:
    /// - plan limiti dolmuşsa `Error(PlanLimitReached)` (STT/çeviri hiç çağrılmaz),
    /// - STT başarısızsa `None` (frame atlanır, akış bloklanmaz),
    /// - aksi halde `Subtitle`.
    pub async fn process_audio(&mut self, audio: &[u8]) -> Option<ServerMessage> {
        // Ses yolu her zaman backend'in kendi kredi ağırlığıyla tarifelenir:
        // bulut/self-hosted STT çalışıyor demektir — yerel kota indirimi
        // uygulanmaz (o yalnız process_transcript/yerel yolda geçerli).
        if let Some(limit_error) = self.tick_usage(self.stt_credit_weight) {
            return Some(limit_error);
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

        Some(self.subtitle_for(transcript.text, transcript.is_final).await)
    }

    /// Lokalde yazıya dökülmüş bir satırı işler (Faz 2.6b): STT adımı atlanır,
    /// metin doğrudan çevrilir. `Subtitle` çıktısı ses yoluyla birebir aynıdır;
    /// kullanım ise `usage_multiplier` ile indirimli işlenir (yerel STT kota
    /// teşviki — docs/local-stt-plan.md §3). Boş/boşluk metin sessizce atlanır.
    pub async fn process_transcript(
        &mut self,
        text: &str,
        is_final: bool,
        usage_multiplier: f64,
    ) -> Option<ServerMessage> {
        if let Some(limit_error) = self.tick_usage(usage_multiplier) {
            return Some(limit_error);
        }

        let text = text.trim();
        if text.is_empty() {
            tracing::debug!("empty local transcript, skipping");
            return None;
        }

        Some(self.subtitle_for(text.to_string(), is_final).await)
    }

    /// Duvar saatiyle geçen süreyi `scale` çarpanıyla kotaya işler; plan
    /// limiti dolduysa `Error(PlanLimitReached)` döndürür (iki pipeline
    /// yolunun ortak girişi — ses 1.0, yerel transcript indirimli).
    fn tick_usage(&mut self, scale: f64) -> Option<ServerMessage> {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_tick).as_secs_f64();
        self.last_tick = now;

        let status = self
            .usage
            .record(&self.user_id, elapsed * scale, self.monthly_hours);
        if !status.limit_reached {
            return None;
        }
        self.logger.log(
            LogLevel::Warn,
            "pipeline",
            "plan limit reached, session closing",
            Some(&self.user_id),
            Some(&self.session_id),
        );
        Some(ServerMessage::error(
            ErrorCode::PlanLimitReached,
            "plan_limit_reached",
        ))
    }

    /// Kaynak metni tüm hedef dillere çevirip `Subtitle` mesajını kurar —
    /// cloud (STT çıktısı) ve local (transcript mesajı) yollarının ortak sonu.
    /// `is_final` ise ve `tts_source: Cloud` ise, hedef dil başına onaylı+aktif
    /// bir ses modeli varsa gerçekten sentezleyip `dubbed_audio`'ya ekler
    /// (Faz 2.2). Interim sonuçlar hiç dublajlanmaz (gereksiz sağlayıcı
    /// çağrısı/kota tüketimi olurdu).
    async fn subtitle_for(&mut self, text: String, is_final: bool) -> ServerMessage {
        let translations = self
            .gateway
            .translate_all(&text, &self.source_lang, &self.target_langs)
            .await;

        let dubbed_audio = if is_final && self.tts_source == TtsSource::Cloud {
            self.synthesize_dubs(&translations).await
        } else {
            Vec::new()
        };

        let ts = chrono::Utc::now().timestamp_millis();
        let seq = self.next_sequence();

        ServerMessage::Subtitle {
            session_id: self.session_id.clone(),
            sequence: seq,
            source: SubtitleSource {
                lang: self.source_lang.clone(),
                text,
                is_final,
            },
            translations,
            dubbed_audio,
            ts_start_ms: ts,
            ts_end_ms: ts,
        }
    }

    /// Her çeviri için, o kullanıcı+hedef dil için admin'in aktif kıldığı
    /// onaylı bir model varsa gerçekten sentezler. Model yoksa/sağlayıcı
    /// hata verirse o dil sessizce atlanır (best-effort — translate_all'ın
    /// başarısız dilleri atlaması ile aynı desen); bir dilin başarısızlığı
    /// diğerlerini engellemez. Başarılı her sentez, gerçek ses baytından
    /// hesaplanan saniyeyi `tts_credit_weight` ile ölçekleyip (varsayılan
    /// 1.0 — indirimsiz/tam maliyetli, admin farklı bir self-hosted model
    /// için değiştirebilir) aynı kullanım havuzuna (`UsageTracker`, STT ile
    /// PAYLAŞILAN) işler; bu havuzun limiti STT'nin bir sonraki tick'inde
    /// zaten kontrol edilir, bu yüzden burada ayrıca session kapatılmaz.
    async fn synthesize_dubs(&mut self, translations: &[Translation]) -> Vec<DubbedAudioTrack> {
        let mut tracks = Vec::with_capacity(translations.len());
        for t in translations {
            let Some(model) = self.voice_models.active_model_for(&self.user_id, &t.lang) else {
                continue;
            };
            match self.gateway.synthesize(&model.provider_voice_id, &t.text).await {
                Ok(audio) => {
                    let seconds = tts_audio_seconds(audio.len());
                    self.usage.record(
                        &self.user_id,
                        seconds * self.tts_credit_weight,
                        self.monthly_hours,
                    );
                    tracks.push(DubbedAudioTrack {
                        lang: t.lang.clone(),
                        audio_base64: B64.encode(&audio),
                    });
                }
                Err(err) => {
                    tracing::debug!(error = %err, lang = %t.lang, "tts synthesize failed, skipping dub");
                    self.logger.log(
                        LogLevel::Warn,
                        "gateway",
                        &format!("tts synthesize failed for {}, skipping dub: {err}", t.lang),
                        Some(&self.user_id),
                        Some(&self.session_id),
                    );
                }
            }
        }
        tracks
    }

    /// Bir sonraki sıra numarasını döndürür (her altyazıda artar).
    fn next_sequence(&mut self) -> u64 {
        let seq = self.sequence;
        self.sequence += 1;
        seq
    }

    /// Test yardımcısı: son tick'i geçmişe çeker — duvar saati tabanlı
    /// kullanım ölçümünü deterministik test etmek için.
    #[cfg(test)]
    fn backdate_last_tick(&mut self, secs: f64) {
        self.last_tick = Instant::now() - std::time::Duration::from_secs_f64(secs);
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

    fn test_voice_models() -> Arc<VoiceModelStore> {
        Arc::new(VoiceModelStore::in_memory())
    }

    fn session(target_langs: Vec<String>, monthly_hours: i64) -> StreamSession {
        session_with_source(target_langs, monthly_hours, SttSource::Cloud)
    }

    fn session_with_source(
        target_langs: Vec<String>,
        monthly_hours: i64,
        stt_source: SttSource,
    ) -> StreamSession {
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
            stt_source,
            TtsSource::Cloud,
            test_voice_models(),
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
            SttSource::Cloud,
            TtsSource::Cloud,
            test_voice_models(),
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
            SttSource::Cloud,
            TtsSource::Cloud,
            test_voice_models(),
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

    // ── Local STT yolu (Faz 2.6b) ────────────────────────────────────────

    #[tokio::test]
    async fn local_transcript_skips_stt_and_translates() {
        let mut session = session_with_source(
            vec!["en".to_string(), "de".to_string()],
            -1,
            SttSource::Local,
        );
        let msg = session
            .process_transcript("bugün çok iyi bir gün", true, 1.0)
            .await
            .expect("subtitle");
        match msg {
            ServerMessage::Subtitle {
                source,
                translations,
                ..
            } => {
                // Metin STT'den geçmeden aynen kaynak olarak kullanılır.
                assert_eq!(source.text, "bugün çok iyi bir gün");
                assert_eq!(source.lang, "tr");
                assert!(source.is_final);
                assert_eq!(translations.len(), 2);
            }
            other => panic!("beklenmeyen mesaj: {other:?}"),
        }
    }

    #[tokio::test]
    async fn local_transcript_preserves_interim_flag_and_trims() {
        let mut session = session_with_source(vec!["en".to_string()], -1, SttSource::Local);
        let msg = session
            .process_transcript("  merha  ", false, 1.0)
            .await
            .expect("subtitle");
        match msg {
            ServerMessage::Subtitle { source, .. } => {
                assert_eq!(source.text, "merha");
                assert!(!source.is_final);
            }
            other => panic!("beklenmeyen mesaj: {other:?}"),
        }
    }

    #[tokio::test]
    async fn empty_local_transcript_skipped() {
        let mut session = session_with_source(vec!["en".to_string()], -1, SttSource::Local);
        assert!(session.process_transcript("   ", true, 1.0).await.is_none());
        assert!(session.process_transcript("", true, 1.0).await.is_none());
    }

    #[tokio::test]
    async fn transcript_and_audio_share_sequence_counter() {
        // Local session'da bulut STT fallback'i de geçerli — iki yol aynı
        // sıra sayacını paylaşır (SDK sıralamayı sequence ile kurar).
        let mut session = session_with_source(vec!["en".to_string()], -1, SttSource::Local);
        let first = session.process_audio(&[1]).await.expect("audio subtitle");
        let second = session
            .process_transcript("merhaba", true, 1.0)
            .await
            .expect("transcript subtitle");
        let seq = |m: &ServerMessage| match m {
            ServerMessage::Subtitle { sequence, .. } => *sequence,
            _ => panic!("subtitle bekleniyordu"),
        };
        assert_eq!(seq(&first), 0);
        assert_eq!(seq(&second), 1);
    }

    #[tokio::test]
    async fn local_transcript_charges_discounted_usage() {
        // 10 sn geçmiş × 0.5 çarpan → kotadan 5 sn düşer (docs/local-stt-plan.md §3).
        let usage = Arc::new(UsageTracker::new());
        let mut session = StreamSession::new(
            "s1",
            "tr",
            vec!["en".to_string()],
            test_gateway(),
            "tok-1",
            "user-1",
            -1,
            Arc::clone(&usage),
            test_logger(),
            SttSource::Local,
            TtsSource::Cloud,
            test_voice_models(),
        );
        session.backdate_last_tick(10.0);
        session
            .process_transcript("merhaba", true, 0.5)
            .await
            .expect("subtitle");
        assert_eq!(usage.status("user-1", -1).used_seconds, 5);
    }

    #[tokio::test]
    async fn audio_path_charges_full_usage() {
        // Ses yolu (bulut STT veya local fallback'i) her zaman 1.0 tarife.
        let usage = Arc::new(UsageTracker::new());
        let mut session = StreamSession::new(
            "s1",
            "tr",
            vec!["en".to_string()],
            test_gateway(),
            "tok-1",
            "user-1",
            -1,
            Arc::clone(&usage),
            test_logger(),
            SttSource::Local,
            TtsSource::Cloud,
            test_voice_models(),
        );
        session.backdate_last_tick(10.0);
        session.process_audio(&[1, 2, 3]).await.expect("subtitle");
        assert_eq!(usage.status("user-1", -1).used_seconds, 10);
    }

    #[tokio::test]
    async fn audio_path_applies_stt_credit_weight() {
        // Admin stt.cloud.credit_weight'i 2.0'a ayarlarsa (ör. pahalı bir
        // self-hosted model), ses yolu tam saniyeyi değil ağırlıklanmış
        // saniyeyi kotadan düşmeli.
        let usage = Arc::new(UsageTracker::new());
        let mut session = StreamSession::new(
            "s1",
            "tr",
            vec!["en".to_string()],
            test_gateway(),
            "tok-1",
            "user-1",
            -1,
            Arc::clone(&usage),
            test_logger(),
            SttSource::Local,
            TtsSource::Cloud,
            test_voice_models(),
        )
        .with_credit_weights(2.0, 1.0);
        session.backdate_last_tick(10.0);
        session.process_audio(&[1, 2, 3]).await.expect("subtitle");
        assert_eq!(usage.status("user-1", -1).used_seconds, 20);
    }

    #[tokio::test]
    async fn local_transcript_respects_plan_limit() {
        let usage = Arc::new(UsageTracker::new());
        usage.record("user-1", 1.0, 0);
        let mut session = StreamSession::new(
            "s1",
            "tr",
            vec!["en".to_string()],
            test_gateway(),
            "tok-1",
            "user-1",
            0,
            usage,
            test_logger(),
            SttSource::Local,
            TtsSource::Cloud,
            test_voice_models(),
        );
        let msg = session
            .process_transcript("merhaba", true, 1.0)
            .await
            .expect("error message");
        assert!(matches!(msg, ServerMessage::Error { code: 4003, .. }));
    }

    // ── Gerçek zamanlı TTS dispatch (Faz 2.2) ────────────────────────────

    use async_trait::async_trait;
    use crate::gateway::tts::{MockTts, TtsError, TtsProvider};
    use crate::voice_models::{
        LanguageMode, VoiceConsent, VoiceModel, VoiceModelStatus, VoiceMode, VoiceSource,
    };

    #[test]
    fn tts_audio_seconds_uses_explicit_bitrate() {
        // 384_000 bps / 8 = 48_000 bayt/sn — bitrate açıkça istendiği için
        // (gateway::elevenlabs `pcm_24000`: 24000 Hz × 16 bit × 1 kanal)
        // tahmini değil, kesin hesap.
        assert_eq!(tts_audio_seconds(48_000), 1.0);
        assert_eq!(tts_audio_seconds(96_000), 2.0);
        assert_eq!(tts_audio_seconds(24_000), 0.5);
    }

    fn gateway_with_tts(tts: Arc<dyn TtsProvider>) -> Arc<Gateway> {
        let config = Arc::new(ServiceConfig::with_defaults());
        config.set("tts.provider", "elevenlabs");
        let mut gw = Gateway::new(config);
        gw.register_stt(Arc::new(MockStt::new("deepgram")));
        gw.register_translate(Arc::new(MockTranslate::new("deepl")));
        gw.register_tts(tts);
        Arc::new(gw)
    }

    /// `user_id`'nin `lang` hedefi için onaylı+aktif tek bir modeli olan depo.
    fn voice_models_with_active(user_id: &str, lang: &str, provider_voice_id: &str) -> Arc<VoiceModelStore> {
        let store = VoiceModelStore::in_memory();
        let model = VoiceModel {
            id: "m1".to_string(),
            user_id: user_id.to_string(),
            name: "test voice".to_string(),
            provider_voice_id: provider_voice_id.to_string(),
            status: VoiceModelStatus::PendingReview,
            created_at: 0,
            language_mode: LanguageMode::SingleToMulti,
            source_languages: vec!["tr".to_string()],
            target_languages: Vec::new(),
            mode: VoiceMode::Dubbing,
            source: VoiceSource::UserCloned,
            has_stored_sample: false,
            consent: VoiceConsent {
                text_version: "v1".to_string(),
                is_own_voice: true,
                accepted_at: 0,
            },
        };
        store.insert(model);
        store.set_status("m1", VoiceModelStatus::Approved);
        store.set_active_for_language("m1", lang).unwrap();
        Arc::new(store)
    }

    struct FailingTts;
    #[async_trait]
    impl TtsProvider for FailingTts {
        fn name(&self) -> &str {
            "elevenlabs"
        }
        async fn clone_voice(&self, _name: &str, _sample: &[u8]) -> Result<String, TtsError> {
            Err(TtsError::Request("clone should not be called here".into()))
        }
        async fn synthesize(&self, _voice_id: &str, _text: &str) -> Result<Vec<u8>, TtsError> {
            Err(TtsError::Request("synthesize always fails in this test".into()))
        }
    }

    #[tokio::test]
    async fn cloud_tts_dispatches_dub_when_active_model_exists() {
        let voice_models = voice_models_with_active("user-1", "en", "voice-abc");
        let mut session = StreamSession::new(
            "s1",
            "tr",
            vec!["en".to_string()],
            gateway_with_tts(Arc::new(MockTts::new("elevenlabs"))),
            "tok-1",
            "user-1",
            -1,
            Arc::new(UsageTracker::new()),
            test_logger(),
            SttSource::Cloud,
            TtsSource::Cloud,
            voice_models,
        );
        let msg = session.process_audio(&[1, 2, 3]).await.expect("subtitle");
        match msg {
            ServerMessage::Subtitle { dubbed_audio, .. } => {
                assert_eq!(dubbed_audio.len(), 1);
                assert_eq!(dubbed_audio[0].lang, "en");
                assert!(!dubbed_audio[0].audio_base64.is_empty());
            }
            other => panic!("beklenmeyen mesaj: {other:?}"),
        }
    }

    #[tokio::test]
    async fn cloud_tts_skips_dub_when_no_active_model() {
        // Hedef dil "en" için hiç atama yok — dub üretilmez ama altyazı/çeviri
        // etkilenmez.
        let mut session = StreamSession::new(
            "s1",
            "tr",
            vec!["en".to_string()],
            gateway_with_tts(Arc::new(MockTts::new("elevenlabs"))),
            "tok-1",
            "user-1",
            -1,
            Arc::new(UsageTracker::new()),
            test_logger(),
            SttSource::Cloud,
            TtsSource::Cloud,
            test_voice_models(),
        );
        let msg = session.process_audio(&[1, 2, 3]).await.expect("subtitle");
        match msg {
            ServerMessage::Subtitle { translations, dubbed_audio, .. } => {
                assert_eq!(translations.len(), 1);
                assert!(dubbed_audio.is_empty());
            }
            other => panic!("beklenmeyen mesaj: {other:?}"),
        }
    }

    #[tokio::test]
    async fn local_tts_source_never_dispatches_dub() {
        // tts_source=local: SDK kendi motoruyla sentezler, backend hiç
        // sentezlemez — aktif model olsa bile dubbed_audio boş kalır.
        let voice_models = voice_models_with_active("user-1", "en", "voice-abc");
        let mut session = StreamSession::new(
            "s1",
            "tr",
            vec!["en".to_string()],
            gateway_with_tts(Arc::new(MockTts::new("elevenlabs"))),
            "tok-1",
            "user-1",
            -1,
            Arc::new(UsageTracker::new()),
            test_logger(),
            SttSource::Cloud,
            TtsSource::Local,
            voice_models,
        );
        let msg = session.process_audio(&[1, 2, 3]).await.expect("subtitle");
        assert!(matches!(msg, ServerMessage::Subtitle { dubbed_audio, .. } if dubbed_audio.is_empty()));
    }

    #[tokio::test]
    async fn interim_transcript_never_dispatches_dub() {
        // is_final=false: gereksiz sentez çağrısı/kota tüketimi olmasın.
        let voice_models = voice_models_with_active("user-1", "en", "voice-abc");
        let mut session = StreamSession::new(
            "s1",
            "tr",
            vec!["en".to_string()],
            gateway_with_tts(Arc::new(MockTts::new("elevenlabs"))),
            "tok-1",
            "user-1",
            -1,
            Arc::new(UsageTracker::new()),
            test_logger(),
            SttSource::Local,
            TtsSource::Cloud,
            voice_models,
        );
        let msg = session
            .process_transcript("merhaba", false, 1.0)
            .await
            .expect("subtitle");
        assert!(matches!(msg, ServerMessage::Subtitle { dubbed_audio, .. } if dubbed_audio.is_empty()));
    }

    #[tokio::test]
    async fn tts_synthesize_failure_is_skipped_not_fatal() {
        let voice_models = voice_models_with_active("user-1", "en", "voice-abc");
        let mut session = StreamSession::new(
            "s1",
            "tr",
            vec!["en".to_string()],
            gateway_with_tts(Arc::new(FailingTts)),
            "tok-1",
            "user-1",
            -1,
            Arc::new(UsageTracker::new()),
            test_logger(),
            SttSource::Cloud,
            TtsSource::Cloud,
            voice_models,
        );
        let msg = session.process_audio(&[1, 2, 3]).await.expect("subtitle still produced");
        match msg {
            ServerMessage::Subtitle { translations, dubbed_audio, .. } => {
                assert_eq!(translations.len(), 1, "çeviri dub hatasından etkilenmemeli");
                assert!(dubbed_audio.is_empty());
            }
            other => panic!("beklenmeyen mesaj: {other:?}"),
        }
    }

    #[tokio::test]
    async fn cloud_tts_charges_usage_pool_shared_with_stt() {
        // Uzun bir metin → gerçek ses baytı 0.5 sn eşiğini rahatça aşar,
        // rounding belirsizliği olmadan pozitif bir kullanım kanıtlanır.
        let voice_models = voice_models_with_active("user-1", "en", "voice-abc");
        let usage = Arc::new(UsageTracker::new());
        let mut session = StreamSession::new(
            "s1",
            "tr",
            vec!["en".to_string()],
            gateway_with_tts(Arc::new(MockTts::new("elevenlabs"))),
            "tok-1",
            "user-1",
            -1,
            Arc::clone(&usage),
            test_logger(),
            SttSource::Local,
            TtsSource::Cloud,
            voice_models,
        );
        // (11 + 60_000) bayt = 60_011 → 60_011*8/384_000 ≈ 1.25 sn — used_seconds
        // en yakın tam saniyeye yuvarlanıyor (`UsageTracker::record_for_period`),
        // bu yüzden 0.5 sn eşiğinden rahatça uzak durmak gerekiyor.
        let long_text = "a".repeat(60_000);
        session
            .process_transcript(&long_text, true, 1.0)
            .await
            .expect("subtitle");
        assert!(usage.status("user-1", -1).used_seconds > 0);
    }

    #[tokio::test]
    async fn cloud_tts_applies_tts_credit_weight() {
        // tts_audio_seconds((11+60_000) bayt) ≈ 1.25 sn → weight=1.0'da
        // round() ile 1 sn, weight=3.0'da ≈3.75 sn → round() ile 4 sn.
        // Admin farklı bir self-hosted TTS modeli için ağırlığı
        // değiştirdiğinde kotaya yansıması gerekir.
        let long_text = "a".repeat(60_000);

        let voice_models_baseline = voice_models_with_active("user-1", "en", "voice-abc");
        let usage_baseline = Arc::new(UsageTracker::new());
        let mut baseline = StreamSession::new(
            "s1",
            "tr",
            vec!["en".to_string()],
            gateway_with_tts(Arc::new(MockTts::new("elevenlabs"))),
            "tok-1",
            "user-1",
            -1,
            Arc::clone(&usage_baseline),
            test_logger(),
            SttSource::Local,
            TtsSource::Cloud,
            voice_models_baseline,
        );
        baseline
            .process_transcript(&long_text, true, 1.0)
            .await
            .expect("subtitle");
        assert_eq!(usage_baseline.status("user-1", -1).used_seconds, 1);

        let voice_models_weighted = voice_models_with_active("user-1", "en", "voice-abc");
        let usage_weighted = Arc::new(UsageTracker::new());
        let mut weighted = StreamSession::new(
            "s1",
            "tr",
            vec!["en".to_string()],
            gateway_with_tts(Arc::new(MockTts::new("elevenlabs"))),
            "tok-1",
            "user-1",
            -1,
            Arc::clone(&usage_weighted),
            test_logger(),
            SttSource::Local,
            TtsSource::Cloud,
            voice_models_weighted,
        )
        .with_credit_weights(1.0, 3.0);
        weighted
            .process_transcript(&long_text, true, 1.0)
            .await
            .expect("subtitle");
        assert_eq!(usage_weighted.status("user-1", -1).used_seconds, 4);
    }
}
