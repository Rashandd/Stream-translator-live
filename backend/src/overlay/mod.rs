//! Genel altyazı overlay yayını (Faz 1.9) — OBS Browser Source için gerçek
//! zamanlı altyazı dağıtımı.
//!
//! Her stream session'ı `stream_start`'ta rastgele, tahmin edilemez bir
//! overlay token alır (bkz. `ws::mod::handle_control`, `StreamAck.overlay_token`).
//! Backend, o session için üretilen her `subtitle` mesajını bu hub üzerinden
//! token'a abone tüm istemcilere (OBS Browser Source, dashboard canlı
//! önizleme) yayınlar. Token, stream key'le aynı güven modelini kullanır:
//! kendisi bir capability'dir, ayrıca JWT gerektirmez.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::extract::ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use tokio::sync::broadcast;

use crate::ws::protocol::ServerMessage;

const CHANNEL_CAPACITY: usize = 32;

/// Token → broadcast kanalı eşlemesini tutan hub.
pub struct OverlayHub {
    channels: Mutex<HashMap<String, broadcast::Sender<ServerMessage>>>,
}

impl Default for OverlayHub {
    fn default() -> Self {
        Self::new()
    }
}

impl OverlayHub {
    /// Boş bir hub kurar.
    pub fn new() -> Self {
        OverlayHub {
            channels: Mutex::new(HashMap::new()),
        }
    }

    /// Bir token için kanal yoksa oluşturur, varsa mevcut olanı döner.
    fn get_or_create(&self, token: &str) -> broadcast::Sender<ServerMessage> {
        let mut map = self.channels.lock().expect("overlay hub lock poisoned");
        map.entry(token.to_string())
            .or_insert_with(|| broadcast::channel(CHANNEL_CAPACITY).0)
            .clone()
    }

    /// Bir mesajı token'a abone tüm istemcilere yayınlar. Hiç dinleyen yoksa
    /// no-op (broadcast semantiği — hata değildir).
    pub fn publish(&self, token: &str, msg: ServerMessage) {
        let sender = self.get_or_create(token);
        let _ = sender.send(msg);
    }

    /// Bir token'a abone olur — WS handler bu receiver'dan okuyup istemciye iletir.
    pub fn subscribe(&self, token: &str) -> broadcast::Receiver<ServerMessage> {
        self.get_or_create(token).subscribe()
    }
}

/// `GET /overlay/{token}` — OBS Browser Source / dashboard önizleme için genel
/// (auth gerektirmeyen) altyazı WebSocket'i. Token bilinmiyorsa bağlantı
/// açılır ama hiç mesaj gelmez (hub token'ı henüz görmemiş olabilir — SDK
/// `stream_start` yapmadan yayıncı overlay URL'sini açabilir).
pub async fn overlay_ws_handler(
    State(hub): State<Arc<OverlayHub>>,
    Path(token): Path<String>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| forward_to_client(socket, hub, token))
}

/// Hub'dan gelen mesajları istemciye JSON text frame olarak iletir; istemci
/// kapatana veya hata verene kadar döner.
async fn forward_to_client(mut socket: WebSocket, hub: Arc<OverlayHub>, token: String) {
    let mut rx = hub.subscribe(&token);
    loop {
        tokio::select! {
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    // İstemciden gelen diğer frame'ler yok sayılır — bu kanal tek yönlü.
                    _ => {}
                }
            }
            msg = rx.recv() => {
                let Ok(msg) = msg else { break };
                let Ok(json) = serde_json::to_string(&msg) else { continue };
                if socket.send(Message::Text(Utf8Bytes::from(json))).await.is_err() {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ws::protocol::{ServerMessage, SubtitleSource};

    fn subtitle(seq: u64) -> ServerMessage {
        ServerMessage::Subtitle {
            session_id: "s1".into(),
            sequence: seq,
            source: SubtitleSource {
                lang: "tr".into(),
                text: "merhaba".into(),
                is_final: true,
            },
            translations: vec![],
            ts_start_ms: 0,
            ts_end_ms: 0,
        }
    }

    #[tokio::test]
    async fn subscriber_receives_published_message() {
        let hub = OverlayHub::new();
        let mut rx = hub.subscribe("tok-1");
        hub.publish("tok-1", subtitle(1));
        let received = rx.recv().await.unwrap();
        assert!(matches!(received, ServerMessage::Subtitle { sequence: 1, .. }));
    }

    #[tokio::test]
    async fn publish_without_subscriber_is_noop() {
        let hub = OverlayHub::new();
        // Kimse abone değilken publish panic/hata vermemeli.
        hub.publish("tok-nobody", subtitle(1));
    }

    #[tokio::test]
    async fn tokens_are_isolated() {
        let hub = OverlayHub::new();
        let mut rx_a = hub.subscribe("tok-a");
        let mut rx_b = hub.subscribe("tok-b");
        hub.publish("tok-a", subtitle(1));
        let got = rx_a.recv().await.unwrap();
        assert!(matches!(got, ServerMessage::Subtitle { sequence: 1, .. }));
        // tok-b'ye hiçbir şey yayınlanmadı.
        assert!(rx_b.try_recv().is_err());
    }
}
