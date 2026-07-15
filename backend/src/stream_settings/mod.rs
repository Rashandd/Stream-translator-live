//! Yayıncının hedef dil seçimi (Faz 2.0) — dashboard'daki çoklu dil seçim
//! UI'ının kaydettiği yer. Düz JSON kalıcılık (dil kodları secret değil —
//! `sdk_link::SttSettingsStore` ile aynı desen).
//!
//! Not: `ws::handle_control`'deki `stream_start` hâlâ SDK'nın o an
//! gönderdiği `target_langs`'i kullanır (protokol değişmedi); bu store
//! panelin seçimini kalıcılaştırır ve `GET /api/translate/languages` onu
//! panelin `PUT` ile yazdığı hâliyle geri okur. SDK'nın stream-start anında
//! bu ayarları çekmesi (relay_sync'in relay hedeflerini çektiği gibi) ayrı
//! bir iş — SDK henüz `run_desktop_shell`'de gerçek bir `stream_start`
//! kurmuyor (yalnız relay + opsiyonel stt-control loop'u var).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::auth::authz::AuthUser;
use crate::config::ServiceConfig;
use crate::integrations::oauth::plan_allowed;
use crate::plans::PlanRegistry;
use crate::translate_languages::DEEPL_TARGET_LANGUAGES;

fn default_source_lang() -> String {
    "auto".to_string()
}

/// Kullanıcı başına kaydedilmiş dil ayarları.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamLanguageSettings {
    #[serde(default = "default_source_lang")]
    pub source_lang: String,
    #[serde(default)]
    pub target_langs: Vec<String>,
}

