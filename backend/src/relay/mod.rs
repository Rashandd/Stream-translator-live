//! SDK relay hedefleri — stream key senkronizasyonu backend ucu.
//!
//! Yayıncı panelden hedef listesini (`platform`, RTMP URL, stream key)
//! kaydeder; eşleşmiş SDK aynı listeyi `GET` ile çeker ve gömülü RTMP
//! relay'ini (1.6) bu hedeflerle kurar. Stream key'ler at-rest AES-256-GCM
//! şifrelidir: kayıtlar `RELAY_FILE`'da tek şifreli blob olarak durur —
//! düz metin key diske asla yazılmaz. Transit TLS 1.3 (CLAUDE.md: üstüne
//! ayrıca AES yok).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::auth::authz::AuthUser;
use crate::crypto::Cipher;
use crate::logging::{LogLevel, Logger};

/// Kullanıcı başına hedef sınırı (yanlışlıkla dev listeler / kötüye kullanım).
const MAX_TARGETS: usize = 8;

/// Bir RTMP dağıtım hedefi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelayTarget {
    /// 'twitch' | 'kick' | serbest etiket. 'youtube' KABUL EDİLMEZ — YouTube
    /// zaten OBS'ten doğrudan yayın alır; altyazı/multi-audio için relay
    /// değil `/api/integrations/youtube` OAuth akışı kullanılır (bkz. `set`).
    pub platform: String,
    pub rtmp_url: String,
    pub stream_key: String,
    /// Doluysa bu hedef bir dublaj hedefidir — SDK bu hedefe orijinal ses
    /// yerine bu dildeki klonlanmış sesi mux'lar (cihazda mux, bkz.
    /// `sdk/src/core/mux`). Boşsa (varsayılan) hedef değişmeden orijinal
    /// sesle yayınlanır — eski kayıtlar `#[serde(default)]` ile `None` okur.
    #[serde(default)]
    pub dub_language: Option<String>,
}

/// Hedef depolama hataları.
#[derive(Debug, thiserror::Error)]
pub enum RelayError {
    #[error("relay store io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("relay store crypto error: {0}")]
    Crypto(String),
    #[error("relay store serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
}

/// Kullanıcı → hedef listesi deposu (opsiyonel şifreli dosya kalıcılığı).
pub struct RelayTargetStore {
    entries: RwLock<HashMap<String, Vec<RelayTarget>>>,
    persist: Option<(PathBuf, Cipher)>,
}

impl RelayTargetStore {
    /// Kalıcılıksız depo (dev/test).
    pub fn in_memory() -> Self {
        RelayTargetStore {
            entries: RwLock::new(HashMap::new()),
            persist: None,
        }
    }

    /// Şifreli dosya kalıcılığıyla kurar; dosya varsa hedefleri yükler.
    pub fn with_file(path: impl Into<PathBuf>, key: &[u8]) -> Result<Self, RelayError> {
        let path = path.into();
        let cipher = Cipher::new(key).map_err(|e| RelayError::Crypto(e.to_string()))?;
        let entries = load_entries(&path, &cipher)?;
        Ok(RelayTargetStore {
            entries: RwLock::new(entries),
            persist: Some((path, cipher)),
        })
    }

    /// Hedef listesini doğrulayıp kullanıcı için (tamamen) değiştirir.
    pub fn set(&self, user_id: &str, targets: Vec<RelayTarget>) -> Result<(), RelayError> {
        if targets.len() > MAX_TARGETS {
            return Err(RelayError::Invalid(format!(
                "at most {MAX_TARGETS} relay targets allowed"
            )));
        }
        for target in &targets {
            if !(target.rtmp_url.starts_with("rtmp://") || target.rtmp_url.starts_with("rtmps://")) {
                return Err(RelayError::Invalid(format!(
                    "rtmp_url must start with rtmp:// or rtmps:// ({})",
                    target.platform
                )));
            }
            if target.stream_key.trim().is_empty() {
                return Err(RelayError::Invalid(format!(
                    "stream_key must not be empty ({})",
                    target.platform
                )));
            }
            if target.platform.trim().is_empty() {
                return Err(RelayError::Invalid("platform must not be empty".into()));
            }
            if target.platform.trim().eq_ignore_ascii_case("youtube") {
                return Err(RelayError::Invalid(
                    "youtube is not a relay target — YouTube already receives your stream \
                     directly from OBS; connect it from Platforms (OAuth) for native captions \
                     and multi-audio instead of routing video through the relay"
                        .into(),
                ));
            }
        }
        {
            let mut map = self
                .entries
                .write()
                .map_err(|_| RelayError::Crypto("relay lock poisoned".into()))?;
            map.insert(user_id.to_string(), targets);
        }
        self.save()
    }

