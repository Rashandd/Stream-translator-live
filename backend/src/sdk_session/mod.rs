//! Canlı SDK cihaz varlığı (presence) — imzalı heartbeat + panelin
//! "bağlı cihazlar" listesi.
//!
//! `pairing::claim` başarılı olduğunda burada bir [`SdkSessionRecord`]
//! oluşturulur ve rastgele 32 byte'lık bir `heartbeat_secret` üretilir; bu
//! secret SDK'ya **yalnızca claim yanıtında bir kez** düz metin döner (TLS
//! üstünden), sonra bir daha gösterilmez — SDK bunu kendi
//! `LocalTokenStore`'unda (AES-256-GCM) saklar.
//!
//! SDK canlı kaldığı sürece periyodik olarak `POST /api/sdk/heartbeat`
//! gönderir: `{ session_id, ts, sig }` — `sig`,
//! `HMAC-SHA256(heartbeat_secret, "{session_id}.{ts}")`'nin hex'idir. Bu,
//! Stripe webhook doğrulamasıyla (`billing::stripe`) birebir aynı
//! zaman-toleranslı HMAC şeması: `ts` şimdiki zamandan
//! [`HEARTBEAT_TOLERANCE_SECS`]'den fazla sapmışsa reddedilir — biri eski
//! bir heartbeat isteğini yakalayıp tekrar oynatarak (replay) cihazı
//! olduğundan uzun süre "bağlı" gösteremez, ve secret'ı bilmeyen biri sahte
//! bir "bağlı" sinyali üretemez.
//!
//! Panel, `GET /api/sdk/devices` ile kendi hesabına eşleşmiş TÜM cihazları
//! (`connected` bayrağıyla — son heartbeat [`CONNECTED_WINDOW_SECS`]
//! içindeyse) listeler ve `DELETE /api/sdk/devices/{id}` ile bir cihazı
//! listeden kaldırabilir (kayıp/çalıntı cihaz senaryosu). Kayıt silinince o
//! `session_id` için heartbeat artık kabul edilmez — ama NOT: bu, cihazın
//! 30 günlük device JWT'sini iptal etmez (relay hedefleri gibi diğer
//! SDK-authenticated uçlar JWT `exp`'e kadar çalışmaya devam eder); tam JWT
//! iptali ayrı bir iş (bkz. roadmap notu). Bu yüzden panelden "kaldır" asıl
//! olarak canlı durumu/listeyi temizler — fiziksel cihazın kendisini
//! susturmak için SDK'nın kendi `unpair` komutu (yerel token silme) gerekir.
//!
//! PostgreSQL yapılandırıldığında cihazlar ve heartbeat anahtarları DB'de
//! authoritative olarak saklanır. `SDK_SESSIONS_FILE` yalnız DB'siz
//! development/test adapter'ıdır.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use axum::Json;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::auth::authz::AuthUser;
use crate::crypto::Cipher;
use crate::logging::{LogLevel, Logger};

type HmacSha256 = Hmac<Sha256>;

/// Heartbeat imzasının kabul edildiği zaman penceresi (saniye) — saat
/// sapması toleransı + replay koruması (bkz. modül dokümanı).
const HEARTBEAT_TOLERANCE_SECS: i64 = 60;
/// Son heartbeat bu süre içindeyse cihaz "bağlı" sayılır. SDK ~20sn'de bir
/// heartbeat attığı için birkaç kaçırılan sinyele tolerans tanır.
const CONNECTED_WINDOW_SECS: i64 = 45;

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_secs() as i64
}

/// Bir eşleşmiş cihazın sunucu tarafı kaydı.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkSessionRecord {
    pub id: String,
    pub user_id: String,
    pub device_name: String,
    pub sdk_version: String,
    pub paired_at: i64,
    pub last_seen_at: i64,
    /// Hex-encoded 32 byte. Yalnızca sunucuda tutulur; SDK'ya bir daha
    /// dönülmez (claim yanıtı dışında).
    heartbeat_secret_hex: String,
}

