//! Twitch Extension EBS (Extension Backend Service) yüzeyi — Faz 2.4.
//!
//! Bkz. `docs/twitch-extension-policy.md` ve `docs/twitch-extension-plan.md` (B2).
//!
//! İki route ailesi:
//! - `/api/ext/twitch/*` — StreamDubber dashboard'ı (kendi JWT'miz,
//!   [`AuthUser`]): bağlama kodu üretme, binding görüntüleme/silme.
//! - `/ext/twitch/*` — Twitch CDN'deki extension frontend'i (Twitch extension
//!   JWT, [`TwitchExtVerifier`]): kod claim'i (broadcaster config), caption
//!   state (poll fallback) ve canlı caption WebSocket'i (viewer).
//!
//! Politika gereği sabitler:
//! - Overlay token'ı viewer'a ASLA dönmez — binding üzerinden sunucu içinde
//!   çözülür ve yalnız `subtitle` mesajları iletilir.
//! - Tüm yüzey `ext.twitch.enabled` config'i kapalıyken 503 döner (varsayılan
//!   kapalı; admin panelden açılır, restart yok).
//! - Per-viewer istek limiti + per-channel eşzamanlı izleyici limiti
//!   config-driven'dır (policy'nin abuse koruması şartı).
//! - Hata gövdeleri secret/iç detay içermez; loglara token yazılmaz.

pub mod jwt;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::auth::authz::AuthUser;
use crate::bindings::{self, ChannelBindingStore};
use crate::config::ServiceConfig;
use crate::logging::{LogLevel, Logger};
use crate::overlay::{ActiveStreams, OverlayError, OverlayHub};
use crate::pairing::{PairingError, PairingStore, CODE_LEN};
use crate::secrets::{names, ManagedSecrets, SecretRef, SecretStore};
use crate::ws::protocol::ServerMessage;

use jwt::{ExtJwtError, TwitchExtClaims, TwitchExtVerifier};

/// Bu modülün platform kimliği (`channel_bindings.platform`).
const PLATFORM: &str = "twitch";
/// Config yok/bozuksa per-channel eşzamanlı viewer limiti.
const DEFAULT_MAX_VIEWERS: usize = 200;
/// Config yok/bozuksa per-viewer state isteği limiti (dakikada).
const DEFAULT_STATE_RATE_PER_MIN: u32 = 60;
/// Rate limit penceresi.
const RATE_WINDOW: Duration = Duration::from_secs(60);
/// Rate map bu boyutu aşarsa süresi geçmiş pencereler ayıklanır.
const RATE_PRUNE_THRESHOLD: usize = 4096;

/// EBS'in paylaşılan durumu.
pub struct ExtTwitch {
    verifier: TwitchExtVerifier,
    /// Dashboard'ın ürettiği kanal bağlama kodları — SDK pairing'le AYNI tip,
    /// AYRI instance (kod uzayları çakışmaz).
    link_codes: PairingStore,
    /// opaque_user_id → (pencere başlangıcı, istek sayısı).
    rate: Mutex<HashMap<String, (Instant, u32)>>,
    /// channel_id → aktif caption WS sayısı.
    viewers: Mutex<HashMap<String, usize>>,
}

impl ExtTwitch {
    pub fn new(verifier: TwitchExtVerifier) -> Self {
        ExtTwitch {
            verifier,
            link_codes: PairingStore::new(),
            rate: Mutex::new(HashMap::new()),
            viewers: Mutex::new(HashMap::new()),
        }
    }

    /// Secret'ları [`ManagedSecrets`]'tan çağrı anında çözen kurulum —
    /// panel/env/Vault'tan girilen secret restart'sız devreye girer.
    pub fn from_secrets(secrets: &Arc<ManagedSecrets>) -> Self {
        let store: Arc<dyn SecretStore> = Arc::clone(secrets) as Arc<dyn SecretStore>;
        ExtTwitch::new(TwitchExtVerifier::new(
            SecretRef::store(Arc::clone(&store), names::TWITCH_EXT_SECRET),
            SecretRef::store(store, names::TWITCH_EXT_SECRET_PREVIOUS),
        ))
    }

    /// Sabit pencereli per-viewer limit. Pencere dolmuşsa sıfırlanır.
    fn check_rate(&self, key: &str, limit: u32) -> bool {
        let Ok(mut map) = self.rate.lock() else {
            return false;
        };
        if map.len() > RATE_PRUNE_THRESHOLD {
            map.retain(|_, (start, _)| start.elapsed() < RATE_WINDOW);
        }
        let now = Instant::now();
        let entry = map.entry(key.to_string()).or_insert((now, 0));
        if entry.0.elapsed() >= RATE_WINDOW {
            *entry = (now, 0);
        }
        entry.1 += 1;
        entry.1 <= limit
    }

