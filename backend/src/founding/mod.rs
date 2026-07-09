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
}

#[derive(Debug, thiserror::Error)]
pub enum FoundingError {
    #[error("store file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("store file parse error: {0}")]
    Parse(#[from] serde_json::Error),
}

/// user_id → başvuru deposu (opsiyonel düz-JSON kalıcılık — kanal linki
/// secret değil; `voice_models::VoiceModelStore` ile aynı desen).
pub struct FoundingStore {
    inner: Mutex<HashMap<String, FoundingApplication>>,
    path: Option<PathBuf>,
}

impl FoundingStore {
    pub fn in_memory() -> Self {
        FoundingStore {
            inner: Mutex::new(HashMap::new()),
            path: None,
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
        })
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
    /// tekrar gönderse bile onay/iptal kararı bozulmaz.
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
            },
        };
        map.insert(user_id.to_string(), application.clone());
        drop(map);
        self.save();
        application
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
        .get_by_id(&user.claims.sub)
        .map(|record| record.email)
        .unwrap_or_default();

    let application = store.upsert_application(
        &user.claims.sub,
        &email,
        req.platform,
        channel_url,
        &req.message,
    );
    Ok((StatusCode::CREATED, Json(application)))
}

/// `GET /api/founding/application` — yayıncının kendi başvurusu (yoksa 404).
pub async fn get_my_application(
    user: AuthUser,
    State(store): State<Arc<FoundingStore>>,
) -> Result<Json<FoundingApplication>, StatusCode> {
    store
        .get(&user.claims.sub)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// `GET /admin/founding/applications` — tüm başvurular (inceleme kuyruğu).
pub async fn list_applications(
    _admin: AdminUser,
    State(store): State<Arc<FoundingStore>>,
) -> Json<Vec<FoundingApplication>> {
    Json(store.list_all())
}

/// `POST /admin/founding/{user_id}/approve`.
pub async fn approve_application(
    _admin: AdminUser,
    State(store): State<Arc<FoundingStore>>,
    Path(user_id): Path<String>,
) -> Result<Json<FoundingApplication>, StatusCode> {
    store
        .set_status(&user_id, FoundingStatus::Approved)
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
        .set_status(&user_id, FoundingStatus::Revoked)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

#[cfg(test)]
mod tests {
    use super::*;

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
