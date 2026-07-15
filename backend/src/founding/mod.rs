//! "Founding 5" kapalı beta başvuruları (Faz 1 — ilk STT-only test).
//!
//! Stripe olmadan, tek GPU STT worker'ıyla ilk 5 yayıncıyı test etmek için:
//! yayıncı hesap açtıktan sonra platform + kanal linkiyle başvurur, admin
//! panelden başvuruları görür ve seçtiği hesapları onaylar.
//!
//! Onay durumu bilinçli olarak `UserRecord.plan`'dan (ve dolayısıyla JWT
//! `claims.plan`'dan) AYRI tutulur: `ws::mod` StreamStart'ta planı imzalı
//! JWT claim'inden okur, bu da 24 saat stale kalabilir — admin onayladığı
//! anda kullanıcının mevcut oturumunun bunu görmesi gerekir. Bu store'daki
//! `is_approved` her StreamStart'ta CANLI okunur (bkz. `ws::mod::effective_plan`),
//! bu yüzden onay/iptal re-login beklemeden anında etkili olur.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::auth::authz::{AdminUser, AuthUser};
use crate::config::ServiceConfig;
use crate::users::UserStore;

const MAX_CHANNEL_URL_LEN: usize = 512;
const MAX_MESSAGE_LEN: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoundingPlatform {
    Kick,
    Youtube,
    Twitch,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoundingStatus {
    Pending,
    Approved,
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoundingApplication {
    pub user_id: String,
    pub email: String,
    pub platform: FoundingPlatform,
    pub channel_url: String,
    #[serde(default)]
    pub message: String,
    pub status: FoundingStatus,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    /// Başvuru, kullanıcının manuel formu doldurmasıyla değil gerçek bir
    /// OAuth platform bağlantısıyla otomatik mi oluşturuldu (bkz.
    /// `auto_apply_if_absent`, `integrations::callback`). Eski kayıtlarda
    /// (bu alan eklenmeden önce) `false` varsayılır.
    #[serde(default)]
    pub auto_created_via_oauth: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum FoundingError {
    #[error("store file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("store file parse error: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("founding applications database error")]
    Database,
}

/// user_id → başvuru deposu (opsiyonel düz-JSON kalıcılık — kanal linki
/// secret değil; `voice_models::VoiceModelStore` ile aynı desen).
pub struct FoundingStore {
    inner: Mutex<HashMap<String, FoundingApplication>>,
    path: Option<PathBuf>,
    db: Option<sqlx::PgPool>,
}

impl FoundingStore {
    pub fn in_memory() -> Self {
        FoundingStore {
            inner: Mutex::new(HashMap::new()),
            path: None,
            db: None,
        }
    }

    pub fn with_file(path: impl AsRef<std::path::Path>) -> Result<Self, FoundingError> {
        let path = path.as_ref().to_path_buf();
        let entries: HashMap<String, FoundingApplication> = match std::fs::read_to_string(&path) {
            Ok(content) if !content.trim().is_empty() => serde_json::from_str(&content)?,
            Ok(_) => HashMap::new(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
            Err(e) => return Err(FoundingError::Io(e)),
        };
        Ok(FoundingStore {
            inner: Mutex::new(entries),
            path: Some(path),
            db: None,
        })
    }

    pub fn with_db(mut self, pool: sqlx::PgPool) -> Self {
        self.db = Some(pool);
        self
    }

    pub fn get(&self, user_id: &str) -> Option<FoundingApplication> {
        self.inner
            .lock()
            .expect("founding store lock poisoned")
            .get(user_id)
            .cloned()
    }

    /// Yeni başvuru oluşturur veya var olanı günceller. Zaten `Approved`/
    /// `Revoked` olan bir başvuruyu tekrar göndermek alanları günceller ama
    /// admin kararını geri almaz (durum korunur) — kullanıcı yanlışlıkla
    /// tekrar gönderse bile onay/iptal kararı bozulmaz. Bu, kullanıcının
    /// formu KENDİSİ doldurduğu yol — `auto_created_via_oauth` her zaman
    /// `false`'a döner (bkz. `auto_apply_if_absent` için OAuth yolu).
    pub fn upsert_application(
        &self,
        user_id: &str,
        email: &str,
        platform: FoundingPlatform,
        channel_url: &str,
        message: &str,
    ) -> FoundingApplication {
        let now = chrono::Utc::now().timestamp_millis();
        let mut map = self.inner.lock().expect("founding store lock poisoned");
        let application = match map.get(user_id) {
            Some(existing) => FoundingApplication {
                user_id: user_id.to_string(),
                email: email.to_string(),
                platform,
                channel_url: channel_url.to_string(),
                message: message.to_string(),
                status: existing.status,
                created_at_ms: existing.created_at_ms,
                updated_at_ms: now,
                auto_created_via_oauth: false,
            },
            None => FoundingApplication {
                user_id: user_id.to_string(),
                email: email.to_string(),
                platform,
                channel_url: channel_url.to_string(),
                message: message.to_string(),
                status: FoundingStatus::Pending,
                created_at_ms: now,
                updated_at_ms: now,
                auto_created_via_oauth: false,
            },
        };
        map.insert(user_id.to_string(), application.clone());
        drop(map);
        self.save();
        application
    }

    /// Bir platform hesabı gerçekten OAuth ile bağlanınca çağrılır
    /// (`integrations::callback`). Kullanıcının **hiç** başvurusu yoksa yeni
    /// bir `Pending` başvuru oluşturur ve döner; zaten bir başvuru varsa
    /// (manuel doldurulmuş ya da başka bir platformdan otomatik oluşmuş)
    /// dokunmadan `None` döner — ne kullanıcının elle girdiği bilgiyi ne de
    /// admin'in onay/iptal kararını asla ezmez.
    pub fn auto_apply_if_absent(
        &self,
        user_id: &str,
        email: &str,
        platform: FoundingPlatform,
        channel_url: &str,
    ) -> Option<FoundingApplication> {
        let now = chrono::Utc::now().timestamp_millis();
        let mut map = self.inner.lock().expect("founding store lock poisoned");
        if map.contains_key(user_id) {
            return None;
        }
        let application = FoundingApplication {
            user_id: user_id.to_string(),
            email: email.to_string(),
            platform,
            channel_url: channel_url.to_string(),
            message: String::new(),
            status: FoundingStatus::Pending,
            created_at_ms: now,
            updated_at_ms: now,
            auto_created_via_oauth: true,
        };
        map.insert(user_id.to_string(), application.clone());
        drop(map);
        self.save();
        Some(application)
    }

    /// Admin onay/iptal. Başvuru yoksa `None`.
    pub fn set_status(&self, user_id: &str, status: FoundingStatus) -> Option<FoundingApplication> {
        let mut map = self.inner.lock().expect("founding store lock poisoned");
        let updated = map.get_mut(user_id).map(|app| {
            app.status = status;
            app.updated_at_ms = chrono::Utc::now().timestamp_millis();
            app.clone()
        });
        drop(map);
        if updated.is_some() {
            self.save();
        }
        updated
    }

    /// `ws::mod::effective_plan` bunu her StreamStart'ta canlı çağırır —
    /// admin onayı/iptali anında yansır, JWT'nin yenilenmesini beklemez.
    pub fn is_approved(&self, user_id: &str) -> bool {
        self.inner
            .lock()
            .expect("founding store lock poisoned")
            .get(user_id)
            .is_some_and(|app| app.status == FoundingStatus::Approved)
    }

    /// Tüm başvurular, en yeni önce (admin inceleme kuyruğu).
    pub fn list_all(&self) -> Vec<FoundingApplication> {
        let mut apps: Vec<FoundingApplication> = self
            .inner
            .lock()
            .expect("founding store lock poisoned")
            .values()
            .cloned()
            .collect();
        apps.sort_by_key(|a| std::cmp::Reverse(a.created_at_ms));
        apps
    }

    fn save(&self) {
        let Some(path) = &self.path else { return };
        let snapshot = self
            .inner
            .lock()
            .expect("founding store lock poisoned")
            .clone();
        match serde_json::to_string_pretty(&snapshot) {
            Ok(json) => {
                if let Err(err) = std::fs::write(path, json) {
                    tracing::warn!(error = %err, "founding store persist failed");
                }
            }
            Err(err) => tracing::warn!(error = %err, "founding store serialize failed"),
        }
    }

    pub async fn persistent_get(
        &self,
        user_id: &str,
    ) -> Result<Option<FoundingApplication>, FoundingError> {
        let Some(pool) = &self.db else {
            return Ok(self.get(user_id));
        };
        let user_id = uuid::Uuid::parse_str(user_id).map_err(|_| FoundingError::Database)?;
        let row: Option<(String, String, String, String, String, String, i64, i64, bool)> = sqlx::query_as("SELECT user_id::text, email, platform, channel_url, message, status, EXTRACT(EPOCH FROM created_at)*1000::BIGINT, EXTRACT(EPOCH FROM updated_at)*1000::BIGINT, auto_created_via_oauth FROM founding_applications WHERE user_id = $1")
            .bind(user_id).fetch_optional(pool).await.map_err(|_| FoundingError::Database)?;
        row.map(application_from_row).transpose()
    }

    pub async fn persistent_upsert(
        &self,
        app: &FoundingApplication,
    ) -> Result<FoundingApplication, FoundingError> {
        let Some(pool) = &self.db else {
            return Ok(self.upsert_application(
                &app.user_id,
                &app.email,
                app.platform,
                &app.channel_url,
                &app.message,
            ));
        };
        let user_id = uuid::Uuid::parse_str(&app.user_id).map_err(|_| FoundingError::Database)?;
        let platform = enum_value(app.platform)?;
        sqlx::query("INSERT INTO founding_applications (user_id, email, platform, channel_url, message, status, auto_created_via_oauth) VALUES ($1, $2, $3, $4, $5, 'pending', FALSE) ON CONFLICT (user_id) DO UPDATE SET email = EXCLUDED.email, platform = EXCLUDED.platform, channel_url = EXCLUDED.channel_url, message = EXCLUDED.message, auto_created_via_oauth = FALSE, updated_at = NOW()")
            .bind(user_id).bind(&app.email).bind(platform).bind(&app.channel_url).bind(&app.message).execute(pool).await.map_err(|_| FoundingError::Database)?;
        self.persistent_get(&app.user_id)
            .await?
            .ok_or(FoundingError::Database)
    }

    pub async fn persistent_list_all(&self) -> Result<Vec<FoundingApplication>, FoundingError> {
        let Some(pool) = &self.db else {
            return Ok(self.list_all());
        };
        let rows: Vec<(String, String, String, String, String, String, i64, i64, bool)> = sqlx::query_as("SELECT user_id::text, email, platform, channel_url, message, status, EXTRACT(EPOCH FROM created_at)*1000::BIGINT, EXTRACT(EPOCH FROM updated_at)*1000::BIGINT, auto_created_via_oauth FROM founding_applications ORDER BY created_at DESC").fetch_all(pool).await.map_err(|_| FoundingError::Database)?;
        rows.into_iter().map(application_from_row).collect()
    }

    pub async fn persistent_set_status(
        &self,
        user_id: &str,
        status: FoundingStatus,
    ) -> Result<Option<FoundingApplication>, FoundingError> {
        let Some(pool) = &self.db else {
            return Ok(self.set_status(user_id, status));
        };
        let id = uuid::Uuid::parse_str(user_id).map_err(|_| FoundingError::Database)?;
        let changed = sqlx::query(
            "UPDATE founding_applications SET status = $2, updated_at = NOW() WHERE user_id = $1",
        )
        .bind(id)
        .bind(enum_value(status)?)
        .execute(pool)
        .await
        .map_err(|_| FoundingError::Database)?;
        if changed.rows_affected() == 0 {
            return Ok(None);
        }
        self.persistent_get(user_id).await
    }

    pub async fn persistent_auto_apply_if_absent(
        &self,
        user_id: &str,
        email: &str,
        platform: FoundingPlatform,
        channel_url: &str,
    ) -> Result<Option<FoundingApplication>, FoundingError> {
        let Some(pool) = &self.db else {
            return Ok(self.auto_apply_if_absent(user_id, email, platform, channel_url));
        };
        let id = uuid::Uuid::parse_str(user_id).map_err(|_| FoundingError::Database)?;
        let result = sqlx::query("INSERT INTO founding_applications (user_id, email, platform, channel_url, message, status, auto_created_via_oauth) VALUES ($1, $2, $3, $4, '', 'pending', TRUE) ON CONFLICT (user_id) DO NOTHING")
            .bind(id).bind(email).bind(enum_value(platform)?).bind(channel_url).execute(pool).await.map_err(|_| FoundingError::Database)?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        self.persistent_get(user_id).await
    }
}

fn enum_value<T: Serialize>(value: T) -> Result<String, FoundingError> {
    serde_json::to_value(value)
        .map_err(FoundingError::Parse)?
        .as_str()
        .map(str::to_string)
        .ok_or(FoundingError::Database)
}

fn application_from_row(
    row: (
        String,
        String,
        String,
        String,
        String,
        String,
        i64,
        i64,
        bool,
    ),
) -> Result<FoundingApplication, FoundingError> {
    let (
        user_id,
        email,
        platform,
        channel_url,
        message,
        status,
        created_at_ms,
        updated_at_ms,
        auto_created_via_oauth,
    ) = row;
    Ok(FoundingApplication {
        user_id,
        email,
        platform: serde_json::from_value(serde_json::Value::String(platform))
            .map_err(FoundingError::Parse)?,
        channel_url,
        message,
        status: serde_json::from_value(serde_json::Value::String(status))
            .map_err(FoundingError::Parse)?,
        created_at_ms,
        updated_at_ms,
        auto_created_via_oauth,
    })
}

// ── HTTP handler'ları ────────────────────────────────────────────────────

/// `POST /api/founding/application` gövdesi.
#[derive(Debug, Deserialize)]
pub struct SubmitApplicationRequest {
    pub platform: FoundingPlatform,
    pub channel_url: String,
    #[serde(default)]
    pub message: String,
}

/// `POST /api/founding/application` — yayıncı platform/kanal bilgisiyle
/// founding beta'ya başvurur (upsert; tekrar gönderim admin kararını korur).
pub async fn submit_application(
    user: AuthUser,
    State(users): State<Arc<UserStore>>,
    State(store): State<Arc<FoundingStore>>,
    Json(req): Json<SubmitApplicationRequest>,
) -> Result<(StatusCode, Json<FoundingApplication>), (StatusCode, String)> {
    let channel_url = req.channel_url.trim();
    if channel_url.is_empty() {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "channel_url must not be empty".to_string(),
        ));
    }
    if channel_url.len() > MAX_CHANNEL_URL_LEN {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("channel_url too long (max {MAX_CHANNEL_URL_LEN} chars)"),
        ));
    }
    if req.message.len() > MAX_MESSAGE_LEN {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("message too long (max {MAX_MESSAGE_LEN} chars)"),
        ));
    }

    let email = users
        .persistent_get_by_id(&user.claims.sub)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "user service unavailable".to_string(),
            )
        })?
        .map(|record| record.email)
        .unwrap_or_default();

    let application = store
        .persistent_upsert(&FoundingApplication {
            user_id: user.claims.sub.clone(),
            email,
            platform: req.platform,
            channel_url: channel_url.to_string(),
            message: req.message,
            status: FoundingStatus::Pending,
            created_at_ms: 0,
            updated_at_ms: 0,
            auto_created_via_oauth: false,
        })
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "founding service unavailable".to_string(),
            )
        })?;
    Ok((StatusCode::CREATED, Json(application)))
}