    /// Kullanıcının hedefleri (yoksa boş liste).
    pub fn get(&self, user_id: &str) -> Vec<RelayTarget> {
        self.entries
            .read()
            .ok()
            .and_then(|m| m.get(user_id).cloned())
            .unwrap_or_default()
    }

    fn save(&self) -> Result<(), RelayError> {
        let Some((path, cipher)) = &self.persist else {
            return Ok(());
        };
        let snapshot: HashMap<String, Vec<RelayTarget>> = self
            .entries
            .read()
            .map_err(|_| RelayError::Crypto("relay lock poisoned".into()))?
            .clone();
        let json = serde_json::to_string(&snapshot)?;
        let sealed = cipher
            .encrypt_str(&json)
            .map_err(|e| RelayError::Crypto(e.to_string()))?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, sealed)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

fn load_entries(
    path: &Path,
    cipher: &Cipher,
) -> Result<HashMap<String, Vec<RelayTarget>>, RelayError> {
    let sealed = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(e) => return Err(RelayError::Io(e)),
    };
    let json = cipher
        .decrypt_str(sealed.trim())
        .map_err(|e| RelayError::Crypto(format!("relay file unreadable: {e}")))?;
    Ok(serde_json::from_str(&json)?)
}

/// `PUT /api/relay/targets` gövdesi.
#[derive(Debug, Deserialize)]
pub struct PutTargetsRequest {
    pub targets: Vec<RelayTarget>,
}

/// `PUT /api/relay/targets` (yayıncı) — hedef listesini kaydeder.
pub async fn put_targets(
    user: AuthUser,
    State(store): State<Arc<RelayTargetStore>>,
    State(logger): State<Arc<Logger>>,
    Json(req): Json<PutTargetsRequest>,
) -> Result<Json<Vec<RelayTarget>>, (StatusCode, String)> {
    let count = req.targets.len();
    store.set(&user.claims.sub, req.targets).map_err(|err| match err {
        RelayError::Invalid(_) => (StatusCode::UNPROCESSABLE_ENTITY, err.to_string()),
        other => (StatusCode::INTERNAL_SERVER_ERROR, other.to_string()),
    })?;
    // Değer loglanmaz — sadece adet (stream key asla log'a düşmez).
    logger.log(
        LogLevel::Info,
        "relay",
        &format!("relay targets updated ({count} targets)"),
        Some(&user.claims.sub),
        None,
    );
    Ok(Json(store.get(&user.claims.sub)))
}

