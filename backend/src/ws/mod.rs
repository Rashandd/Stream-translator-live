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
use crate::logging::LogLevel;
use crate::pipeline::StreamSession;
use crate::state::AppState;
use protocol::{ClientMessage, ErrorCode, ServerMessage};

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
                if let Some(reply) = handle_control(text.as_str(), &state, &claims, &mut session) {
                    if send(&mut socket, &reply).await.is_err() {
                        break;
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

/// Doğrulanmış claim'lerden, gerçek kullanım durumuna göre `session_ready` kurar.
fn session_ready(claims: &Claims, state: &AppState) -> ServerMessage {
    let monthly_hours = monthly_hours_for(state, &claims.plan);
    let status = state.usage.status(&claims.sub, monthly_hours);
    ServerMessage::SessionReady {
        session_id: claims.sub.clone(),
        user_id: claims.sub.clone(),
        plan: claims.plan.clone(),
        remaining_seconds: status.remaining_seconds,
    }
}

/// Auth sonrası kontrol mesajlarını işler.
fn handle_control(
    text: &str,
    state: &AppState,
    claims: &Claims,
    session: &mut Option<StreamSession>,
) -> Option<ServerMessage> {
    let msg: ClientMessage = match serde_json::from_str(text) {
        Ok(msg) => msg,
        Err(err) => {
            tracing::debug!(error = %err, "malformed control message");
            return Some(ServerMessage::error(
                ErrorCode::InternalError,
                "malformed_message",
            ));
        }
    };

    match msg {
        ClientMessage::Ping { ts } => Some(ServerMessage::Pong { ts }),
        ClientMessage::StreamStart {
            session_id,
            source_lang,
            target_langs,
            audio_format,
            // Pipeline entegrasyonu bir sonraki adımda (2.6b madde 3) —
            // şimdilik alan sadece protokolde tanınıyor.
            stt_source: _,
        } => {
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
            let overlay_token = uuid::Uuid::new_v4().simple().to_string();
            let monthly_hours = monthly_hours_for(state, &claims.plan);
            state.logger.log(
                LogLevel::Info,
                "ws",
                "stream started",
                Some(&claims.sub),
                Some(&session_id),
            );
            *session = Some(StreamSession::new(
                session_id.clone(),
                source_lang,
                target_langs,
                Arc::clone(&state.gateway),
                overlay_token.clone(),
                claims.sub.clone(),
                monthly_hours,
                Arc::clone(&state.usage),
                Arc::clone(&state.logger),
            ));
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
        ClientMessage::Transcript { .. } => {
            // Lokal STT pipeline yolu sonraki adımda bağlanıyor (2.6b madde 3);
            // stt_source saklanmadığından bütün session'lar cloud sayılır ve
            // transcript reddedilir — davranış, cloud session için nihai halidir.
            if session.is_none() {
                Some(ServerMessage::error(
                    ErrorCode::AudioBeforeStart,
                    "stream_not_started",
                ))
            } else {
                Some(ServerMessage::error(
                    ErrorCode::LocalSttRejected,
                    "stt_source_not_local",
                ))
            }
        }
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
        state.overlay.publish(active.overlay_token(), reply.clone());
        let limit_reached = matches!(
            reply,
            ServerMessage::Error { code, .. } if code == ErrorCode::PlanLimitReached as u16
        );
        send(socket, &reply).await?;
        if limit_reached {
            // Plan limiti dolan session'ı kapat — sonraki ses paketleri
            // "stream_not_started" (4002) alır, yayıncı yeniden başlatmalı.
            state
                .active_streams
                .clear_if(user_id, active.overlay_token());
            *session = None;
        }
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
        let reply = handle_control(json, &st, &claims(), &mut session);
        assert!(matches!(reply, Some(ServerMessage::StreamAck { .. })));
        assert!(session.is_some());
    }

    #[test]
    fn stream_start_rejects_unsupported_codec() {
        let st = state();
        let mut session = None;
        let json = r#"{"type":"stream_start","session_id":"s1","source_lang":"tr","target_langs":["en"],"audio_format":{"codec":"mp3","sample_rate":16000,"channels":1}}"#;
        let reply = handle_control(json, &st, &claims(), &mut session);
        assert!(matches!(reply, Some(ServerMessage::Error { code: 4004, .. })));
        assert!(session.is_none());
    }

    #[test]
    fn ping_yields_pong_after_auth() {
        let st = state();
        let mut session = None;
        let reply = handle_control(r#"{"type":"ping","ts":42}"#, &st, &claims(), &mut session);
        assert_eq!(reply, Some(ServerMessage::Pong { ts: 42 }));
    }

    #[test]
    fn stream_stop_clears_session() {
        let st = state();
        let mut session = None;
        let start = r#"{"type":"stream_start","session_id":"s1","source_lang":"tr","target_langs":["en"],"audio_format":{"codec":"opus","sample_rate":16000,"channels":1}}"#;
        handle_control(start, &st, &claims(), &mut session);
        assert!(session.is_some());
        handle_control(r#"{"type":"stream_stop","session_id":"s1"}"#, &st, &claims(), &mut session);
        assert!(session.is_none());
    }
}
