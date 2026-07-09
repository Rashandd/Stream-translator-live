//! WebSocket katmanı — SDK ↔ backend ses pipeline'ı.
//!
//! Bağlantı kurulunca ilk mesaj `auth` olmalı (5 sn içinde). Doğrulanınca
//! `session_ready` döner; `stream_start` ile bir [`StreamSession`] açılır ve
//! gelen binary ses frame'leri pipeline'a beslenir.

pub mod frame;
pub mod protocol;

use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade},
    extract::State,
    response::IntoResponse,
};

use crate::auth::jwt::{Claims, Jwt};
use crate::integrations::oauth::plan_allowed;
use crate::logging::LogLevel;
use crate::pipeline::StreamSession;
use crate::state::AppState;
use protocol::{ClientMessage, ErrorCode, ServerMessage, SttSource};

/// `auth` mesajı için bekleme süresi.
const AUTH_TIMEOUT: Duration = Duration::from_secs(5);

/// `/ws/stream` route handler — bağlantıyı WebSocket'e yükseltir.
pub async fn ws_handler(State(state): State<AppState>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Tek bir WebSocket bağlantısının yaşam döngüsü.
async fn handle_socket(mut socket: WebSocket, state: AppState) {
    // 1) Auth handshake — 5 sn içinde `auth` gelmezse kapat.
    let claims = match await_auth(&mut socket, &state.jwt).await {
        Some(claims) => claims,
        None => return,
    };

    state.logger.log(
        LogLevel::Info,
        "ws",
        "websocket authenticated",
        Some(&claims.sub),
        None,
    );

    let ready = session_ready(&claims, &state);
    if send(&mut socket, &ready).await.is_err() {
        return;
    }

    // 2) Stream döngüsü.
    let mut session: Option<StreamSession> = None;

    while let Some(incoming) = socket.recv().await {
        let msg = match incoming {
            Ok(msg) => msg,
            Err(err) => {
                tracing::debug!(error = %err, "websocket receive error");
                break;
            }
        };

        match msg {
            Message::Text(text) => {
                match serde_json::from_str::<ClientMessage>(text.as_str()) {
                    // Transcript async pipeline gerektirir (çeviri await'i),
                    // diğer kontrol mesajları senkron işlenir.
                    Ok(ClientMessage::Transcript { text: line, is_final, .. }) => {
                        if send_for_transcript(
                            &mut socket,
                            &line,
                            is_final,
                            &mut session,
                            &state,
                            &claims.sub,
                        )
                        .await
                        .is_err()
                        {
                            break;
                        }
                    }
                    Ok(control) => {
                        if let Some(reply) = handle_control(control, &state, &claims, &mut session)
                        {
                            if send(&mut socket, &reply).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(err) => {
                        tracing::debug!(error = %err, "malformed control message");
                        let reply =
                            ServerMessage::error(ErrorCode::InternalError, "malformed_message");
                        if send(&mut socket, &reply).await.is_err() {
                            break;
                        }
                    }
                }
            }
            Message::Binary(bytes) => {
                if send_for_audio(&mut socket, &bytes, &mut session, &state, &claims.sub)
                    .await
                    .is_err()
                {
                    break;
                }
            }
            Message::Ping(_) | Message::Pong(_) => {}
            Message::Close(_) => break,
        }
    }

    // Bağlantı koptu — aktif yayın kaydını temizle (EBS "live" durumu).
    if let Some(active) = session.as_ref() {
        state
            .active_streams
            .clear_if(&claims.sub, active.overlay_token());
    }
}

/// İlk mesajı bekler, `auth` ise doğrular. Aksi halde uygun error yollayıp `None`.
async fn await_auth(socket: &mut WebSocket, jwt: &Jwt) -> Option<Claims> {
    let first = match tokio::time::timeout(AUTH_TIMEOUT, socket.recv()).await {
        Ok(Some(Ok(Message::Text(text)))) => text,
        Ok(Some(Ok(Message::Close(_)))) | Ok(None) => return None,
        Ok(Some(Ok(_))) => {
            // Auth'tan önce binary/diğer frame: 4002.
            let _ = send(
                socket,
                &ServerMessage::error(ErrorCode::AudioBeforeStart, "auth_required"),
            )
            .await;
            return None;
        }
        Ok(Some(Err(_))) | Err(_) => {
            let _ = send(
                socket,
                &ServerMessage::error(ErrorCode::InvalidToken, "auth_timeout"),
            )
            .await;
            return None;
        }
    };

    match authenticate(jwt, first.as_str()) {
        Ok(claims) => Some(claims),
        Err(reply) => {
            let _ = send(socket, &reply).await;
            None
        }
    }
}

/// İlk text mesajını `auth` olarak parse edip JWT'yi doğrular.
///
/// Hata, gönderilmeye hazır bir `Error` mesajı taşır. `ServerMessage` büyük bir
/// enum olduğundan `Box`'lanır (clippy `result_large_err`).
fn authenticate(jwt: &Jwt, text: &str) -> Result<Claims, Box<ServerMessage>> {
    let msg: ClientMessage = serde_json::from_str(text)
        .map_err(|_| Box::new(ServerMessage::error(ErrorCode::InvalidToken, "malformed_auth")))?;

    let token = match msg {
        ClientMessage::Auth { token } => token,
        _ => {
            return Err(Box::new(ServerMessage::error(
                ErrorCode::InvalidToken,
                "auth_required",
            )))
        }
    };

    jwt.verify(&token)
        .map_err(|_| Box::new(ServerMessage::error(ErrorCode::InvalidToken, "invalid_token")))
}

/// Bir planın aylık saat limitini saniyeye çevirir. Bilinmeyen plan adı için
/// güvenli (kısıtlayıcı) varsayılan olarak `free` planı kullanılır; o da
/// yoksa sınırsız (`-1`) kabul edilir — pratikte olmaz, seed her zaman `free`
/// içerir.
fn monthly_hours_for(state: &AppState, plan: &str) -> i64 {
    state
        .plans
        .get(plan)
        .or_else(|| state.plans.get("free"))
        .map(|p| p.monthly_hours)
        .unwrap_or(-1)
}

/// JWT `claims.plan` 24 saat stale kalabilir (token yenilenene kadar) —
/// admin bir kullanıcıyı "founding" beta'ya onayladığında bu anında
/// yansımalı, re-login beklenmemeli. Bu yüzden plan limitleri için
/// `claims.plan` yerine bu fonksiyon kullanılır: onaylı founding
/// başvurusu varsa `founding` planı, yoksa claim'deki plan.
fn effective_plan(state: &AppState, claims: &Claims) -> String {
    if state.founding.is_approved(&claims.sub) {
        "founding".to_string()
    } else {
        claims.plan.clone()
    }
}

/// Doğrulanmış claim'lerden, gerçek kullanım durumuna göre `session_ready` kurar.
fn session_ready(claims: &Claims, state: &AppState) -> ServerMessage {
    let plan = effective_plan(state, claims);
    let monthly_hours = monthly_hours_for(state, &plan);
    let status = state.usage.status(&claims.sub, monthly_hours);
    ServerMessage::SessionReady {
        session_id: claims.sub.clone(),
        user_id: claims.sub.clone(),
        plan,
        remaining_seconds: status.remaining_seconds,
    }
}

/// Auth sonrası kontrol mesajlarını işler (`transcript` hariç — o, çeviri
/// await'i gerektirdiğinden socket döngüsünde `send_for_transcript` ile akar).
fn handle_control(
    msg: ClientMessage,
    state: &AppState,
    claims: &Claims,
    session: &mut Option<StreamSession>,
) -> Option<ServerMessage> {
    match msg {
        ClientMessage::Ping { ts } => Some(ServerMessage::Pong { ts }),
        ClientMessage::StreamStart {
            session_id,
            source_lang,
            target_langs,
            audio_format,
            stt_source,
            tts_source,
        } => {
            // "Founding 5" kapalı beta kapısı (Faz 1 ilk test): açıksa ve
            // kullanıcı admin değilse, onaylı bir founding başvurusu şart —
            // canlı okunur (`is_approved`), JWT yenilenmesini beklemez.
            let founding_gate_enabled = state
                .service_config
                .get("founding.gate.enabled")
                .as_deref()
                == Some("true");
            if founding_gate_enabled
                && claims.role != "admin"
                && !state.founding.is_approved(&claims.sub)
            {
                state.logger.log(
                    LogLevel::Warn,
                    "ws",
                    "stream_start rejected: founding approval pending",
                    Some(&claims.sub),
                    Some(&session_id),
                );
                return Some(ServerMessage::error(
                    ErrorCode::FoundingApprovalPending,
                    "founding_approval_pending",
                ));
            }
            if audio_format.codec != "opus" && audio_format.codec != "pcm_s16le" {
                state.logger.log(
                    LogLevel::Warn,
                    "ws",
                    &format!("stream_start rejected: unsupported codec {}", audio_format.codec),
                    Some(&claims.sub),
                    Some(&session_id),
                );
                return Some(ServerMessage::error(
                    ErrorCode::UnsupportedCodec,
                    "unsupported_codec",
                ));
            }
            // Onaylı founding testerlar için limitler `claims.plan` yerine
            // canlı okunan `effective_plan`'dan gelir (bkz. yukarıdaki not).
            let plan = effective_plan(state, claims);
            // DeepL hedef dil matrisi (Faz 2.0): plan başına azami hedef dil
            // sayısı (`PlanRegistry.max_languages`, -1 = sınırsız) + admin'in
            // açtığı katalog alt kümesi (`translate.deepl.enabled_targets`).
            // Panel zaten `PUT /api/translate/languages`'ta aynı kuralları
            // uygular — burası SDK doğrudan eski/elle kurulmuş bir
            // `target_langs` gönderirse diye sunucu tarafı ikinci savunma.
            let max_languages = state.plans.get(&plan).map(|p| p.max_languages).unwrap_or(1);
            if max_languages >= 0 && target_langs.len() as i64 > max_languages {
                state.logger.log(
                    LogLevel::Warn,
                    "ws",
                    &format!(
                        "stream_start rejected: {} target languages exceeds plan {} limit of {}",
                        target_langs.len(),
                        plan,
                        max_languages
                    ),
                    Some(&claims.sub),
                    Some(&session_id),
                );
                return Some(ServerMessage::error(
                    ErrorCode::TooManyTargetLanguages,
                    "too_many_target_languages",
                ));
            }
            let enabled_targets_csv = state
                .service_config
                .get("translate.deepl.enabled_targets")
                .unwrap_or_default();
            if let Some(unsupported) = target_langs
                .iter()
                .find(|lang| !plan_allowed(&enabled_targets_csv, lang))
            {
                state.logger.log(
                    LogLevel::Warn,
                    "ws",
                    &format!("stream_start rejected: unsupported target language {unsupported}"),
                    Some(&claims.sub),
                    Some(&session_id),
                );
                return Some(ServerMessage::error(
                    ErrorCode::UnsupportedTargetLanguage,
                    "unsupported_target_language",
                ));
            }
            // Lokal STT plan gating'i (2.6b): hangi planların stt_source=local
            // bildirebileceği `stt.local.plans` config'inden okunur (admin
            // panelden restart'sız değiştirilir).
            if stt_source == SttSource::Local {
                let plans_csv = state
                    .service_config
                    .get("stt.local.plans")
                    .unwrap_or_default();
                if !plan_allowed(&plans_csv, &plan) {
                    state.logger.log(
                        LogLevel::Warn,
                        "ws",
                        &format!(
                            "stream_start rejected: plan {} not in stt.local.plans",
                            plan
                        ),
                        Some(&claims.sub),
                        Some(&session_id),
                    );
                    return Some(ServerMessage::error(
                        ErrorCode::LocalSttRejected,
                        "local_stt_not_allowed",
                    ));
                }
            }
            let overlay_token = uuid::Uuid::new_v4().simple().to_string();
            let monthly_hours = monthly_hours_for(state, &plan);
            state.logger.log(
                LogLevel::Info,
                "ws",
                &format!(
                    "stream started (stt_source={})",
                    match stt_source {
                        SttSource::Cloud => "cloud",
                        SttSource::Local => "local",
                    }
                ),
                Some(&claims.sub),
                Some(&session_id),
            );
            let stt_credit_weight = crate::stt_policy::cloud_credit_weight(&state.service_config);
            let tts_credit_weight = crate::tts_policy::cloud_credit_weight(&state.service_config);
            *session = Some(
                StreamSession::new(
                    session_id.clone(),
                    source_lang,
                    target_langs,
                    Arc::clone(&state.gateway),
                    overlay_token.clone(),
                    claims.sub.clone(),
                    monthly_hours,
                    Arc::clone(&state.usage),
                    Arc::clone(&state.logger),
                    stt_source,
                    tts_source,
                    Arc::clone(&state.voice_models),
                )
                .with_credit_weights(stt_credit_weight, tts_credit_weight),
            );
            // EBS (Twitch Extension) kanalın canlı olduğunu buradan görür.
            state.active_streams.set(&claims.sub, &overlay_token);
            Some(ServerMessage::StreamAck {
                session_id,
                status: "streaming".to_string(),
                overlay_token,
            })
        }
        ClientMessage::StreamStop { session_id } => {
            state.logger.log(
                LogLevel::Info,
                "ws",
                "stream stopped",
                Some(&claims.sub),
                Some(&session_id),
            );
            if let Some(active) = session.as_ref() {
                state
                    .active_streams
                    .clear_if(&claims.sub, active.overlay_token());
            }
            *session = None;
            None
        }
        // Socket döngüsü Transcript'i buraya düşürmez; savunmacı no-op.
        ClientMessage::Transcript { .. } => None,
        ClientMessage::Auth { .. } => None, // zaten doğrulandı, yok say
    }
}

/// Binary ses frame'ini parse edip pipeline'a besler; üretilen subtitle'ı hem
/// SDK'ya yollar hem de overlay hub'ında session'ın token'ına yayınlar.
async fn send_for_audio(
    socket: &mut WebSocket,
    bytes: &[u8],
    session: &mut Option<StreamSession>,
    state: &AppState,
    user_id: &str,
) -> Result<(), axum::Error> {
    let Some(active) = session.as_mut() else {
        // `stream_start` gelmeden ses paketi: 4002.
        return send(
            socket,
            &ServerMessage::error(ErrorCode::AudioBeforeStart, "stream_not_started"),
        )
        .await;
    };

    let frame = match frame::parse(bytes) {
        Ok(f) => f,
        Err(err) => {
            tracing::debug!(error = %err, "invalid audio frame");
            return send(
                socket,
                &ServerMessage::error(ErrorCode::UnsupportedCodec, "invalid_audio_frame"),
            )
            .await;
        }
    };

    if let Some(reply) = active.process_audio(frame.payload).await {
        return publish_and_send(socket, state, session, user_id, reply).await;
    }
    Ok(())
}

/// Lokal STT transcript'ini pipeline'a besler (Faz 2.6b). Ses yoluyla aynı
/// kurallar: session yoksa 4002; session `stt_source: local` değilse 4007 —
/// plan gating'i baypas etmemek için mod yayın başında bildirilmiş olmalı.
async fn send_for_transcript(
    socket: &mut WebSocket,
    text: &str,
    is_final: bool,
    session: &mut Option<StreamSession>,
    state: &AppState,
    user_id: &str,
) -> Result<(), axum::Error> {
    let Some(active) = session.as_mut() else {
        return send(
            socket,
            &ServerMessage::error(ErrorCode::AudioBeforeStart, "stream_not_started"),
        )
        .await;
    };

    if active.stt_source() != SttSource::Local {
        state.logger.log(
            LogLevel::Warn,
            "ws",
            "transcript rejected: session stt_source is cloud",
            Some(user_id),
            None,
        );
        return send(
            socket,
            &ServerMessage::error(ErrorCode::LocalSttRejected, "stt_source_not_local"),
        )
        .await;
    }

    // Çeviri API'si karakter başına faturalanır — tek satır üst sınırı
    // `stt.local.max_chars` (VAD bitmiş cümle yollar, 500 normalde bol).
    let max_chars = state
        .service_config
        .get("stt.local.max_chars")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(500);
    let char_count = text.chars().count();
    if char_count > max_chars {
        state.logger.log(
            LogLevel::Warn,
            "ws",
            &format!("transcript rejected: {char_count} chars > stt.local.max_chars={max_chars}"),
            Some(user_id),
            None,
        );
        return send(
            socket,
            &ServerMessage::error(ErrorCode::LocalSttRejected, "transcript_too_long"),
        )
        .await;
    }

    let multiplier = crate::stt_policy::usage_multiplier(&state.service_config);
    if let Some(reply) = active.process_transcript(text, is_final, multiplier).await {
        return publish_and_send(socket, state, session, user_id, reply).await;
    }
    Ok(())
}

/// Pipeline çıktısını overlay hub'ına yayınlar ve SDK'ya gönderir; plan
/// limiti hatasında session'ı kapatır (ses ve transcript yollarının ortak
/// teslim ucu).
async fn publish_and_send(
    socket: &mut WebSocket,
    state: &AppState,
    session: &mut Option<StreamSession>,
    user_id: &str,
    reply: ServerMessage,
) -> Result<(), axum::Error> {
    let Some(active) = session.as_ref() else {
        return Ok(());
    };
    let overlay_token = active.overlay_token().to_string();

    state.overlay.publish(&overlay_token, reply.clone());
    let limit_reached = matches!(
        reply,
        ServerMessage::Error { code, .. } if code == ErrorCode::PlanLimitReached as u16
    );
    send(socket, &reply).await?;
    if limit_reached {
        // Plan limiti dolan session'ı kapat — sonraki ses paketleri
        // "stream_not_started" (4002) alır, yayıncı yeniden başlatmalı.
        state.active_streams.clear_if(user_id, &overlay_token);
        *session = None;
    }
    Ok(())
}

/// Bir `ServerMessage`'ı JSON text frame olarak gönderir.
async fn send(socket: &mut WebSocket, msg: &ServerMessage) -> Result<(), axum::Error> {
    let json = match serde_json::to_string(msg) {
        Ok(json) => json,
        Err(err) => {
            tracing::error!(error = %err, "failed to serialize server message");
            return Ok(());
        }
    };
    socket.send(Message::Text(Utf8Bytes::from(json))).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ServiceConfig;
    use crate::gateway::{stt::MockStt, translate::MockTranslate, Gateway};

    fn jwt() -> Jwt {
        Jwt::new(b"test-secret")
    }

    fn claims() -> Claims {
        Claims {
            sub: "u-1".into(),
            role: "broadcaster".into(),
            plan: "free".into(),
            exp: chrono::Utc::now().timestamp() + 3600,
        }
    }

    fn state() -> AppState {
        let config = Arc::new(ServiceConfig::with_defaults());
        let mut gw = Gateway::new(Arc::clone(&config));
        gw.register_stt(Arc::new(MockStt::new("deepgram")));
        gw.register_translate(Arc::new(MockTranslate::new("deepl")));
        AppState::skeleton(jwt(), config, gw)
    }

    /// Test yardımcı: JSON'u parse edip `handle_control`'e verir.
    fn control(
        json: &str,
        st: &AppState,
        claims: &Claims,
        session: &mut Option<StreamSession>,
    ) -> Option<ServerMessage> {
        let msg: ClientMessage = serde_json::from_str(json).expect("geçerli kontrol JSON'u");
        handle_control(msg, st, claims, session)
    }

    #[test]
    fn authenticate_accepts_valid_token() {
        let j = jwt();
        let token = j.issue(&claims()).expect("issue");
        let auth = format!(r#"{{"type":"auth","token":"{token}"}}"#);
        let decoded = authenticate(&j, &auth).expect("authenticate");
        assert_eq!(decoded.sub, "u-1");
    }

    #[test]
    fn authenticate_rejects_non_auth_first_message() {
        let j = jwt();
        let err = authenticate(&j, r#"{"type":"ping","ts":1}"#).unwrap_err();
        assert!(matches!(*err, ServerMessage::Error { code: 4001, .. }));
    }

    #[test]
    fn authenticate_rejects_bad_token() {
        let j = jwt();
        let err = authenticate(&j, r#"{"type":"auth","token":"garbage"}"#).unwrap_err();
        assert!(matches!(*err, ServerMessage::Error { code: 4001, .. }));
    }

    #[test]
    fn stream_start_opens_session_and_acks() {
        let st = state();
        let mut session = None;
        let json = r#"{"type":"stream_start","session_id":"s1","source_lang":"tr","target_langs":["en"],"audio_format":{"codec":"opus","sample_rate":16000,"channels":1}}"#;
        let reply = control(json, &st, &claims(), &mut session);
        assert!(matches!(reply, Some(ServerMessage::StreamAck { .. })));
        // stt_source gönderilmedi → cloud varsayılır (eski SDK uyumu).
        assert_eq!(
            session.as_ref().expect("session").stt_source(),
            SttSource::Cloud
        );
    }

    #[test]
    fn stream_start_stores_local_stt_source() {
        let st = state();
        let mut session = None;
        let json = r#"{"type":"stream_start","session_id":"s1","source_lang":"tr","target_langs":["en"],"audio_format":{"codec":"opus","sample_rate":16000,"channels":1},"stt_source":"local"}"#;
        let reply = control(json, &st, &claims(), &mut session);
        assert!(matches!(reply, Some(ServerMessage::StreamAck { .. })));
        assert_eq!(
            session.as_ref().expect("session").stt_source(),
            SttSource::Local
        );
    }

    #[test]
    fn stream_start_local_denied_when_plan_not_in_config() {
        let st = state();
        // Sadece pro'ya izin ver — claims planı "free".
        st.service_config.set("stt.local.plans", "pro");
        let mut session = None;
        let json = r#"{"type":"stream_start","session_id":"s1","source_lang":"tr","target_langs":["en"],"audio_format":{"codec":"opus","sample_rate":16000,"channels":1},"stt_source":"local"}"#;
        let reply = control(json, &st, &claims(), &mut session);
        match reply {
            Some(ServerMessage::Error { code, message, .. }) => {
                assert_eq!(code, 4007);
                assert_eq!(message, "local_stt_not_allowed");
            }
            other => panic!("beklenmeyen yanıt: {other:?}"),
        }
        assert!(session.is_none());

        // Cloud başlangıç aynı planla serbest kalır — gating sadece local'i keser.
        let cloud = r#"{"type":"stream_start","session_id":"s1","source_lang":"tr","target_langs":["en"],"audio_format":{"codec":"opus","sample_rate":16000,"channels":1}}"#;
        let reply = control(cloud, &st, &claims(), &mut session);
        assert!(matches!(reply, Some(ServerMessage::StreamAck { .. })));
    }

    #[test]
    fn stream_start_local_allowed_by_default_config() {
        // Varsayılan seed tüm planları içerir → free plan lokal STT açabilir.
        let st = state();
        let mut session = None;
        let json = r#"{"type":"stream_start","session_id":"s1","source_lang":"tr","target_langs":["en"],"audio_format":{"codec":"opus","sample_rate":16000,"channels":1},"stt_source":"local"}"#;
        let reply = control(json, &st, &claims(), &mut session);
        assert!(matches!(reply, Some(ServerMessage::StreamAck { .. })));
    }

    #[test]
    fn stream_start_rejects_unsupported_codec() {
        let st = state();
        let mut session = None;
        let json = r#"{"type":"stream_start","session_id":"s1","source_lang":"tr","target_langs":["en"],"audio_format":{"codec":"mp3","sample_rate":16000,"channels":1}}"#;
        let reply = control(json, &st, &claims(), &mut session);
        assert!(matches!(reply, Some(ServerMessage::Error { code: 4004, .. })));
        assert!(session.is_none());
    }

    #[test]
    fn stream_start_rejects_too_many_target_languages_for_plan() {
        // free plan: max_languages = 1 (PlanRegistry seed'i).
        let st = state();
        let mut session = None;
        let json = r#"{"type":"stream_start","session_id":"s1","source_lang":"tr","target_langs":["en","de"],"audio_format":{"codec":"opus","sample_rate":16000,"channels":1}}"#;
        let reply = control(json, &st, &claims(), &mut session);
        assert!(matches!(reply, Some(ServerMessage::Error { code: 4008, .. })));
        assert!(session.is_none());
    }

    #[test]
    fn stream_start_allows_up_to_plan_limit() {
        let st = state();
        let mut session = None;
        // pro plan: max_languages = 10 — aynı claims'e pro veriyoruz.
        let mut pro_claims = claims();
        pro_claims.plan = "pro".into();
        let json = r#"{"type":"stream_start","session_id":"s1","source_lang":"tr","target_langs":["en","de","es"],"audio_format":{"codec":"opus","sample_rate":16000,"channels":1}}"#;
        let reply = control(json, &st, &pro_claims, &mut session);
        assert!(matches!(reply, Some(ServerMessage::StreamAck { .. })));
    }

    #[test]
    fn stream_start_rejects_unsupported_target_language() {
        let st = state();
        // Admin katalogu daraltır: sadece "en" açık.
        st.service_config.set("translate.deepl.enabled_targets", "en");
        let mut session = None;
        let json = r#"{"type":"stream_start","session_id":"s1","source_lang":"tr","target_langs":["xx"],"audio_format":{"codec":"opus","sample_rate":16000,"channels":1}}"#;
        let reply = control(json, &st, &claims(), &mut session);
        assert!(matches!(reply, Some(ServerMessage::Error { code: 4009, .. })));
        assert!(session.is_none());

        // "en" hâlâ açık — kabul edilir.
        let ok = r#"{"type":"stream_start","session_id":"s1","source_lang":"tr","target_langs":["en"],"audio_format":{"codec":"opus","sample_rate":16000,"channels":1}}"#;
        let reply = control(ok, &st, &claims(), &mut session);
        assert!(matches!(reply, Some(ServerMessage::StreamAck { .. })));
    }

    #[test]
    fn ping_yields_pong_after_auth() {
        let st = state();
        let mut session = None;
        let reply = control(r#"{"type":"ping","ts":42}"#, &st, &claims(), &mut session);
        assert_eq!(reply, Some(ServerMessage::Pong { ts: 42 }));
    }

    #[test]
    fn stream_stop_clears_session() {
        let st = state();
        let mut session = None;
        let start = r#"{"type":"stream_start","session_id":"s1","source_lang":"tr","target_langs":["en"],"audio_format":{"codec":"opus","sample_rate":16000,"channels":1}}"#;
        control(start, &st, &claims(), &mut session);
        assert!(session.is_some());
        control(r#"{"type":"stream_stop","session_id":"s1"}"#, &st, &claims(), &mut session);
        assert!(session.is_none());
    }

    // ── "Founding 5" kapalı beta kapısı ───────────────────────────────────

    use crate::founding::FoundingStatus;

    const START_JSON: &str = r#"{"type":"stream_start","session_id":"s1","source_lang":"tr","target_langs":["en"],"audio_format":{"codec":"opus","sample_rate":16000,"channels":1}}"#;

    #[test]
    fn gate_disabled_by_default_allows_unapproved_user() {
        // Seed varsayılanı: founding.gate.enabled = "false".
        let st = state();
        let mut session = None;
        let reply = control(START_JSON, &st, &claims(), &mut session);
        assert!(matches!(reply, Some(ServerMessage::StreamAck { .. })));
    }

    #[test]
    fn gate_enabled_rejects_unapproved_user() {
        let st = state();
        st.service_config.set("founding.gate.enabled", "true");
        let mut session = None;
        let reply = control(START_JSON, &st, &claims(), &mut session);
        match reply {
            Some(ServerMessage::Error { code, message, .. }) => {
                assert_eq!(code, 4010);
                assert_eq!(message, "founding_approval_pending");
            }
            other => panic!("beklenmeyen yanıt: {other:?}"),
        }
        assert!(session.is_none());
    }

    #[test]
    fn gate_enabled_allows_approved_user_and_grants_founding_plan_limits() {
        let st = state();
        st.service_config.set("founding.gate.enabled", "true");
        st.founding.upsert_application(
            "u-1",
            "u1@example.com",
            crate::founding::FoundingPlatform::Kick,
            "https://kick.com/u1",
            "",
        );
        st.founding.set_status("u-1", FoundingStatus::Approved);

        let mut session = None;
        // free plan tek dille sınırlı ama founding planı max_languages=3 —
        // 2 hedef dil onaylı founding kullanıcısı için kabul edilmeli.
        let json = r#"{"type":"stream_start","session_id":"s1","source_lang":"tr","target_langs":["en","de"],"audio_format":{"codec":"opus","sample_rate":16000,"channels":1}}"#;
        let reply = control(json, &st, &claims(), &mut session);
        assert!(matches!(reply, Some(ServerMessage::StreamAck { .. })));
    }

    #[test]
    fn gate_enabled_allows_admin_role_without_approval() {
        let st = state();
        st.service_config.set("founding.gate.enabled", "true");
        let mut admin_claims = claims();
        admin_claims.role = "admin".into();
        let mut session = None;
        let reply = control(START_JSON, &st, &admin_claims, &mut session);
        assert!(matches!(reply, Some(ServerMessage::StreamAck { .. })));
    }

    #[test]
    fn revoking_approval_takes_effect_immediately_without_new_token() {
        let st = state();
        st.service_config.set("founding.gate.enabled", "true");
        st.founding.upsert_application(
            "u-1",
            "u1@example.com",
            crate::founding::FoundingPlatform::Kick,
            "https://kick.com/u1",
            "",
        );
        st.founding.set_status("u-1", FoundingStatus::Approved);

        let mut session = None;
        let reply = control(START_JSON, &st, &claims(), &mut session);
        assert!(matches!(reply, Some(ServerMessage::StreamAck { .. })));
        control(r#"{"type":"stream_stop","session_id":"s1"}"#, &st, &claims(), &mut session);

        // Aynı claims (aynı "eski" JWT) — admin iptal etti, canlı store hemen yansır.
        st.founding.set_status("u-1", FoundingStatus::Revoked);
        let reply = control(START_JSON, &st, &claims(), &mut session);
        assert!(matches!(reply, Some(ServerMessage::Error { code: 4010, .. })));
    }
}