/// `GET /api/relay/targets` — eşleşmiş SDK (pairing token'ı) veya panel
/// kendi hedeflerini çeker. TLS üstünden düz key döner (RTMP publish için
/// SDK'nın düz metne ihtiyacı var).
pub async fn get_targets(
    user: AuthUser,
    State(store): State<Arc<RelayTargetStore>>,
) -> Json<Vec<RelayTarget>> {
    Json(store.get(&user.claims.sub))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(platform: &str, key: &str) -> RelayTarget {
        RelayTarget {
            platform: platform.into(),
            rtmp_url: format!("rtmp://ingest.{platform}.example/live"),
            stream_key: key.into(),
            dub_language: None,
        }
    }

    #[test]
    fn set_and_get_roundtrip_per_user() {
        let store = RelayTargetStore::in_memory();
        store.set("u1", vec![target("twitch", "tw-key"), target("kick", "kick-key")]).unwrap();
        store.set("u2", vec![target("kick", "kick-key")]).unwrap();

        let u1 = store.get("u1");
        assert_eq!(u1.len(), 2);
        assert_eq!(u1[0].stream_key, "tw-key");
        assert_eq!(store.get("u2").len(), 1);
        assert!(store.get("u3").is_empty());

        // set tam liste değiştirir.
        store.set("u1", vec![target("twitch", "new-key")]).unwrap();
        let u1 = store.get("u1");
        assert_eq!(u1.len(), 1);
        assert_eq!(u1[0].stream_key, "new-key");
    }

    #[test]
    fn dub_language_roundtrips_and_defaults_to_none() {
        let store = RelayTargetStore::in_memory();
        let mut dub_target = target("kick", "kick-key");
        dub_target.dub_language = Some("en".into());
        store.set("u1", vec![target("youtube", "yt-key"), dub_target]).unwrap();

        let targets = store.get("u1");
        assert_eq!(targets[0].dub_language, None);
        assert_eq!(targets[1].dub_language.as_deref(), Some("en"));
    }

    #[test]
    fn dub_language_missing_in_json_defaults_to_none() {
        // Eski kayıt/istemci — dub_language alanı hiç yok.
        let json = r#"{"platform":"youtube","rtmp_url":"rtmp://ingest.youtube.example/live","stream_key":"k"}"#;
        let parsed: RelayTarget = serde_json::from_str(json).expect("parse");
        assert_eq!(parsed.dub_language, None);
    }

    #[test]
    fn validation_rejects_bad_targets() {
        let store = RelayTargetStore::in_memory();
        // http URL reddedilir.
        let mut bad = target("twitch", "k");
        bad.rtmp_url = "http://not-rtmp.example".into();
        assert!(matches!(store.set("u1", vec![bad]), Err(RelayError::Invalid(_))));
        // Boş stream key reddedilir.
        let mut empty = target("twitch", "");
        empty.stream_key = "  ".into();
        assert!(matches!(store.set("u1", vec![empty]), Err(RelayError::Invalid(_))));
        // Hedef sınırı.
        let many = (0..9).map(|i| target("t", &format!("k{i}"))).collect();
        assert!(matches!(store.set("u1", many), Err(RelayError::Invalid(_))));
        // rtmps kabul edilir.
        let mut ok = target("twitch", "k");
        ok.rtmp_url = "rtmps://ingest.example/live".into();
        assert!(store.set("u1", vec![ok]).is_ok());
    }

    #[test]
    fn validation_rejects_youtube_as_relay_target() {
        let store = RelayTargetStore::in_memory();
        // YouTube zaten OBS'ten doğrudan yayın alır — relay hedefi olamaz.
        let err = store.set("u1", vec![target("youtube", "k")]).unwrap_err();
        assert!(matches!(err, RelayError::Invalid(_)));
        // Case-insensitive.
        let err = store.set("u1", vec![target("YouTube", "k")]).unwrap_err();
        assert!(matches!(err, RelayError::Invalid(_)));
    }

    #[test]
    fn persists_encrypted_and_reloads_without_plaintext_keys() {
        let path = std::env::temp_dir().join(format!("sd-relay-{}.enc", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let key = crate::crypto::derive_key(b"jwt", "streamdubber.relay.v1");

        {
            let store = RelayTargetStore::with_file(&path, &key).unwrap();
            store.set("u1", vec![target("twitch", "super-secret-stream-key")]).unwrap();
        }

        // Diskte düz metin stream key / URL yok.
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("super-secret-stream-key"));
        assert!(!raw.contains("rtmp://"));

        let reloaded = RelayTargetStore::with_file(&path, &key).unwrap();
        assert_eq!(reloaded.get("u1")[0].stream_key, "super-secret-stream-key");

        let wrong = crate::crypto::derive_key(b"other", "streamdubber.relay.v1");
        assert!(RelayTargetStore::with_file(&path, &wrong).is_err());

        let _ = std::fs::remove_file(&path);
    }
}