    /// Kanal için viewer slotu ayırır; limit doluysa `None`. Dönen guard
    /// düşünce sayaç azalır (WS kopması dahil her çıkış yolunda).
    fn acquire_viewer(self: &Arc<Self>, channel_id: &str, max: usize) -> Option<ViewerSlot> {
        let mut map = self.viewers.lock().ok()?;
        let count = map.entry(channel_id.to_string()).or_insert(0);
        if *count >= max {
            return None;
        }
        *count += 1;
        Some(ViewerSlot {
            ext: Arc::clone(self),
            channel_id: channel_id.to_string(),
        })
    }

    fn release_viewer(&self, channel_id: &str) {
        if let Ok(mut map) = self.viewers.lock() {
            if let Some(count) = map.get_mut(channel_id) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    map.remove(channel_id);
                }
            }
        }
    }
}

/// RAII viewer sayacı — drop'ta slotu bırakır.
struct ViewerSlot {
    ext: Arc<ExtTwitch>,
    channel_id: String,
}

impl Drop for ViewerSlot {
    fn drop(&mut self) {
        self.ext.release_viewer(&self.channel_id);
    }
}

/// Pozitif tamsayı config değeri; yok/bozuksa varsayılan.
fn cfg_num<T: std::str::FromStr + PartialOrd + Default>(
    cfg: &ServiceConfig,
    key: &str,
    default: T,
) -> T {
    cfg.get(key)
        .and_then(|v| v.parse::<T>().ok())
        .filter(|n| *n > T::default())
        .unwrap_or(default)
}

/// `ext.twitch.enabled` kapalıysa 503 — tüm handler'ların ilk kontrolü.
fn ensure_enabled(cfg: &ServiceConfig) -> Result<(), (StatusCode, String)> {
    if cfg.get("ext.twitch.enabled").as_deref() == Some("true") {
        Ok(())
    } else {
        Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "twitch extension disabled".to_string(),
        ))
    }
}

/// `Authorization: Bearer <twitch-ext-jwt>` başlığından token'ı alır.
fn bearer(headers: &HeaderMap) -> Result<&str, (StatusCode, String)> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .ok_or((
            StatusCode::UNAUTHORIZED,
            "missing bearer token".to_string(),
        ))
}

/// JWT hatası → HTTP durumu. Secret detayı sızdırmaz.
fn jwt_status(err: ExtJwtError) -> (StatusCode, String) {
    match err {
        ExtJwtError::NotConfigured => (
            StatusCode::SERVICE_UNAVAILABLE,
            "twitch extension not configured".to_string(),
        ),
        ExtJwtError::Expired | ExtJwtError::Invalid => {
            (StatusCode::UNAUTHORIZED, "invalid token".to_string())
        }
    }
}

/// Claim'deki kanal, istenen kanalla eşleşmek zorunda (yatay yetki koruması).
fn ensure_channel(
    claims: &TwitchExtClaims,
    channel_id: &str,
) -> Result<(), (StatusCode, String)> {
    if claims.channel_id.as_deref() == Some(channel_id) {
        Ok(())
    } else {
        Err((StatusCode::FORBIDDEN, "channel mismatch".to_string()))
    }
}

/// Rate limit anahtarı: opaque viewer id, yoksa kanal.
fn rate_key(claims: &TwitchExtClaims, channel_id: &str) -> String {
    claims
        .opaque_user_id
        .clone()
        .unwrap_or_else(|| format!("ch:{channel_id}"))
}

// ---------------------------------------------------------------------------
// Dashboard tarafı (StreamDubber JWT)
// ---------------------------------------------------------------------------

/// `POST /api/ext/twitch/code` yanıtı — dashboard bu kodu gösterir, yayıncı
/// Twitch config sayfasına yazar.
#[derive(Debug, Serialize)]
pub struct LinkCodeResponse {
    pub code: String,
    pub expires_in_secs: u64,
}

/// `POST /api/ext/twitch/code` (yayıncı, StreamDubber auth).
pub async fn create_link_code(
    user: AuthUser,
    State(cfg): State<Arc<ServiceConfig>>,
    State(ext): State<Arc<ExtTwitch>>,
    State(logger): State<Arc<Logger>>,
) -> Result<Json<LinkCodeResponse>, (StatusCode, String)> {
    ensure_enabled(&cfg)?;
    let code = ext
        .link_codes
        .issue(&user.claims.sub, &user.claims.plan)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    logger.log(
        LogLevel::Info,
        "ext_twitch",
        "channel link code issued",
        Some(&user.claims.sub),
        None,
    );
    Ok(Json(LinkCodeResponse {
        code,
        expires_in_secs: ext.link_codes.ttl_secs(),
    }))
}

