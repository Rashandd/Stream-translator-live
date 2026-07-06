//! Ses klonlama model kayıtları + admin onay kuyruğu (Faz 2.1).
//!
//! ElevenLabs (veya kayıtlı başka bir `TtsProvider`) "hazır" der demez model
//! kullanılabilir olur — sağlayıcı API'sinde insan onayı kavramı yok.
//! StreamDubber kendi onay kapısını üstüne koyar: yeni oluşturulan her model
//! `pending_review` ile başlar; admin onaylamadan yayıncı onu canlı TTS'te
//! (Faz 2.2) kullanamaz. Ses klonlamanın rıza/kötüye kullanım riski
//! nedeniyle bu insan-onaylı adım bilinçli bir tasarım kararı, teknik bir
//! sağlayıcı kısıtı değil.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::auth::authz::{AdminUser, AuthUser};
use crate::gateway::Gateway;
use crate::plans::PlanRegistry;

/// Ses örneği üst sınırı (decode sonrası, ham bayt) — yanlışlıkla/kötüye
/// kullanım amaçlı dev yüklemeleri engeller.
const MAX_SAMPLE_BYTES: usize = 10 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceModelStatus {
    /// Sağlayıcıda hâlâ eğitiliyor — bugünkü senkron `clone_voice` akışında
    /// hiç gözlemlenmez (ileride sağlayıcı asenkron eğitim sunarsa kullanılır).
    Processing,
    PendingReview,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceModel {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub provider_voice_id: String,
    pub status: VoiceModelStatus,
    pub created_at: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum VoiceModelError {
    #[error("store file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("store file parse error: {0}")]
    Parse(#[from] serde_json::Error),
}

/// Model id → kayıt deposu (opsiyonel düz-JSON kalıcılık — model metadata'sı
/// secret değil; ham ses örneği hiç saklanmaz, sadece sağlayıcının döndürdüğü
/// `provider_voice_id` tutulur).
pub struct VoiceModelStore {
    inner: Mutex<HashMap<String, VoiceModel>>,
    path: Option<PathBuf>,
}

impl VoiceModelStore {
    pub fn in_memory() -> Self {
        VoiceModelStore {
            inner: Mutex::new(HashMap::new()),
            path: None,
        }
    }

    pub fn with_file(path: impl AsRef<std::path::Path>) -> Result<Self, VoiceModelError> {
        let path = path.as_ref().to_path_buf();
        let inner = match std::fs::read_to_string(&path) {
            Ok(content) if !content.trim().is_empty() => serde_json::from_str(&content)?,
            Ok(_) => HashMap::new(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
            Err(e) => return Err(VoiceModelError::Io(e)),
        };
        Ok(VoiceModelStore {
            inner: Mutex::new(inner),
            path: Some(path),
        })
    }

    pub fn insert(&self, model: VoiceModel) {
        self.persist_with(|map| {
            map.insert(model.id.clone(), model);
        });
    }

    pub fn get(&self, id: &str) -> Option<VoiceModel> {
        self.inner
            .lock()
            .expect("voice model lock poisoned")
            .get(id)
            .cloned()
    }

    /// Bir kullanıcının kendi modelleri, oluşturulma sırasına göre.
    pub fn list_for_user(&self, user_id: &str) -> Vec<VoiceModel> {
        let mut models: Vec<VoiceModel> = self
            .inner
            .lock()
            .expect("voice model lock poisoned")
            .values()
            .filter(|m| m.user_id == user_id)
            .cloned()
            .collect();
        models.sort_by_key(|m| m.created_at);
        models
    }

    /// Tüm kullanıcıların tüm modelleri (admin onay kuyruğu).
    pub fn list_all(&self) -> Vec<VoiceModel> {
        let mut models: Vec<VoiceModel> = self
            .inner
            .lock()
            .expect("voice model lock poisoned")
            .values()
            .cloned()
            .collect();
        models.sort_by_key(|m| m.created_at);
        models
    }

    /// Durumu günceller (admin onay/red). Kayıt yoksa `None`.
    pub fn set_status(&self, id: &str, status: VoiceModelStatus) -> Option<VoiceModel> {
        let mut updated = None;
        self.persist_with(|map| {
            if let Some(model) = map.get_mut(id) {
                model.status = status;
                updated = Some(model.clone());
            }
        });
        updated
    }

    fn persist_with(&self, apply: impl FnOnce(&mut HashMap<String, VoiceModel>)) {
        let snapshot = {
            let mut map = self.inner.lock().expect("voice model lock poisoned");
            apply(&mut map);
            self.path
                .as_ref()
                .map(|_| serde_json::to_string_pretty(&*map).unwrap_or_default())
        };
        if let (Some(json), Some(path)) = (snapshot, &self.path) {
            if let Err(err) = std::fs::write(path, json) {
                tracing::warn!(error = %err, "voice model store persist failed");
            }
        }
    }
}

// ── HTTP handler'ları ────────────────────────────────────────────────────

/// `POST /api/voice/clone` gövdesi.
#[derive(Debug, Deserialize)]
pub struct CloneVoiceRequest {
    pub name: String,
    /// Base64 kodlu ses örneği (WAV/MP3 — sağlayıcıya WAV olarak iletilir).
    pub sample_base64: String,
}

/// `POST /api/voice/clone` — ses örneğinden klonlanmış bir model oluşturur.
/// Plan `has_voice_clone` şartı (free/starter'da 403); model `pending_review`
/// ile başlar — admin onaylamadan Faz 2.2 canlı TTS akışında kullanılamaz.
pub async fn clone_voice(
    user: AuthUser,
    State(gateway): State<Arc<Gateway>>,
    State(plans): State<Arc<PlanRegistry>>,
    State(store): State<Arc<VoiceModelStore>>,
    Json(req): Json<CloneVoiceRequest>,
) -> Result<Json<VoiceModel>, (StatusCode, String)> {
    let allowed = plans
        .get(&user.claims.plan)
        .map(|p| p.has_voice_clone)
        .unwrap_or(false);
    if !allowed {
        return Err((
            StatusCode::FORBIDDEN,
            "voice cloning is not available on your plan".to_string(),
        ));
    }
    if req.name.trim().is_empty() {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "name must not be empty".to_string(),
        ));
    }

    let sample = B64
        .decode(&req.sample_base64)
        .map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, format!("invalid base64: {e}")))?;
    if sample.is_empty() {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "sample must not be empty".to_string(),
        ));
    }
    if sample.len() > MAX_SAMPLE_BYTES {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("sample too large (max {MAX_SAMPLE_BYTES} bytes)"),
        ));
    }

    let provider_voice_id = gateway
        .clone_voice(&req.name, &sample)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;

    let model = VoiceModel {
        id: uuid::Uuid::new_v4().to_string(),
        user_id: user.claims.sub.clone(),
        name: req.name,
        provider_voice_id,
        status: VoiceModelStatus::PendingReview,
        created_at: chrono::Utc::now().timestamp(),
    };
    store.insert(model.clone());
    Ok(Json(model))
}

