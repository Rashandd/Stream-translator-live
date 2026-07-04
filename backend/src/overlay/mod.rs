//! Genel altyazı overlay yayını (Faz 1.9) — OBS Browser Source için gerçek
//! zamanlı altyazı dağıtımı.
//!
//! Her stream session'ı `stream_start`'ta rastgele, tahmin edilemez bir
//! overlay token alır (bkz. `ws::mod::handle_control`, `StreamAck.overlay_token`).
//! Backend, o session için üretilen her `subtitle` mesajını bu hub üzerinden
//! token'a abone tüm istemcilere (OBS Browser Source, dashboard canlı
//! önizleme, Twitch Extension EBS) yayınlar. Token, stream key'le aynı güven
//! modelini kullanır: kendisi bir capability'dir, ayrıca JWT gerektirmez.
//!
//! Faz 2 hazırlığı (A1): token başına abone üst sınırı ve son N final
//! altyazının snapshot'ı — yeni bağlanan istemci boş ekran yerine son
//! satırları görür, extension viewer fan-out'u sınırlanabilir. İki değer de
//! config-driven'dır (`overlay.max_subscribers`, `overlay.snapshot_lines`),
//! restart gerektirmez.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use axum::extract::ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use tokio::sync::broadcast;

use crate::config::ServiceConfig;
use crate::ws::protocol::ServerMessage;

const CHANNEL_CAPACITY: usize = 32;
/// Config anahtarı yok/bozuksa geçerli abone limiti.
const DEFAULT_MAX_SUBSCRIBERS: usize = 100;
/// Config anahtarı yok/bozuksa snapshot satır sayısı.
const DEFAULT_SNAPSHOT_LINES: usize = 3;

/// Bir token'ın canlı kanalı + son final altyazı snapshot'ı.
struct Channel {
    sender: broadcast::Sender<ServerMessage>,
    snapshot: VecDeque<ServerMessage>,
}

impl Channel {
    fn new() -> Self {
        Channel {
            sender: broadcast::channel(CHANNEL_CAPACITY).0,
            snapshot: VecDeque::new(),
        }
    }
}

/// Abonelik reddi.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum OverlayError {
    #[error("overlay subscriber limit reached")]
    SubscriberLimit,
}

/// Başarılı abonelik: önce `replay` satırları gönderilir, sonra `receiver`
/// canlı akışı taşır.
pub struct OverlaySubscription {
    pub receiver: broadcast::Receiver<ServerMessage>,
    pub replay: Vec<ServerMessage>,
}

/// Token → broadcast kanalı eşlemesini tutan hub.
pub struct OverlayHub {
    channels: Mutex<HashMap<String, Channel>>,
    config: Arc<ServiceConfig>,
}

impl OverlayHub {
    /// Boş bir hub kurar. Limitler çağrı anında config'ten okunur.
    pub fn new(config: Arc<ServiceConfig>) -> Self {
        OverlayHub {
            channels: Mutex::new(HashMap::new()),
            config,
        }
    }