/// `GET /api/ext/twitch/binding` yanıtı.
#[derive(Debug, Serialize)]
pub struct BindingStatusResponse {
    pub linked: bool,
    pub channels: Vec<String>,
}

/// `GET /api/ext/twitch/binding` (yayıncı) — dashboard durum kartı.
pub async fn get_binding(
    user: AuthUser,
    State(cfg): State<Arc<ServiceConfig>>,
    State(bindings): State<Arc<ChannelBindingStore>>,
) -> Result<Json<BindingStatusResponse>, (StatusCode, String)> {
    ensure_enabled(&cfg)?;
    let channels: Vec<String> = bindings
        .for_user(PLATFORM, &user.claims.sub)
        .into_iter()
        .map(|b| b.channel_id)
        .collect();
    Ok(Json(BindingStatusResponse {
        linked: !channels.is_empty(),
        channels,
    }))
}

/// `DELETE /api/ext/twitch/binding` (yayıncı) — tüm Twitch bağlarını kaldırır.
pub async fn delete_binding(
    user: AuthUser,
    State(cfg): State<Arc<ServiceConfig>>,
    State(bindings): State<Arc<ChannelBindingStore>>,
    State(db): State<Option<sqlx::PgPool>>,
    State(logger): State<Arc<Logger>>,
) -> Result<Json<BindingStatusResponse>, (StatusCode, String)> {
    ensure_enabled(&cfg)?;
    let removed = bindings.unbind_user(PLATFORM, &user.claims.sub);
    if removed > 0 {
        if let Some(pool) = &db {
            bindings::delete_bindings_for_user(pool, PLATFORM, &user.claims.sub, &logger).await;
        }
        logger.log(
            LogLevel::Info,
            "ext_twitch",
            &format!("channel binding removed ({removed})"),
            Some(&user.claims.sub),
            None,
        );
    }
    Ok(Json(BindingStatusResponse {
        linked: false,
        channels: Vec::new(),
    }))
}

// ---------------------------------------------------------------------------
// Extension tarafı (Twitch extension JWT)
// ---------------------------------------------------------------------------

/// `POST /ext/twitch/bind` gövdesi — broadcaster config sayfasından gelir.
#[derive(Debug, Deserialize)]
pub struct BindRequest {
    pub code: String,
}

/// `POST /ext/twitch/bind` yanıtı.
#[derive(Debug, Serialize)]
pub struct BindResponse {
    pub status: String,
    pub channel_id: String,
}

/// `POST /ext/twitch/bind` — Twitch config sayfası, dashboard'daki kodu
/// claim ederek kanalı hesaba bağlar. Yalnız `role=broadcaster`.
pub async fn bind_channel(
    State(cfg): State<Arc<ServiceConfig>>,
    State(ext): State<Arc<ExtTwitch>>,
    State(bindings): State<Arc<ChannelBindingStore>>,
    State(db): State<Option<sqlx::PgPool>>,
    State(logger): State<Arc<Logger>>,
    headers: HeaderMap,
    Json(req): Json<BindRequest>,
) -> Result<Json<BindResponse>, (StatusCode, String)> {
    ensure_enabled(&cfg)?;
    let claims = ext
        .verifier
        .verify(bearer(&headers)?)
        .await
        .map_err(jwt_status)?;
    if claims.role.as_deref() != Some("broadcaster") {
        return Err((StatusCode::FORBIDDEN, "broadcaster role required".to_string()));
    }
    let Some(channel_id) = claims.channel_id.clone().filter(|c| !c.is_empty()) else {
        return Err((StatusCode::FORBIDDEN, "channel_id claim required".to_string()));
    };

    let code = req.code.trim();
    if code.len() != CODE_LEN || !code.bytes().all(|b| b.is_ascii_digit()) {
        return Err((StatusCode::BAD_REQUEST, PairingError::BadFormat.to_string()));
    }
    let (user_id, _plan) = ext.link_codes.claim(code).map_err(|e| {
        let status = match e {
            PairingError::UnknownCode => StatusCode::NOT_FOUND,
            PairingError::Expired => StatusCode::GONE,
            PairingError::BadFormat => StatusCode::BAD_REQUEST,
            PairingError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, e.to_string())
    })?;

    let binding = bindings.bind(PLATFORM, &channel_id, &user_id);
    if let Some(pool) = &db {
        bindings::persist_binding(pool, &binding, &logger).await;
    }
    logger.log(
        LogLevel::Info,
        "ext_twitch",
        &format!("channel {channel_id} linked"),
        Some(&user_id),
        None,
    );
    Ok(Json(BindResponse {
        status: "linked".to_string(),
        channel_id,
    }))
}