/// `GET /api/voice/models` — yayıncının kendi ses modelleri.
pub async fn list_my_voice_models(
    user: AuthUser,
    State(store): State<Arc<VoiceModelStore>>,
) -> Json<Vec<VoiceModel>> {
    Json(store.list_for_user(&user.claims.sub))
}

/// `GET /admin/voice/models` — onay kuyruğu (tüm kullanıcılar, tüm durumlar;
/// panel `pending_review` olanları öne çıkarır).
pub async fn list_all_voice_models(
    _admin: AdminUser,
    State(store): State<Arc<VoiceModelStore>>,
) -> Json<Vec<VoiceModel>> {
    Json(store.list_all())
}

/// `POST /admin/voice/models/{id}/approve`.
pub async fn approve_voice_model(
    _admin: AdminUser,
    State(store): State<Arc<VoiceModelStore>>,
    Path(id): Path<String>,
) -> Result<Json<VoiceModel>, StatusCode> {
    store
        .set_status(&id, VoiceModelStatus::Approved)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// `POST /admin/voice/models/{id}/reject`.
pub async fn reject_voice_model(
    _admin: AdminUser,
    State(store): State<Arc<VoiceModelStore>>,
    Path(id): Path<String>,
) -> Result<Json<VoiceModel>, StatusCode> {
    store
        .set_status(&id, VoiceModelStatus::Rejected)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(user_id: &str, created_at: i64) -> VoiceModel {
        VoiceModel {
            id: uuid::Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            name: "test voice".to_string(),
            provider_voice_id: "provider-voice-1".to_string(),
            status: VoiceModelStatus::PendingReview,
            created_at,
        }
    }

    #[test]
    fn lists_only_the_owning_user_models_sorted_by_creation() {
        let store = VoiceModelStore::in_memory();
        store.insert(model("u1", 200));
        store.insert(model("u1", 100));
        store.insert(model("u2", 150));

        let mine = store.list_for_user("u1");
        assert_eq!(mine.len(), 2);
        assert_eq!(mine[0].created_at, 100);
        assert_eq!(mine[1].created_at, 200);
        assert!(store.list_for_user("u3").is_empty());
    }

    #[test]
    fn list_all_spans_every_user() {
        let store = VoiceModelStore::in_memory();
        store.insert(model("u1", 1));
        store.insert(model("u2", 2));
        assert_eq!(store.list_all().len(), 2);
    }

    #[test]
    fn set_status_updates_and_persists_in_memory() {
        let store = VoiceModelStore::in_memory();
        let m = model("u1", 1);
        let id = m.id.clone();
        store.insert(m);

        let updated = store.set_status(&id, VoiceModelStatus::Approved).expect("updated");
        assert_eq!(updated.status, VoiceModelStatus::Approved);
        assert_eq!(store.get(&id).unwrap().status, VoiceModelStatus::Approved);
    }

    #[test]
    fn set_status_on_unknown_id_is_none() {
        let store = VoiceModelStore::in_memory();
        assert!(store.set_status("ghost", VoiceModelStatus::Approved).is_none());
    }

    #[test]
    fn persists_to_plain_json_and_reloads() {
        let path = std::env::temp_dir().join(format!(
            "sd-voice-models-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_file(&path);

        let id = {
            let store = VoiceModelStore::with_file(&path).unwrap();
            let m = model("u1", 42);
            let id = m.id.clone();
            store.insert(m);
            id
        };

        let reloaded = VoiceModelStore::with_file(&path).unwrap();
        assert_eq!(reloaded.get(&id).unwrap().user_id, "u1");

        let _ = std::fs::remove_file(&path);
    }
}