    /// Pozitif tamsayı config değeri; yok/bozuksa varsayılan.
    fn cfg_usize(&self, key: &str, default: usize) -> usize {
        self.config
            .get(key)
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|n| *n > 0)
            .unwrap_or(default)
    }

    /// Bir mesajı token'a abone tüm istemcilere yayınlar. Hiç dinleyen yoksa
    /// canlı gönderim no-op'tur (broadcast semantiği — hata değildir); final
    /// altyazılar yine de snapshot'a yazılır ki sonra bağlanan istemci görsün.
    pub fn publish(&self, token: &str, msg: ServerMessage) {
        let snapshot_lines = self.cfg_usize("overlay.snapshot_lines", DEFAULT_SNAPSHOT_LINES);
        let Ok(mut map) = self.channels.lock() else {
            return;
        };
        let channel = map.entry(token.to_string()).or_insert_with(Channel::new);
        if matches!(&msg, ServerMessage::Subtitle { source, .. } if source.is_final) {
            channel.snapshot.push_back(msg.clone());
            while channel.snapshot.len() > snapshot_lines {
                channel.snapshot.pop_front();
            }
        }
        let _ = channel.sender.send(msg);
    }

    /// Bir token'a abone olur — WS handler önce `replay`'i istemciye iletir,
    /// sonra `receiver`'dan okur. Token başına abone sayısı
    /// `overlay.max_subscribers` ile sınırlıdır (extension fan-out koruması).
    pub fn subscribe(&self, token: &str) -> Result<OverlaySubscription, OverlayError> {
        let max = self.cfg_usize("overlay.max_subscribers", DEFAULT_MAX_SUBSCRIBERS);
        let Ok(mut map) = self.channels.lock() else {
            return Err(OverlayError::SubscriberLimit);
        };
        let channel = map.entry(token.to_string()).or_insert_with(Channel::new);
        if channel.sender.receiver_count() >= max {
            return Err(OverlayError::SubscriberLimit);
        }
        Ok(OverlaySubscription {
            receiver: channel.sender.subscribe(),
            replay: channel.snapshot.iter().cloned().collect(),
        })
    }

    /// Bir token'ın snapshot'ındaki son final altyazılar (eski → yeni).
    /// Extension EBS'in poll/state endpoint'i buradan okur — abone olmadan.
    pub fn snapshot(&self, token: &str) -> Vec<ServerMessage> {
        self.channels
            .lock()
            .ok()
            .and_then(|map| map.get(token).map(|c| c.snapshot.iter().cloned().collect()))
            .unwrap_or_default()
    }
}

/// Kullanıcı → aktif yayının overlay token'ı kaydı (Faz 2.4 EBS çözümlemesi).
///
/// `ws::handle_control` stream_start'ta yazar, stream_stop / bağlantı kopması /
/// plan limitinde siler. Twitch Extension EBS'i, kanal binding'inden bulduğu
/// kullanıcının canlı yayınına buradan ulaşır — overlay token'ı **sunucu
/// içinde** çözülür, extension viewer'a asla gönderilmez (token capability'dir).
pub struct ActiveStreams {
    map: Mutex<HashMap<String, String>>,
}

impl ActiveStreams {
    pub fn new() -> Self {
        ActiveStreams {
            map: Mutex::new(HashMap::new()),
        }
    }

    /// Kullanıcının aktif overlay token'ını kaydeder (son yazan kazanır —
    /// SDK yeniden bağlanınca yeni session eskisini geçersiz kılar).
    pub fn set(&self, user_id: &str, overlay_token: &str) {
        if let Ok(mut map) = self.map.lock() {
            map.insert(user_id.to_string(), overlay_token.to_string());
        }
    }

    /// Kaydı yalnız hâlâ aynı token'ı gösteriyorsa siler — eski session'ın
    /// gecikmiş temizliği, yeni session'ın kaydını ezmesin.
    pub fn clear_if(&self, user_id: &str, overlay_token: &str) {
        if let Ok(mut map) = self.map.lock() {
            if map.get(user_id).map(String::as_str) == Some(overlay_token) {
                map.remove(user_id);
            }
        }
    }

    /// Kullanıcının aktif overlay token'ı (yayında değilse `None`).
    pub fn get(&self, user_id: &str) -> Option<String> {
        self.map.lock().ok().and_then(|map| map.get(user_id).cloned())
    }
}

impl Default for ActiveStreams {
    fn default() -> Self {
        Self::new()
    }
}

/// `GET /overlay/{token}` — OBS Browser Source / dashboard önizleme için genel
/// (auth gerektirmeyen) altyazı WebSocket'i. Token bilinmiyorsa bağlantı
/// açılır ama hiç mesaj gelmez (hub token'ı henüz görmemiş olabilir — SDK
/// `stream_start` yapmadan yayıncı overlay URL'sini açabilir).
pub async fn overlay_ws_handler(
    State(hub): State<Arc<OverlayHub>>,
    State(logger): State<Arc<crate::logging::Logger>>,
    Path(token): Path<String>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    // Token bir capability'dir — log'a tamamı yazılmaz, sadece referans öneki.
    let token_ref: String = token.chars().take(8).collect();
    logger.log(
        crate::logging::LogLevel::Info,
        "overlay",
        &format!("overlay subscriber connected (token {token_ref}…)"),
        None,
        None,
    );
    ws.on_upgrade(move |socket| forward_to_client(socket, hub, logger, token))
}

