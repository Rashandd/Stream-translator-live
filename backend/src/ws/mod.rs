//! WebSocket katmanı — SDK ↔ backend ses pipeline'ı.

pub mod frame;
pub mod protocol;

use axum::{
    extract::ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
};

use protocol::{ClientMessage, ErrorCode, ServerMessage};

/// `/ws/stream` route handler — bağlantıyı WebSocket'e yükseltir.
pub async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

/// Tek bir WebSocket bağlantısının yaşam döngüsü.
///
/// Faz 1 iskelet: ilk mesaj `auth` olmalı (spec). Auth doğrulama (JWT) ayrı
/// taskta gelecek; şimdilik kontrol akışı ve mesaj çerçeveleme kuruluyor.
async fn handle_socket(mut socket: WebSocket) {
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
                if let Some(reply) = handle_text(text.as_str()) {
                    if send(&mut socket, &reply).await.is_err() {
                        break;
                    }
                }
            }
            Message::Binary(bytes) => {
                // Ses frame'i — header parse, sonra STT'ye iletilecek (sonraki task).
                match frame::parse(&bytes) {
                    Ok(f) => tracing::trace!(codec = ?f.codec, len = f.payload.len(), "audio frame"),
                    Err(err) => {
                        tracing::debug!(error = %err, "invalid audio frame");
                        let reply =
                            ServerMessage::error(ErrorCode::UnsupportedCodec, "invalid_audio_frame");
                        if send(&mut socket, &reply).await.is_err() {
                            break;
                        }
                    }
                }
            }
            Message::Ping(_) | Message::Pong(_) => {
                // axum frame seviyesinde ping/pong'u otomatik yönetir.
            }
            Message::Close(_) => break,
        }
    }
}

/// Bir Text mesajını parse edip varsa yanıt üretir.
fn handle_text(text: &str) -> Option<ServerMessage> {
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
        ClientMessage::Auth { .. } => {
            // JWT doğrulama sonraki taskta. İskelet: henüz session açmıyoruz.
            Some(ServerMessage::error(
                ErrorCode::InvalidToken,
                "auth_not_implemented",
            ))
        }
        ClientMessage::StreamStart { .. } | ClientMessage::StreamStop { .. } => None,
    }
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

    #[test]
    fn ping_yields_pong() {
        let reply = handle_text(r#"{"type":"ping","ts":42}"#);
        assert_eq!(reply, Some(ServerMessage::Pong { ts: 42 }));
    }

    #[test]
    fn malformed_yields_error() {
        let reply = handle_text("not json");
        match reply {
            Some(ServerMessage::Error { code, .. }) => assert_eq!(code, 5000),
            other => panic!("beklenmeyen yanıt: {other:?}"),
        }
    }

    #[test]
    fn stream_start_has_no_immediate_reply() {
        let json = r#"{"type":"stream_stop","session_id":"s1"}"#;
        assert_eq!(handle_text(json), None);
    }
}