/// `founding.gate.enabled` config değerini okur. `ws::mod::handle_control`
/// (StreamStart kapısı) ve `gate_status` handler'ı (dashboard'a görünürlük)
/// aynı okuma mantığını paylaşır — ikisi de aynı config anahtarına bakar.
pub fn gate_enabled(cfg: &ServiceConfig) -> bool {
    cfg.get("founding.gate.enabled").as_deref() == Some("true")
}

/// `GET /api/founding/gate` — yayıncının "Go live" gibi aksiyonları
/// gate'lemesi için kapının açık/kapalı olduğunu bildirir (önceden sadece
/// admin `PUT /admin/config` ile okuyabiliyordu).
pub async fn gate_status(
    _user: AuthUser,
    State(cfg): State<Arc<ServiceConfig>>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "enabled": gate_enabled(&cfg) }))
}

/// `GET /api/founding/application` — yayıncının kendi başvurusu (yoksa 404).
pub async fn get_my_application(
    user: AuthUser,
    State(store): State<Arc<FoundingStore>>,
) -> Result<Json<FoundingApplication>, StatusCode> {
    store
        .persistent_get(&user.claims.sub)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// `GET /admin/founding/applications` — tüm başvurular (inceleme kuyruğu).
pub async fn list_applications(
    _admin: AdminUser,
    State(store): State<Arc<FoundingStore>>,
) -> Result<Json<Vec<FoundingApplication>>, StatusCode> {
    store
        .persistent_list_all()
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// `POST /admin/founding/{user_id}/approve`.
pub async fn approve_application(
    _admin: AdminUser,
    State(store): State<Arc<FoundingStore>>,
    Path(user_id): Path<String>,
) -> Result<Json<FoundingApplication>, StatusCode> {
    store
        .persistent_set_status(&user_id, FoundingStatus::Approved)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// `POST /admin/founding/{user_id}/revoke`.
pub async fn revoke_application(
    _admin: AdminUser,
    State(store): State<Arc<FoundingStore>>,
    Path(user_id): Path<String>,
) -> Result<Json<FoundingApplication>, StatusCode> {
    store
        .persistent_set_status(&user_id, FoundingStatus::Revoked)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_enabled_reads_config() {
        let cfg = ServiceConfig::with_defaults();
        assert!(!gate_enabled(&cfg));
        cfg.set("founding.gate.enabled", "true");
        assert!(gate_enabled(&cfg));
    }

    #[test]
    fn new_application_starts_pending() {
        let store = FoundingStore::in_memory();
        let app = store.upsert_application(
            "u1",
            "u1@example.com",
            FoundingPlatform::Kick,
            "https://kick.com/u1",
            "hi",
        );
        assert_eq!(app.status, FoundingStatus::Pending);
        assert_eq!(app.user_id, "u1");
        assert!(!store.is_approved("u1"));
    }

    #[test]
    fn approve_and_revoke_round_trip() {
        let store = FoundingStore::in_memory();
        store.upsert_application(
            "u1",
            "u1@example.com",
            FoundingPlatform::Youtube,
            "https://youtube.com/@u1",
            "",
        );
        assert!(!store.is_approved("u1"));

        let approved = store.set_status("u1", FoundingStatus::Approved).unwrap();
        assert_eq!(approved.status, FoundingStatus::Approved);
        assert!(store.is_approved("u1"));

        let revoked = store.set_status("u1", FoundingStatus::Revoked).unwrap();
        assert_eq!(revoked.status, FoundingStatus::Revoked);
        assert!(!store.is_approved("u1"));
    }

    #[test]
    fn set_status_on_unknown_user_is_none() {
        let store = FoundingStore::in_memory();
        assert!(store
            .set_status("ghost", FoundingStatus::Approved)
            .is_none());
    }

    #[test]
    fn is_approved_false_for_unknown_user() {
        let store = FoundingStore::in_memory();
        assert!(!store.is_approved("ghost"));
    }

    #[test]
    fn resubmitting_updates_fields_but_preserves_admin_decision() {
        let store = FoundingStore::in_memory();
        store.upsert_application(
            "u1",
            "u1@example.com",
            FoundingPlatform::Kick,
            "https://kick.com/u1",
            "v1",
        );
        store.set_status("u1", FoundingStatus::Approved);

        // Kullanıcı formu tekrar gönderir (kanal linkini günceller).
        let updated = store.upsert_application(
            "u1",
            "u1@example.com",
            FoundingPlatform::Kick,
            "https://kick.com/u1-new",
            "v2",
        );
        assert_eq!(updated.status, FoundingStatus::Approved);
        assert_eq!(updated.channel_url, "https://kick.com/u1-new");
        assert_eq!(updated.message, "v2");
        assert!(store.is_approved("u1"));
    }

    #[test]
    fn auto_apply_if_absent_creates_pending_application() {
        let store = FoundingStore::in_memory();
        let app = store
            .auto_apply_if_absent(
                "u1",
                "u1@example.com",
                FoundingPlatform::Youtube,
                "https://youtube.com/channel/u1",
            )
            .expect("no prior application, should create one");
        assert_eq!(app.status, FoundingStatus::Pending);
        assert!(app.auto_created_via_oauth);
        assert_eq!(
            store.get("u1").unwrap().channel_url,
            "https://youtube.com/channel/u1"
        );
    }

    #[test]
    fn auto_apply_if_absent_noop_when_manual_application_exists() {
        let store = FoundingStore::in_memory();
        store.upsert_application(
            "u1",
            "u1@example.com",
            FoundingPlatform::Kick,
            "https://kick.com/u1",
            "manual note",
        );

        let result = store.auto_apply_if_absent(
            "u1",
            "u1@example.com",
            FoundingPlatform::Youtube,
            "https://youtube.com/channel/u1",
        );
        assert!(result.is_none());
        let existing = store.get("u1").unwrap();
        assert_eq!(existing.platform, FoundingPlatform::Kick);
        assert_eq!(existing.channel_url, "https://kick.com/u1");
        assert!(!existing.auto_created_via_oauth);
    }

    #[test]
    fn auto_apply_if_absent_noop_when_already_auto_created() {
        let store = FoundingStore::in_memory();
        store
            .auto_apply_if_absent(
                "u1",
                "u1@example.com",
                FoundingPlatform::Youtube,
                "https://youtube.com/a",
            )
            .unwrap();

        let result = store.auto_apply_if_absent(
            "u1",
            "u1@example.com",
            FoundingPlatform::Twitch,
            "https://twitch.tv/a",
        );
        assert!(result.is_none());
        assert_eq!(store.get("u1").unwrap().platform, FoundingPlatform::Youtube);
    }

    #[test]
    fn list_all_sorted_newest_first() {
        let store = FoundingStore::in_memory();
        let a = store.upsert_application(
            "u1",
            "a@example.com",
            FoundingPlatform::Kick,
            "https://kick.com/a",
            "",
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = store.upsert_application(
            "u2",
            "b@example.com",
            FoundingPlatform::Twitch,
            "https://twitch.tv/b",
            "",
        );

        let all = store.list_all();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].user_id, b.user_id);
        assert_eq!(all[1].user_id, a.user_id);
        assert!(a.created_at_ms <= b.created_at_ms);
    }

    #[test]
    fn persists_to_plain_json_and_reloads() {
        let path = std::env::temp_dir().join(format!(
            "sd-founding-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_file(&path);

        {
            let store = FoundingStore::with_file(&path).unwrap();
            store.upsert_application(
                "u1",
                "u1@example.com",
                FoundingPlatform::Other,
                "https://example.com/u1",
                "",
            );
            store.set_status("u1", FoundingStatus::Approved);
        }

        let reloaded = FoundingStore::with_file(&path).unwrap();
        assert!(reloaded.is_approved("u1"));
        assert_eq!(
            reloaded.get("u1").unwrap().channel_url,
            "https://example.com/u1"
        );

        let _ = std::fs::remove_file(&path);
    }
}