/// Önce snapshot'ı, sonra hub'dan gelen canlı mesajları istemciye JSON text
/// frame olarak iletir; istemci kapatana veya hata verene kadar döner. Abone
/// limiti dolmuşsa bağlantı sessizce kapatılır (log'a warn düşer).
async fn forward_to_client(
    mut socket: WebSocket,
    hub: Arc<OverlayHub>,
    logger: Arc<crate::logging::Logger>,
    token: String,
) {
    let subscription = match hub.subscribe(&token) {
        Ok(sub) => sub,
        Err(OverlayError::SubscriberLimit) => {
            let token_ref: String = token.chars().take(8).collect();
            logger.log(
                crate::logging::LogLevel::Warn,
                "overlay",
                &format!("overlay subscriber rejected: limit reached (token {token_ref}…)"),
                None,
                None,
            );
            return;
        }
    };

    let mut rx = subscription.receiver;
    for msg in subscription.replay {
        if send_json(&mut socket, &msg).await.is_err() {
            return;
        }
    }

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
                if send_json(&mut socket, &msg).await.is_err() {
                    break;
                }
            }
        }
    }
}

/// Bir mesajı JSON text frame olarak gönderir; serialize edilemeyen mesaj atlanır.
async fn send_json(socket: &mut WebSocket, msg: &ServerMessage) -> Result<(), axum::Error> {
    let Ok(json) = serde_json::to_string(msg) else {
        return Ok(());
    };
    socket.send(Message::Text(Utf8Bytes::from(json))).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ws::protocol::{ServerMessage, SubtitleSource};

    fn test_hub() -> (OverlayHub, Arc<ServiceConfig>) {
        let config = Arc::new(ServiceConfig::with_defaults());
        (OverlayHub::new(Arc::clone(&config)), config)
    }

    fn subtitle(seq: u64) -> ServerMessage {
        ServerMessage::Subtitle {
            session_id: "s1".into(),
            sequence: seq,
            source: SubtitleSource {
                lang: "tr".into(),
                text: format!("merhaba {seq}"),
                is_final: true,
            },
            translations: vec![],
            ts_start_ms: 0,
            ts_end_ms: 0,
        }
    }

    fn interim_subtitle(seq: u64) -> ServerMessage {
        match subtitle(seq) {
            ServerMessage::Subtitle {
                session_id,
                sequence,
                mut source,
                translations,
                ts_start_ms,
                ts_end_ms,
            } => {
                source.is_final = false;
                ServerMessage::Subtitle {
                    session_id,
                    sequence,
                    source,
                    translations,
                    ts_start_ms,
                    ts_end_ms,
                }
            }
            other => other,
        }
    }

    #[tokio::test]
    async fn subscriber_receives_published_message() {
        let (hub, _) = test_hub();
        let mut sub = hub.subscribe("tok-1").expect("subscribe");
        hub.publish("tok-1", subtitle(1));
        let received = sub.receiver.recv().await.expect("recv");
        assert!(matches!(received, ServerMessage::Subtitle { sequence: 1, .. }));
    }

    #[tokio::test]
    async fn publish_without_subscriber_is_noop() {
        let (hub, _) = test_hub();
        // Kimse abone değilken publish panic/hata vermemeli.
        hub.publish("tok-nobody", subtitle(1));
    }

    #[tokio::test]
    async fn tokens_are_isolated() {
        let (hub, _) = test_hub();
        let mut sub_a = hub.subscribe("tok-a").expect("subscribe a");
        let mut sub_b = hub.subscribe("tok-b").expect("subscribe b");
        hub.publish("tok-a", subtitle(1));
        let got = sub_a.receiver.recv().await.expect("recv");
        assert!(matches!(got, ServerMessage::Subtitle { sequence: 1, .. }));
        // tok-b'ye hiçbir şey yayınlanmadı.
        assert!(sub_b.receiver.try_recv().is_err());
        assert!(sub_b.replay.is_empty());
    }

    #[tokio::test]
    async fn late_subscriber_gets_snapshot_replay() {
        let (hub, _) = test_hub();
        hub.publish("tok-1", subtitle(1));
        hub.publish("tok-1", subtitle(2));
        let sub = hub.subscribe("tok-1").expect("subscribe");
        let seqs: Vec<u64> = sub
            .replay
            .iter()
            .map(|m| match m {
                ServerMessage::Subtitle { sequence, .. } => *sequence,
                _ => panic!("snapshot yalnız subtitle içermeli"),
            })
            .collect();
        assert_eq!(seqs, vec![1, 2]);
    }

    #[tokio::test]
    async fn snapshot_keeps_only_last_n_lines() {
        let (hub, config) = test_hub();
        config.set("overlay.snapshot_lines", "2");
        for seq in 1..=5 {
            hub.publish("tok-1", subtitle(seq));
        }
        let snapshot = hub.snapshot("tok-1");
        let seqs: Vec<u64> = snapshot
            .iter()
            .map(|m| match m {
                ServerMessage::Subtitle { sequence, .. } => *sequence,
                _ => panic!("snapshot yalnız subtitle içermeli"),
            })
            .collect();
        assert_eq!(seqs, vec![4, 5]);
    }

    #[tokio::test]
    async fn interim_and_non_subtitle_messages_are_not_snapshotted() {
        let (hub, _) = test_hub();
        hub.publish("tok-1", interim_subtitle(1));
        hub.publish("tok-1", ServerMessage::Pong { ts: 1 });
        assert!(hub.snapshot("tok-1").is_empty());
        // Canlı akışta yine de iletilirler.
        let mut sub = hub.subscribe("tok-1").expect("subscribe");
        hub.publish("tok-1", interim_subtitle(2));
        let got = sub.receiver.recv().await.expect("recv");
        assert!(matches!(
            got,
            ServerMessage::Subtitle { source: SubtitleSource { is_final: false, .. }, .. }
        ));
    }

    #[tokio::test]
    async fn subscriber_limit_is_enforced() {
        let (hub, config) = test_hub();
        config.set("overlay.max_subscribers", "1");
        let first = hub.subscribe("tok-1").expect("first subscribe");
        assert_eq!(
            hub.subscribe("tok-1").err(),
            Some(OverlayError::SubscriberLimit)
        );
        // Farklı token etkilenmez.
        assert!(hub.subscribe("tok-2").is_ok());
        // İlk abone düşünce yer açılır.
        drop(first);
        assert!(hub.subscribe("tok-1").is_ok());
    }

    #[tokio::test]
    async fn bogus_limit_config_falls_back_to_default() {
        let (hub, config) = test_hub();
        config.set("overlay.max_subscribers", "not-a-number");
        // Varsayılan (100) geçerli — abonelik reddedilmez.
        assert!(hub.subscribe("tok-1").is_ok());
    }

    #[test]
    fn active_streams_set_get_roundtrip() {
        let streams = ActiveStreams::new();
        assert_eq!(streams.get("u-1"), None);
        streams.set("u-1", "tok-a");
        assert_eq!(streams.get("u-1").as_deref(), Some("tok-a"));
    }

    #[test]
    fn active_streams_last_writer_wins() {
        let streams = ActiveStreams::new();
        streams.set("u-1", "tok-a");
        streams.set("u-1", "tok-b");
        assert_eq!(streams.get("u-1").as_deref(), Some("tok-b"));
    }

    #[test]
    fn active_streams_clear_if_only_matches_current_token() {
        let streams = ActiveStreams::new();
        streams.set("u-1", "tok-b");
        // Eski session'ın gecikmiş temizliği yeni kaydı silmemeli.
        streams.clear_if("u-1", "tok-a");
        assert_eq!(streams.get("u-1").as_deref(), Some("tok-b"));
        streams.clear_if("u-1", "tok-b");
        assert_eq!(streams.get("u-1"), None);
    }
}