impl Default for StreamLanguageSettings {
    fn default() -> Self {
        StreamLanguageSettings {
            source_lang: default_source_lang(),
            target_langs: Vec::new(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StreamSettingsError {
    #[error("settings file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("settings file parse error: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("stream settings database error")]
    Database,
}

/// Kullanıcı → dil ayarları deposu (opsiyonel düz-JSON kalıcılık).
pub struct StreamSettingsStore {
    inner: Mutex<HashMap<String, StreamLanguageSettings>>,
    path: Option<PathBuf>,
    db: Option<sqlx::PgPool>,
}

impl StreamSettingsStore {
    /// Bellek içi depo (dev/CI — restart'ta varsayılana döner).
    pub fn in_memory() -> Self {
        StreamSettingsStore {
            inner: Mutex::new(HashMap::new()),
            path: None,
            db: None,
        }
    }

    /// Dosya kalıcılığıyla kurar; mevcut içerik geri yüklenir.
    pub fn with_file(path: impl AsRef<Path>) -> Result<Self, StreamSettingsError> {
        let path = path.as_ref().to_path_buf();
        let inner = match std::fs::read_to_string(&path) {
            Ok(content) if !content.trim().is_empty() => serde_json::from_str(&content)?,
            Ok(_) => HashMap::new(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
            Err(e) => return Err(StreamSettingsError::Io(e)),
        };
        Ok(StreamSettingsStore {
            inner: Mutex::new(inner),
            path: Some(path),
            db: None,
        })
    }

    pub fn with_db(mut self, pool: sqlx::PgPool) -> Self {
        self.db = Some(pool);
        self
    }

    /// Kullanıcının ayarları (kayıt yoksa varsayılan: auto kaynak, boş hedef).
    pub fn get(&self, user_id: &str) -> StreamLanguageSettings {
        self.inner
            .lock()
            .expect("stream settings lock poisoned")
            .get(user_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Ayarları yazar+kalıcılaştırır. Doğrulama çağıran handler'da
    /// (`put_languages`) yapılır — store yalnızca kalıcılık katmanı.
    pub fn set(&self, user_id: &str, settings: StreamLanguageSettings) {
        let snapshot = {
            let mut map = self.inner.lock().expect("stream settings lock poisoned");
            map.insert(user_id.to_string(), settings);
            self.path
                .as_ref()
                .map(|_| serde_json::to_string_pretty(&*map).unwrap_or_default())
        };
        if let (Some(json), Some(path)) = (snapshot, &self.path) {
            if let Err(err) = std::fs::write(path, json) {
                tracing::warn!(error = %err, "stream language settings persist failed");
            }
        }
    }

    pub async fn persistent_get(
        &self,
        user_id: &str,
    ) -> Result<StreamLanguageSettings, StreamSettingsError> {
        let Some(pool) = &self.db else {
            return Ok(self.get(user_id));
        };
        let user_id = uuid::Uuid::parse_str(user_id).map_err(|_| StreamSettingsError::Database)?;
        let row: Option<(String, Vec<String>)> = sqlx::query_as(
            "SELECT source_lang, target_langs FROM user_stream_settings WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| StreamSettingsError::Database)?;
        Ok(row
            .map(|(source_lang, target_langs)| StreamLanguageSettings {
                source_lang,
                target_langs,
            })
            .unwrap_or_default())
    }

    pub async fn persistent_set(
        &self,
        user_id: &str,
        settings: StreamLanguageSettings,
    ) -> Result<(), StreamSettingsError> {
        let Some(pool) = &self.db else {
            self.set(user_id, settings);
            return Ok(());
        };
        let user_id = uuid::Uuid::parse_str(user_id).map_err(|_| StreamSettingsError::Database)?;
        sqlx::query("INSERT INTO user_stream_settings (user_id, source_lang, target_langs) VALUES ($1, $2, $3) ON CONFLICT (user_id) DO UPDATE SET source_lang = EXCLUDED.source_lang, target_langs = EXCLUDED.target_langs, updated_at = NOW()")
            .bind(user_id).bind(&settings.source_lang).bind(&settings.target_langs).execute(pool).await.map_err(|_| StreamSettingsError::Database)?;
        Ok(())
    }
}

// ── HTTP handler'ları ────────────────────────────────────────────────────

/// `/api/translate/languages` yanıtındaki tek dil seçeneği.
#[derive(Debug, Serialize)]
pub struct LanguageOption {
    pub code: String,
    pub name: String,
    pub supports_formality: bool,
    pub enabled: bool,
}

/// `GET /api/translate/languages` yanıtı.
#[derive(Debug, Serialize)]
pub struct LanguagesResponse {
    pub languages: Vec<LanguageOption>,
    pub max_languages: i64,
    pub source_lang: String,
    pub target_langs: Vec<String>,
}

/// `GET /api/translate/catalog` yanıtındaki tek katalog kaydı — kullanıcıya
/// özel alan yok (`enabled`/seçim), bu yüzden auth gerektirmez.
#[derive(Debug, Serialize)]
pub struct LanguageCatalogEntry {
    pub code: String,
    pub name: String,
    pub supports_formality: bool,
}

/// `GET /api/translate/catalog` — DeepL kataloğunun tamamı, auth gerekmez
/// (gizli veri içermez). Admin panelin "hangi diller açık" listesini
/// `translate.deepl.enabled_targets` config'iyle birlikte oluşturması için;
/// yayıncı tarafı zaten `GET /api/translate/languages`'ta aynı veriyi
/// kendine özel alanlarla (plan azami sayısı, kayıtlı seçim) birlikte alır.
pub async fn get_catalog() -> Json<Vec<LanguageCatalogEntry>> {
    Json(
        DEEPL_TARGET_LANGUAGES
            .iter()
            .map(|entry| LanguageCatalogEntry {
                code: entry.code.to_string(),
                name: entry.name.to_string(),
                supports_formality: entry.supports_formality,
            })
            .collect(),
    )
}

/// `GET /api/translate/languages` — tam katalog + admin'in açtığı diller +
/// yayıncının planının izin verdiği azami sayı + kayıtlı seçimi tek çağrıda.
pub async fn get_languages(
    user: AuthUser,
    State(config): State<Arc<ServiceConfig>>,
    State(plans): State<Arc<PlanRegistry>>,
    State(store): State<Arc<StreamSettingsStore>>,
) -> Result<Json<LanguagesResponse>, (StatusCode, String)> {
    let enabled_csv = config
        .get("translate.deepl.enabled_targets")
        .unwrap_or_default();
    let languages = DEEPL_TARGET_LANGUAGES
        .iter()
        .map(|entry| LanguageOption {
            code: entry.code.to_string(),
            name: entry.name.to_string(),
            supports_formality: entry.supports_formality,
            enabled: plan_allowed(&enabled_csv, entry.code),
        })
        .collect();
    let max_languages = plans
        .persistent_get(&user.claims.plan)
        .await
        .map_err(|err| {
            tracing::warn!(%err, "plans: language settings read failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "plan service unavailable".to_string(),
            )
        })?
        .map(|p| p.max_languages)
        .unwrap_or(1);
    let settings = store.persistent_get(&user.claims.sub).await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "stream settings unavailable".to_string(),
        )
    })?;
    Ok(Json(LanguagesResponse {
        languages,
        max_languages,
        source_lang: settings.source_lang,
        target_langs: settings.target_langs,
    }))
}

/// `PUT /api/translate/languages` gövdesi.
#[derive(Debug, Deserialize)]
pub struct PutLanguagesRequest {
    #[serde(default = "default_source_lang")]
    pub source_lang: String,
    pub target_langs: Vec<String>,
}

/// `PUT /api/translate/languages` — yayıncının hedef dil seçimini kaydeder.
/// Admin'in kapattığı/katalogda olmayan bir kod veya plan kapasitesini aşan
/// seçim sayısı 422/403 ile reddedilir — yazılan her şey zaten geçerlidir.
pub async fn put_languages(
    user: AuthUser,
    State(config): State<Arc<ServiceConfig>>,
    State(plans): State<Arc<PlanRegistry>>,
    State(store): State<Arc<StreamSettingsStore>>,
    Json(req): Json<PutLanguagesRequest>,
) -> Result<Json<StreamLanguageSettings>, (StatusCode, String)> {
    if req.target_langs.is_empty() {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "at least one target language required".to_string(),
        ));
    }

    let enabled_csv = config
        .get("translate.deepl.enabled_targets")
        .unwrap_or_default();
    for target in &req.target_langs {
        if !plan_allowed(&enabled_csv, target) {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("target language not enabled: {target}"),
            ));
        }
    }

    let max_languages = plans
        .persistent_get(&user.claims.plan)
        .await
        .map_err(|err| {
            tracing::warn!(%err, "plans: language settings write validation read failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "plan service unavailable".to_string(),
            )
        })?
        .map(|p| p.max_languages)
        .unwrap_or(1);
    if max_languages >= 0 && req.target_langs.len() as i64 > max_languages {
        return Err((
            StatusCode::FORBIDDEN,
            format!("plan allows at most {max_languages} target language(s)"),
        ));
    }

    let settings = StreamLanguageSettings {
        source_lang: req.source_lang,
        target_langs: req.target_langs,
    };
    store
        .persistent_set(&user.claims.sub, settings.clone())
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "stream settings unavailable".to_string(),
            )
        })?;
    Ok(Json(settings))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(source: &str, targets: &[&str]) -> StreamLanguageSettings {
        StreamLanguageSettings {
            source_lang: source.to_string(),
            target_langs: targets.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[tokio::test]
    async fn catalog_endpoint_needs_no_auth_and_matches_the_static_list() {
        let Json(catalog) = get_catalog().await;
        assert_eq!(catalog.len(), DEEPL_TARGET_LANGUAGES.len());
        assert!(catalog
            .iter()
            .any(|c| c.code == "tr" && c.supports_formality));
    }

    #[test]
    fn defaults_to_auto_source_and_empty_targets() {
        let store = StreamSettingsStore::in_memory();
        let s = store.get("nobody");
        assert_eq!(s.source_lang, "auto");
        assert!(s.target_langs.is_empty());
    }

    #[test]
    fn set_and_get_roundtrip_per_user() {
        let store = StreamSettingsStore::in_memory();
        store.set("u1", settings("tr", &["en", "de"]));
        store.set("u2", settings("auto", &["es"]));

        assert_eq!(store.get("u1").target_langs, vec!["en", "de"]);
        assert_eq!(store.get("u2").source_lang, "auto");
        assert!(store.get("u3").target_langs.is_empty());
    }

    #[test]
    fn persists_to_plain_json_and_reloads() {
        let path = std::env::temp_dir().join(format!(
            "sd-stream-settings-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_file(&path);

        {
            let store = StreamSettingsStore::with_file(&path).unwrap();
            store.set("u1", settings("tr", &["en", "es"]));
        }

        let reloaded = StreamSettingsStore::with_file(&path).unwrap();
        assert_eq!(reloaded.get("u1").target_langs, vec!["en", "es"]);

        let _ = std::fs::remove_file(&path);
    }
}