/// `GET /ext/twitch/state/{channel_id}` yanıtı — poll fallback + ilk açılış.
///
/// `lines` alanı overlay snapshot'ındaki `subtitle` mesajlarıdır (WS ile
/// birebir aynı şekil) — viewer iki transport arasında tek parser kullanır.
#[derive(Debug, Serialize)]
pub struct ExtStateResponse {
    pub configured: bool,
    pub live: bool,
    pub default_lang: String,
    /// Kaynak dil + aktif çeviri dilleri (son altyazıdan türetilir).
    pub languages: Vec<String>,
    pub lines: Vec<ServerMessage>,
}

/// `GET /ext/twitch/state/{channel_id}` — herhangi bir Twitch rolü.
pub async fn get_state(
    State(cfg): State<Arc<ServiceConfig>>,
    State(ext): State<Arc<ExtTwitch>>,
    State(bindings): State<Arc<ChannelBindingStore>>,
    State(active): State<Arc<ActiveStreams>>,
    State(hub): State<Arc<OverlayHub>>,
    Path(channel_id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    ensure_enabled(&cfg)?;
    let claims = ext
        .verifier
        .verify(bearer(&headers)?)
        .await
        .map_err(jwt_status)?;
    ensure_channel(&claims, &channel_id)?;

    let limit = cfg_num(&cfg, "ext.twitch.state_rate_per_min", DEFAULT_STATE_RATE_PER_MIN);
    if !ext.check_rate(&rate_key(&claims, &channel_id), limit) {
        return Err((StatusCode::TOO_MANY_REQUESTS, "rate limited".to_string()));
    }

    let default_lang = cfg
        .get("ext.twitch.default_lang")
        .unwrap_or_else(|| "en".to_string());

    let binding = bindings.lookup(PLATFORM, &channel_id);
    let response = match binding {
        None => ExtStateResponse {
            configured: false,
            live: false,
            default_lang,
            languages: Vec::new(),
            lines: Vec::new(),
        },
        Some(binding) => {
            let overlay_token = active.get(&binding.user_id);
            let lines = overlay_token
                .as_deref()
                .map(|token| hub.snapshot(token))
                .unwrap_or_default();
            ExtStateResponse {
                configured: true,
                live: overlay_token.is_some(),
                default_lang,
                languages: languages_of(&lines),
                lines,
            }
        }
    };

    // Kısa istemci-yanı cache: caption temposunda poll baskısını düşürür,
    // yanıt zaten snapshot olduğundan tazelik kaybı ihmal edilebilir.
    Ok((
        [(header::CACHE_CONTROL, "private, max-age=2")],
        Json(response),
    ))
}

/// Son altyazıdan dil listesi: kaynak dil + çeviri dilleri (sıralı).
fn languages_of(lines: &[ServerMessage]) -> Vec<String> {
    let Some(ServerMessage::Subtitle {
        source,
        translations,
        ..
    }) = lines.last()
    else {
        return Vec::new();
    };
    let mut langs = vec![source.lang.clone()];
    for t in translations {
        if !langs.contains(&t.lang) {
            langs.push(t.lang.clone());
        }
    }
    langs
}

/// `GET /ext/twitch/captions/{channel_id}?token=…` sorgu parametreleri.
///
/// Tarayıcı `WebSocket` API'si başlık gönderemediği için Twitch JWT query
/// string'te taşınır. Token kısa ömürlüdür; URL loglanmaz (aşağıda yalnız
/// kanal id'si loglanır).
#[derive(Debug, Deserialize)]
pub struct CaptionsQuery {
    #[serde(default)]
    pub token: String,
}

/// `GET /ext/twitch/captions/{channel_id}` — canlı caption WebSocket'i.
///
/// Upgrade ÖNCESİ tüm kontroller yapılır: gate → JWT → kanal eşleşmesi →
/// binding → canlı yayın → viewer slotu. Upgrade sonrası yalnız `subtitle`
/// mesajları iletilir (overlay token ve diğer pipeline mesajları sızmaz).
pub async fn captions_ws(
    State(state): State<crate::state::AppState>,
    Path(channel_id): Path<String>,
    Query(query): Query<CaptionsQuery>,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let cfg = &state.service_config;
    let ext = &state.ext_twitch;
    ensure_enabled(cfg)?;
    if query.token.is_empty() {
        return Err((StatusCode::UNAUTHORIZED, "missing token".to_string()));
    }
    let claims = ext.verifier.verify(&query.token).await.map_err(jwt_status)?;
    ensure_channel(&claims, &channel_id)?;

    let Some(binding) = state.bindings.lookup(PLATFORM, &channel_id) else {
        return Err((StatusCode::NOT_FOUND, "channel not configured".to_string()));
    };
    let Some(overlay_token) = state.active_streams.get(&binding.user_id) else {
        return Err((StatusCode::CONFLICT, "channel not live".to_string()));
    };

    let max_viewers = cfg_num(cfg, "ext.twitch.max_viewers_per_channel", DEFAULT_MAX_VIEWERS);
    let Some(slot) = ext.acquire_viewer(&channel_id, max_viewers) else {
        return Err((StatusCode::TOO_MANY_REQUESTS, "viewer limit reached".to_string()));
    };

    let subscription = state.overlay.subscribe(&overlay_token).map_err(
        |OverlayError::SubscriberLimit| {
            (StatusCode::TOO_MANY_REQUESTS, "caption capacity reached".to_string())
        },
    )?;

    state.logger.log(
        LogLevel::Info,
        "ext_twitch",
        &format!("caption viewer connected (channel {channel_id})"),
        None,
        None,
    );

    Ok(ws.on_upgrade(move |socket| forward_captions(socket, subscription, slot)))
}

/// Snapshot replay + canlı akış; yalnız `subtitle` mesajları iletilir.
/// `_slot` bağlantı boyunca viewer sayacını tutar, çıkışta bırakır.
async fn forward_captions(
    mut socket: axum::extract::ws::WebSocket,
    subscription: crate::overlay::OverlaySubscription,
    _slot: ViewerSlot,
) {
    use axum::extract::ws::{Message, Utf8Bytes};

    let mut rx = subscription.receiver;
    let replay = subscription.replay;

    let send = |msg: ServerMessage| serde_json::to_string(&msg).ok();

    for msg in replay {
        let Some(json) = send(msg) else { continue };
        if socket.send(Message::Text(Utf8Bytes::from(json))).await.is_err() {
            return;
        }
    }

    loop {
        tokio::select! {
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    // Viewer'dan veri kabul edilmez — kanal tek yönlü.
                    _ => {}
                }
            }
            msg = rx.recv() => {
                let Ok(msg) = msg else { break };
                if !matches!(msg, ServerMessage::Subtitle { .. }) {
                    continue;
                }
                let Some(json) = send(msg) else { continue };
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

    fn ext() -> Arc<ExtTwitch> {
        Arc::new(ExtTwitch::new(TwitchExtVerifier::new(
            SecretRef::none(),
            SecretRef::none(),
        )))
    }

    #[test]
    fn rate_limit_resets_after_window() {
        let ext = ext();
        assert!(ext.check_rate("v1", 2));
        assert!(ext.check_rate("v1", 2));
        assert!(!ext.check_rate("v1", 2));
        // Farklı anahtar etkilenmez.
        assert!(ext.check_rate("v2", 2));
    }

    #[test]
    fn viewer_slots_are_released_on_drop() {
        let ext = ext();
        let first = ext.acquire_viewer("ch-1", 1).expect("first slot");
        assert!(ext.acquire_viewer("ch-1", 1).is_none());
        // Farklı kanal etkilenmez.
        assert!(ext.acquire_viewer("ch-2", 1).is_some());
        drop(first);
        assert!(ext.acquire_viewer("ch-1", 1).is_some());
    }

    #[test]
    fn languages_derived_from_last_subtitle() {
        use crate::ws::protocol::{SubtitleSource, Translation};
        let lines = vec![ServerMessage::Subtitle {
            session_id: "s1".into(),
            sequence: 1,
            source: SubtitleSource {
                lang: "tr".into(),
                text: "merhaba".into(),
                is_final: true,
            },
            translations: vec![
                Translation { lang: "en".into(), text: "hello".into() },
                Translation { lang: "de".into(), text: "hallo".into() },
            ],
            dubbed_audio: vec![],
            ts_start_ms: 0,
            ts_end_ms: 0,
        }];
        assert_eq!(languages_of(&lines), vec!["tr", "en", "de"]);
        assert!(languages_of(&[]).is_empty());
    }
}