/// `sdk_session` hataları.
#[derive(Debug, thiserror::Error)]
pub enum SdkSessionError {
    #[error("sdk session store io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("sdk session store crypto error: {0}")]
    Crypto(String),
    #[error("sdk session store serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("rng failure: {0}")]
    Rng(#[from] getrandom::Error),
    #[error("unknown or unowned session")]
    NotFound,
    #[error("heartbeat timestamp out of tolerance")]
    StaleTimestamp,
    #[error("heartbeat signature mismatch")]
    BadSignature,
}

fn status_for(err: &SdkSessionError) -> StatusCode {
    match err {
        SdkSessionError::NotFound => StatusCode::NOT_FOUND,
        SdkSessionError::StaleTimestamp | SdkSessionError::BadSignature => StatusCode::UNAUTHORIZED,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Kullanıcı → eşleşmiş cihazlar deposu (opsiyonel şifreli dosya kalıcılığı).
pub struct SdkSessionStore {
    entries: RwLock<HashMap<String, SdkSessionRecord>>,
    persist: Option<(PathBuf, Cipher)>,
    db: Option<(sqlx::PgPool, Cipher)>,
}

impl SdkSessionStore {
    /// Kalıcılıksız depo (dev/test).
    pub fn in_memory() -> Self {
        SdkSessionStore {
            entries: RwLock::new(HashMap::new()),
            persist: None,
            db: None,
        }
    }

    /// Şifreli dosya kalıcılığıyla kurar; dosya varsa kayıtları yükler.
    pub fn with_file(path: impl Into<PathBuf>, key: &[u8]) -> Result<Self, SdkSessionError> {
        let path = path.into();
        let cipher = Cipher::new(key).map_err(|e| SdkSessionError::Crypto(e.to_string()))?;
        let entries = load_entries(&path, &cipher)?;
        Ok(SdkSessionStore {
            entries: RwLock::new(entries),
            persist: Some((path, cipher)),
            db: None,
        })
    }

    /// PostgreSQL repository'sini bağlar. Key, DB'deki heartbeat secret için
    /// AES-256-GCM anahtarıdır.
    pub fn with_db(mut self, pool: sqlx::PgPool, key: &[u8]) -> Result<Self, SdkSessionError> {
        let cipher = Cipher::new(key).map_err(|e| SdkSessionError::Crypto(e.to_string()))?;
        self.db = Some((pool, cipher));
        Ok(self)
    }

    /// Yeni bir cihaz kaydı oluşturur (pairing claim sırasında çağrılır).
    /// `(session_id, heartbeat_secret_hex)` döner — secret burada bir kez
    /// düz metin görünür, çağıran bunu `ClaimResponse`'a koyar.
    pub fn create(
        &self,
        user_id: &str,
        device_name: &str,
        sdk_version: &str,
    ) -> Result<(String, String), SdkSessionError> {
        let mut secret_bytes = [0u8; 32];
        getrandom::getrandom(&mut secret_bytes)?;
        let secret_hex = hex::encode(secret_bytes);
        let id = uuid::Uuid::new_v4().to_string();
        let now = now_ts();
        let record = SdkSessionRecord {
            id: id.clone(),
            user_id: user_id.to_string(),
            device_name: device_name.to_string(),
            sdk_version: sdk_version.to_string(),
            paired_at: now,
            last_seen_at: now,
            heartbeat_secret_hex: secret_hex.clone(),
        };
        {
            let mut map = self
                .entries
                .write()
                .map_err(|_| SdkSessionError::Crypto("sdk session lock poisoned".into()))?;
            map.insert(id.clone(), record);
        }
        self.save()?;
        Ok((id, secret_hex))
    }

    /// Kullanıcının eşleşmiş cihazları (bağlı/bağlı değil bilgisiyle).
    pub fn list(&self, user_id: &str) -> Vec<SdkSessionRecord> {
        self.entries
            .read()
            .ok()
            .map(|m| {
                m.values()
                    .filter(|r| r.user_id == user_id)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// İmzalı heartbeat'i doğrular ve `last_seen_at`'i günceller. `ts`
    /// toleransı ve HMAC uyuşmuyorsa reddeder; `session_id` bu kullanıcıya
    /// ait değilse (veya yoksa) `NotFound` (varlığını sızdırmaz).
    pub fn heartbeat(
        &self,
        user_id: &str,
        session_id: &str,
        ts: i64,
        sig_hex: &str,
    ) -> Result<(), SdkSessionError> {
        if (now_ts() - ts).abs() > HEARTBEAT_TOLERANCE_SECS {
            return Err(SdkSessionError::StaleTimestamp);
        }
        let mut map = self
            .entries
            .write()
            .map_err(|_| SdkSessionError::Crypto("sdk session lock poisoned".into()))?;
        let record = map
            .get_mut(session_id)
            .filter(|r| r.user_id == user_id)
            .ok_or(SdkSessionError::NotFound)?;
        if !verify_heartbeat_sig(&record.heartbeat_secret_hex, session_id, ts, sig_hex) {
            return Err(SdkSessionError::BadSignature);
        }
        record.last_seen_at = now_ts();
        Ok(())
    }

    /// Bir cihazı listeden kaldırır ("bu cihazı unpair et"). Kendi
    /// hesabına ait değilse veya yoksa `false`.
    pub fn remove(&self, user_id: &str, session_id: &str) -> Result<bool, SdkSessionError> {
        let removed = {
            let mut map = self
                .entries
                .write()
                .map_err(|_| SdkSessionError::Crypto("sdk session lock poisoned".into()))?;
            match map.get(session_id) {
                Some(r) if r.user_id == user_id => {
                    map.remove(session_id);
                    true
                }
                _ => false,
            }
        };
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    /// DB-authoritative create. The secret is encrypted before it crosses the
    /// SQL boundary and is only returned to the pairing response once.
    pub async fn persistent_create(
        &self,
        user_id: &str,
        device_name: &str,
        sdk_version: &str,
    ) -> Result<(String, String), SdkSessionError> {
        let Some((pool, cipher)) = &self.db else {
            return self.create(user_id, device_name, sdk_version);
        };
        let user_uuid = uuid::Uuid::parse_str(user_id).map_err(|_| SdkSessionError::NotFound)?;
        let mut secret_bytes = [0u8; 32];
        getrandom::getrandom(&mut secret_bytes)?;
        let secret_hex = hex::encode(secret_bytes);
        let id = uuid::Uuid::new_v4().to_string();
        let sealed = cipher
            .encrypt_str(&secret_hex)
            .map_err(|e| SdkSessionError::Crypto(e.to_string()))?;
        sqlx::query(
            "INSERT INTO sdk_sessions (id, user_id, device_name, sdk_version, paired_at, last_seen_at, heartbeat_secret_encrypted, is_active) \
             VALUES ($1::uuid, $2, $3, $4, NOW(), NOW(), $5, TRUE)",
        )
        .bind(&id)
        .bind(user_uuid)
        .bind(device_name)
        .bind(sdk_version)
        .bind(sealed)
        .execute(pool)
        .await
        .map_err(|_| SdkSessionError::Crypto("sdk session database write failed".into()))?;
        Ok((id, secret_hex))
    }

    pub async fn persistent_list(
        &self,
        user_id: &str,
    ) -> Result<Vec<SdkSessionRecord>, SdkSessionError> {
        let Some((pool, cipher)) = &self.db else {
            return Ok(self.list(user_id));
        };
        let user_uuid = uuid::Uuid::parse_str(user_id).map_err(|_| SdkSessionError::NotFound)?;
        let rows = sqlx::query(
            "SELECT id::text, user_id::text, COALESCE(device_name, '') AS device_name, COALESCE(sdk_version, '') AS sdk_version, \
                    EXTRACT(EPOCH FROM paired_at)::BIGINT AS paired_at, \
                    EXTRACT(EPOCH FROM last_seen_at)::BIGINT AS last_seen_at, heartbeat_secret_encrypted \
             FROM sdk_sessions WHERE user_id = $1 AND is_active ORDER BY paired_at DESC",
        )
        .bind(user_uuid)
        .fetch_all(pool)
        .await
        .map_err(|_| SdkSessionError::Crypto("sdk session database read failed".into()))?;
        rows.into_iter()
            .map(|row| {
                use sqlx::Row;
                let sealed: Option<String> = row
                    .try_get("heartbeat_secret_encrypted")
                    .map_err(|_| SdkSessionError::NotFound)?;
                let secret = sealed.ok_or(SdkSessionError::NotFound).and_then(|value| {
                    cipher
                        .decrypt_str(&value)
                        .map_err(|e| SdkSessionError::Crypto(e.to_string()))
                })?;
                Ok(SdkSessionRecord {
                    id: row.try_get("id").map_err(|_| SdkSessionError::NotFound)?,
                    user_id: row
                        .try_get("user_id")
                        .map_err(|_| SdkSessionError::NotFound)?,
                    device_name: row
                        .try_get("device_name")
                        .map_err(|_| SdkSessionError::NotFound)?,
                    sdk_version: row
                        .try_get("sdk_version")
                        .map_err(|_| SdkSessionError::NotFound)?,
                    paired_at: row
                        .try_get::<Option<i64>, _>("paired_at")
                        .ok()
                        .flatten()
                        .unwrap_or(0),
                    last_seen_at: row
                        .try_get::<Option<i64>, _>("last_seen_at")
                        .ok()
                        .flatten()
                        .unwrap_or(0),
                    heartbeat_secret_hex: secret,
                })
            })
            .collect()
    }

    pub async fn persistent_heartbeat(
        &self,
        user_id: &str,
        session_id: &str,
        ts: i64,
        sig_hex: &str,
    ) -> Result<(), SdkSessionError> {
        if self.db.is_none() {
            return self.heartbeat(user_id, session_id, ts, sig_hex);
        }
        if (now_ts() - ts).abs() > HEARTBEAT_TOLERANCE_SECS {
            return Err(SdkSessionError::StaleTimestamp);
        }
        let Some((pool, cipher)) = &self.db else {
            return Err(SdkSessionError::NotFound);
        };
        let user_uuid = uuid::Uuid::parse_str(user_id).map_err(|_| SdkSessionError::NotFound)?;
        let secret: Option<String> = sqlx::query_scalar(
            "SELECT heartbeat_secret_encrypted FROM sdk_sessions WHERE id = $1::uuid AND user_id = $2 AND is_active",
        )
        .bind(session_id)
        .bind(user_uuid)
        .fetch_optional(pool)
        .await
        .map_err(|_| SdkSessionError::Crypto("sdk session database read failed".into()))?;
        let secret = secret.ok_or(SdkSessionError::NotFound).and_then(|value| {
            cipher
                .decrypt_str(&value)
                .map_err(|e| SdkSessionError::Crypto(e.to_string()))
        })?;
        if !verify_heartbeat_sig(&secret, session_id, ts, sig_hex) {
            return Err(SdkSessionError::BadSignature);
        }
        sqlx::query(
            "UPDATE sdk_sessions SET last_seen_at = NOW() WHERE id = $1::uuid AND user_id = $2",
        )
        .bind(session_id)
        .bind(user_uuid)
        .execute(pool)
        .await
        .map_err(|_| SdkSessionError::Crypto("sdk session database write failed".into()))?;
        Ok(())
    }

    pub async fn persistent_remove(
        &self,
        user_id: &str,
        session_id: &str,
    ) -> Result<bool, SdkSessionError> {
        let Some((pool, _)) = &self.db else {
            return self.remove(user_id, session_id);
        };
        let user_uuid = uuid::Uuid::parse_str(user_id).map_err(|_| SdkSessionError::NotFound)?;
        let result = sqlx::query("UPDATE sdk_sessions SET is_active = FALSE WHERE id = $1::uuid AND user_id = $2 AND is_active")
            .bind(session_id)
            .bind(user_uuid)
            .execute(pool)
            .await
            .map_err(|_| SdkSessionError::Crypto("sdk session database write failed".into()))?;
        Ok(result.rows_affected() == 1)
    }

    fn save(&self) -> Result<(), SdkSessionError> {
        let Some((path, cipher)) = &self.persist else {
            return Ok(());
        };
        let snapshot: HashMap<String, SdkSessionRecord> = self
            .entries
            .read()
            .map_err(|_| SdkSessionError::Crypto("sdk session lock poisoned".into()))?
            .clone();
        let json = serde_json::to_string(&snapshot)?;
        let sealed = cipher
            .encrypt_str(&json)
            .map_err(|e| SdkSessionError::Crypto(e.to_string()))?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, sealed)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

fn load_entries(
    path: &Path,
    cipher: &Cipher,
) -> Result<HashMap<String, SdkSessionRecord>, SdkSessionError> {
    let sealed = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(e) => return Err(SdkSessionError::Io(e)),
    };
    let json = cipher
        .decrypt_str(sealed.trim())
        .map_err(|e| SdkSessionError::Crypto(format!("sdk session file unreadable: {e}")))?;
    Ok(serde_json::from_str(&json)?)
}

/// `HMAC-SHA256(secret, "{session_id}.{ts}")`'nin hex'i (bkz.
/// `billing::stripe::compute_signature` — aynı zaman-toleranslı HMAC şeması).
fn compute_heartbeat_sig(secret_hex: &str, session_id: &str, ts: i64) -> Option<String> {
    let key = hex::decode(secret_hex).ok()?;
    let mut mac = HmacSha256::new_from_slice(&key).ok()?;
    mac.update(session_id.as_bytes());
    mac.update(b".");
    mac.update(ts.to_string().as_bytes());
    Some(hex::encode(mac.finalize().into_bytes()))
}

fn verify_heartbeat_sig(secret_hex: &str, session_id: &str, ts: i64, sig_hex: &str) -> bool {
    match compute_heartbeat_sig(secret_hex, session_id, ts) {
        Some(expected) => constant_time_eq(&expected, sig_hex),
        None => false,
    }
}

/// Zamanlama saldırılarına karşı sabit zamanlı karşılaştırma (bkz.
/// `billing::stripe::constant_time_eq` — aynı desen, ayrı instance).
fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Panelin gördüğü cihaz görünümü — `heartbeat_secret` asla dışarı sızmaz.
#[derive(Debug, Serialize)]
pub struct DeviceDto {
    pub id: String,
    pub device_name: String,
    pub sdk_version: String,
    pub paired_at: i64,
    pub last_seen_at: i64,
    pub connected: bool,
}

impl From<SdkSessionRecord> for DeviceDto {
    fn from(r: SdkSessionRecord) -> Self {
        let connected = (now_ts() - r.last_seen_at) <= CONNECTED_WINDOW_SECS;
        DeviceDto {
            id: r.id,
            device_name: r.device_name,
            sdk_version: r.sdk_version,
            paired_at: r.paired_at,
            last_seen_at: r.last_seen_at,
            connected,
        }
    }
}

/// `GET /api/sdk/devices` (panel, auth) — kullanıcının eşleşmiş TÜM
/// cihazları, canlı bağlantı durumuyla.
pub async fn list_devices(
    user: AuthUser,
    State(store): State<Arc<SdkSessionStore>>,
) -> Result<Json<Vec<DeviceDto>>, (StatusCode, String)> {
    let mut devices: Vec<DeviceDto> = store
        .persistent_list(&user.claims.sub)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .into_iter()
        .map(DeviceDto::from)
        .collect();
    devices.sort_by_key(|d| std::cmp::Reverse(d.paired_at));
    Ok(Json(devices))
}

/// `DELETE /api/sdk/devices/{id}` (panel, auth) — cihazı listeden kaldırır.
pub async fn remove_device(
    user: AuthUser,
    State(store): State<Arc<SdkSessionStore>>,
    State(logger): State<Arc<Logger>>,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let removed = store
        .persistent_remove(&user.claims.sub, &id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !removed {
        return Err((StatusCode::NOT_FOUND, "device not found".to_string()));
    }
    logger.log(
        LogLevel::Info,
        "sdk_session",
        "device removed by owner",
        Some(&user.claims.sub),
        None,
    );
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /api/sdk/heartbeat` gövdesi.
#[derive(Debug, Deserialize)]
pub struct HeartbeatRequest {
    pub session_id: String,
    pub ts: i64,
    pub sig: String,
}

/// `POST /api/sdk/heartbeat` (SDK, device JWT auth) — canlılık sinyali.
pub async fn heartbeat(
    user: AuthUser,
    State(store): State<Arc<SdkSessionStore>>,
    Json(req): Json<HeartbeatRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    store
        .persistent_heartbeat(&user.claims.sub, &req.session_id, req.ts, &req.sig)
        .await
        .map_err(|e| (status_for(&e), e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_then_heartbeat_updates_last_seen() {
        let store = SdkSessionStore::in_memory();
        let (id, secret) = store.create("u1", "Windows PC", "0.1.0").unwrap();

        let ts = now_ts();
        let sig = compute_heartbeat_sig(&secret, &id, ts).unwrap();
        store.heartbeat("u1", &id, ts, &sig).unwrap();

        let devices = store.list("u1");
        assert_eq!(devices.len(), 1);
        assert!(DeviceDto::from(devices[0].clone()).connected);
    }

    #[test]
    fn heartbeat_rejects_stale_timestamp() {
        let store = SdkSessionStore::in_memory();
        let (id, secret) = store.create("u1", "PC", "0.1.0").unwrap();

        let stale_ts = now_ts() - (HEARTBEAT_TOLERANCE_SECS + 30);
        let sig = compute_heartbeat_sig(&secret, &id, stale_ts).unwrap();
        assert!(matches!(
            store.heartbeat("u1", &id, stale_ts, &sig),
            Err(SdkSessionError::StaleTimestamp)
        ));
    }

    #[test]
    fn heartbeat_rejects_bad_signature() {
        let store = SdkSessionStore::in_memory();
        let (id, _secret) = store.create("u1", "PC", "0.1.0").unwrap();

        let ts = now_ts();
        assert!(matches!(
            store.heartbeat("u1", &id, ts, "not-the-right-signature"),
            Err(SdkSessionError::BadSignature)
        ));
    }

    #[test]
    fn heartbeat_rejects_wrong_owner() {
        let store = SdkSessionStore::in_memory();
        let (id, secret) = store.create("u1", "PC", "0.1.0").unwrap();

        let ts = now_ts();
        let sig = compute_heartbeat_sig(&secret, &id, ts).unwrap();
        // u2, u1'in session_id'sini bilse bile (ör. tahmin/sızıntı) kendi
        // secret'ı olmadan geçerli imza üretemez; ayrıca sahiplik de reddeder.
        assert!(matches!(
            store.heartbeat("u2", &id, ts, &sig),
            Err(SdkSessionError::NotFound)
        ));
    }

    #[test]
    fn heartbeat_rejects_unknown_session() {
        let store = SdkSessionStore::in_memory();
        assert!(matches!(
            store.heartbeat("u1", "no-such-session", now_ts(), "x"),
            Err(SdkSessionError::NotFound)
        ));
    }

    #[test]
    fn list_only_returns_own_devices() {
        let store = SdkSessionStore::in_memory();
        store.create("u1", "PC A", "0.1.0").unwrap();
        store.create("u2", "PC B", "0.1.0").unwrap();

        assert_eq!(store.list("u1").len(), 1);
        assert_eq!(store.list("u1")[0].device_name, "PC A");
        assert_eq!(store.list("u2").len(), 1);
    }

    #[test]
    fn remove_only_own_device() {
        let store = SdkSessionStore::in_memory();
        let (id, _) = store.create("u1", "PC", "0.1.0").unwrap();

        // Başka kullanıcı kaldıramaz.
        assert!(!store.remove("u2", &id).unwrap());
        assert_eq!(store.list("u1").len(), 1);

        assert!(store.remove("u1", &id).unwrap());
        assert_eq!(store.list("u1").len(), 0);

        // Kaldırıldıktan sonra heartbeat artık kabul edilmez.
        assert!(matches!(
            store.heartbeat("u1", &id, now_ts(), "x"),
            Err(SdkSessionError::NotFound)
        ));
    }

    #[test]
    fn disconnected_after_connected_window() {
        let store = SdkSessionStore::in_memory();
        let (id, _secret) = store.create("u1", "PC", "0.1.0").unwrap();
        // last_seen_at'i pencere dışına manuel it (gerçek zamanda 45sn
        // beklemeden test etmek için doğrudan store'a eriş).
        {
            let mut map = store.entries.write().unwrap();
            map.get_mut(&id).unwrap().last_seen_at = now_ts() - (CONNECTED_WINDOW_SECS + 5);
        }
        let devices = store.list("u1");
        assert!(!DeviceDto::from(devices[0].clone()).connected);
    }

    #[test]
    fn persists_across_reopen_but_not_last_seen_precision() {
        let dir = std::env::temp_dir().join(format!(
            "streamdubber-sdk-session-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sessions.enc");
        let key = [7u8; 32];

        let (id, secret) = {
            let store = SdkSessionStore::with_file(&path, &key).unwrap();
            store.create("u1", "PC", "0.1.0").unwrap()
        };

        let reopened = SdkSessionStore::with_file(&path, &key).unwrap();
        let devices = reopened.list("u1");
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].id, id);

        // Secret de kalıcı — reopen sonrası heartbeat hâlâ doğrulanabilir.
        let ts = now_ts();
        let sig = compute_heartbeat_sig(&secret, &id, ts).unwrap();
        assert!(reopened.heartbeat("u1", &id, ts, &sig).is_ok());

        std::fs::remove_dir_all(&dir).ok();
    }
}
