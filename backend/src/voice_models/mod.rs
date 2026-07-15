//! Ses klonlama model kayıtları + admin onay kuyruğu (Faz 2.1) + dil ataması
//! (Faz 2.2 — roadmap "Öz Kaynaklı Ses Klonlama + Speech-to-Speech" bölümü).
//!
//! ElevenLabs (veya kayıtlı başka bir `TtsProvider`) "hazır" der demez model
//! kullanılabilir olur — sağlayıcı API'sinde insan onayı kavramı yok.
//! StreamDubber kendi onay kapısını üstüne koyar: yeni oluşturulan her model
//! `pending_review` ile başlar; admin onaylamadan yayıncı onu canlı TTS'te
//! (Faz 2.2) kullanamaz. Ses klonlamanın rıza/kötüye kullanım riski
//! nedeniyle bu insan-onaylı adım bilinçli bir tasarım kararı, teknik bir
//! sağlayıcı kısıtı değil.
//!
//! Ham ses örneği artık (2026-07-06 kullanıcı kararı) `VoiceSampleStore`'da
//! AES-256-GCM şifreli saklanır — yeniden eğitim/sağlayıcı değişimi için.
//! Model metadata'sı (bu dosyadaki `VoiceModel`) gizli değildir, düz JSON'da
//! kalır; sadece ham bayt ayrı, şifreli bir dosyada durur.

mod sample_store;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde::{Deserialize, Serialize};

pub use sample_store::{VoiceSampleError, VoiceSampleStore};

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

/// Modelin dil yeteneği (2026-07-06 kullanıcı kararı — roadmap "Öz Kaynaklı
/// Ses Klonlama + Speech-to-Speech" bölümü madde 7). `language: String`
/// düzlüğü yerine üç modlu bir şema: hem "yayıncı+hedef dil başına ayrı
/// model" senaryosuyla hem "tek klonlanmış ses, çok dilde sentez" (bugünkü
/// ElevenLabs `eleven_multilingual_v2` davranışı) senaryosuyla uyumlu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguageMode {
    /// Tek kaynak dilini algılar/kabul eder, tek hedef dile çevirir/sentezler.
    /// N hedef dil = N ayrı model (admin her (yayıncı, hedef dil) çifti için
    /// ayrı model atar).
    SingleToSingle,
    /// Birden fazla kaynak dilini algılayabilir, ama tek sabit hedef dile
    /// çevirir.
    MultiToSingle,
    /// Tek kaynak dilinden eğitilir, ama motor çok dilli olduğu için AYNI
    /// model birden fazla hedef dilde sentez yapabilir — ayrı model/onay
    /// gerekmez. Bugünkü ElevenLabs entegrasyonu bu moda en yakın (tek
    /// `provider_voice_id`, hangi dilde metin verirsen o dilde konuşur).
    SingleToMulti,
}

/// Yayıncının seçtiği dublaj modu (roadmap madde 1). Gerçek Speech-to-Speech
/// (metin ara katmanı olmadan, orijinal tonlama korunarak) native bir motor
/// gerektirir ve roadmap'in sonunda ayrı bir araştırma notu olarak duruyor —
/// bu alan sadece şemayı hazırlar; `SpeechToSpeech` seçilse bile Faz 2.2
/// pipeline dispatch'i (bkz. `pipeline::StreamSession`) şimdilik yalnızca
/// `Dubbing` yolunu çalıştırır.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceMode {
    #[default]
    Dubbing,
    SpeechToSpeech,
}

/// Modelin kaynağı — kullanıcının kendi klonu, veya (ileride) StreamDubber'ın
/// kendi self-hosted motoruyla ürettiği hazır bir "seçili ses" (roadmap
/// madde 4 — self-hosted motor olmadan bu kaynaktan hiçbir model YOK, KALAN).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceSource {
    #[default]
    UserCloned,
    Preset,
}

/// Yayıncının ses klonlama için verdiği rıza kaydı (docs/security.md
/// GDPR/KVKK — ses biyometrik veri sayılır, açık/spesifik rıza gerektirir).
/// `clone_voice`'un zorunlu ön koşulu: `is_own_voice=false` veya
/// `text_version` boşsa model asla oluşturulmaz, `pending_review`'a bile
/// giremez. Admin onay kapısı (bkz. modül dokümantasyonu) kötüye kullanımı
/// gözden geçirir; bu kayıt ise hukuki rızanın kendisidir — ikisi ayrı
/// amaçlara hizmet eder.
///
/// `#[serde(default)]` ile `VoiceModel` üstünden okunur: bu alan eklenmeden
/// önce diske yazılmış kayıtlar `is_own_voice=false` (rızasız) olarak geri
/// yüklenir — fail-closed, eski kayıtları geriye dönük "onaylı" saymaz.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VoiceConsent {
    /// Onaylanan rıza metninin versiyonu (ör. "v1") — hangi metne onay
    /// verildiğinin kanıtı olarak saklanır.
    pub text_version: String,
    /// Yayıncının "bu benim kendi sesim" beyanı. Başkasının sesini klonlama
    /// (rızasız taklit) bu akıştan geçemez.
    pub is_own_voice: bool,
    /// Sunucunun kayıt anı — client'tan gelmez, spoofing'e kapalı.
    #[serde(default)]
    pub accepted_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceModel {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub provider_voice_id: String,
    pub status: VoiceModelStatus,
    pub created_at: i64,
    pub language_mode: LanguageMode,
    /// Eğitim örneğinin/örneklerinin dili (`SingleToSingle`/`SingleToMulti`
    /// için tek eleman, `MultiToSingle` için birden fazla).
    pub source_languages: Vec<String>,
    /// Adminin bu model için aktif kıldığı hedef diller (`set_active_for_language`
    /// tarafından doldurulur — oluşturma anında boş başlar). `SingleToSingle`/
    /// `MultiToSingle` için en fazla 1 eleman; `SingleToMulti` için birden
    /// fazla olabilir (aynı model birden fazla hedef dile atanabilir).
    #[serde(default)]
    pub target_languages: Vec<String>,
    #[serde(default)]
    pub mode: VoiceMode,
    #[serde(default)]
    pub source: VoiceSource,
    /// Ham ses örneği `VoiceSampleStore`'a şifreli kaydedildiyse `true`.
    #[serde(default)]
    pub has_stored_sample: bool,
    /// Klonlama rızası — `clone_voice` bunu doğrulamadan model oluşturmaz.
    #[serde(default)]
    pub consent: VoiceConsent,
}

#[derive(Debug, thiserror::Error)]
pub enum VoiceModelError {
    #[error("store file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("store file parse error: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("voice model database error")]
    Database,
}

/// `set_active_for_language` hataları.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SetActiveError {
    #[error("voice model not found")]
    NotFound,
    #[error("voice model is not approved")]
    NotApproved,
    #[error("voice model is already assigned to a different target language")]
    LanguageModeConflict,
}

/// Dosyaya yazılan tam durum — model kayıtları + admin'in aktif dil ataması.
#[derive(Debug, Default, Serialize, Deserialize)]
struct PersistedState {
    #[serde(default)]
    models: HashMap<String, VoiceModel>,
    #[serde(default)]
    active: HashMap<String, String>,
}

/// (user_id, lang) çiftini tek bir anahtar string'ine indirger — JSON obje
/// anahtarları tuple olamaz.
fn active_key(user_id: &str, lang: &str) -> String {
    format!("{user_id}\u{1f}{lang}")
}

/// Model id → kayıt deposu (opsiyonel düz-JSON kalıcılık — model metadata'sı
/// secret değil; ham ses örneği ayrı, şifreli `VoiceSampleStore`'da durur)
/// + admin'in (kullanıcı, hedef dil) başına aktif model ataması.
pub struct VoiceModelStore {
    inner: Mutex<HashMap<String, VoiceModel>>,
    active: Mutex<HashMap<String, String>>,
    path: Option<PathBuf>,
    db: Option<sqlx::PgPool>,
}

impl VoiceModelStore {
    pub fn in_memory() -> Self {
        VoiceModelStore {
            inner: Mutex::new(HashMap::new()),
            active: Mutex::new(HashMap::new()),
            path: None,
            db: None,
        }
    }

    pub fn with_file(path: impl AsRef<std::path::Path>) -> Result<Self, VoiceModelError> {
        let path = path.as_ref().to_path_buf();
        let state: PersistedState = match std::fs::read_to_string(&path) {
            Ok(content) if !content.trim().is_empty() => serde_json::from_str(&content)?,
            Ok(_) => PersistedState::default(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => PersistedState::default(),
            Err(e) => return Err(VoiceModelError::Io(e)),
        };
        Ok(VoiceModelStore {
            inner: Mutex::new(state.models),
            active: Mutex::new(state.active),
            path: Some(path),
            db: None,
        })
    }

    pub fn with_db(mut self, pool: sqlx::PgPool) -> Self {
        self.db = Some(pool);
        self
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

    /// Durumu günceller (admin onay/red). Kayıt yoksa `None`. Not: reddedilen
    /// bir modelin önceden aktif kaldığı (kullanıcı, dil) atamaları burada
    /// silinmez — `active_model_for` zaten yalnız `Approved` modelleri
    /// döndürür, bu yüzden stale bir atama zararsız kalır.
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

    /// Admin: `model_id`'yi, sahibinin `lang` hedef dili için AKTİF model
    /// yapar (Faz 2.2 canlı TTS dispatch'i `active_model_for` ile okur).
    /// Model onaylı değilse veya `language_mode` zaten farklı bir tek hedefe
    /// kilitliyse reddeder.
    pub fn set_active_for_language(
        &self,
        model_id: &str,
        lang: &str,
    ) -> Result<VoiceModel, SetActiveError> {
        let (user_id, updated) = {
            let mut map = self.inner.lock().expect("voice model lock poisoned");
            let model = map.get_mut(model_id).ok_or(SetActiveError::NotFound)?;
            if model.status != VoiceModelStatus::Approved {
                return Err(SetActiveError::NotApproved);
            }
            match model.language_mode {
                LanguageMode::SingleToSingle | LanguageMode::MultiToSingle => {
                    match model.target_languages.first() {
                        Some(existing) if existing != lang => {
                            return Err(SetActiveError::LanguageModeConflict);
                        }
                        Some(_) => {}
                        None => model.target_languages.push(lang.to_string()),
                    }
                }
                LanguageMode::SingleToMulti => {
                    if !model.target_languages.iter().any(|l| l == lang) {
                        model.target_languages.push(lang.to_string());
                    }
                }
            }
            (model.user_id.clone(), model.clone())
        };
        {
            let mut active = self.active.lock().expect("voice model active lock poisoned");
            active.insert(active_key(&user_id, lang), model_id.to_string());
        }
        self.save();
        Ok(updated)
    }

    /// Bir kullanıcının bir hedef dil için o an aktif modeli — canlı TTS
    /// pipeline dispatch'i (Faz 2.2, `pipeline::StreamSession`) bunu kullanır.
    /// Model sonradan reddedildiyse (`set_status`) `None` döner — atama
    /// haritası ayrıca temizlenmez, bu filtre yeterli.
    pub fn active_model_for(&self, user_id: &str, lang: &str) -> Option<VoiceModel> {
        let model_id = self
            .active
            .lock()
            .expect("voice model active lock poisoned")
            .get(&active_key(user_id, lang))
            .cloned()?;
        self.get(&model_id)
            .filter(|m| m.status == VoiceModelStatus::Approved)
    }

    fn persist_with(&self, apply: impl FnOnce(&mut HashMap<String, VoiceModel>)) {
        {
            let mut map = self.inner.lock().expect("voice model lock poisoned");
            apply(&mut map);
        }
        self.save();
    }

    fn save(&self) {
        let Some(path) = &self.path else { return };
        let models = self.inner.lock().expect("voice model lock poisoned").clone();
        let active = self
            .active
            .lock()
            .expect("voice model active lock poisoned")
            .clone();
        match serde_json::to_string_pretty(&PersistedState { models, active }) {
            Ok(json) => {
                if let Err(err) = std::fs::write(path, json) {
                    tracing::warn!(error = %err, "voice model store persist failed");
                }
            }
            Err(err) => tracing::warn!(error = %err, "voice model store serialize failed"),
        }
    }

    pub async fn persistent_insert(&self, model: &VoiceModel) -> Result<(), VoiceModelError> {
        let Some(pool) = &self.db else { self.insert(model.clone()); return Ok(()); };
        let user_id = uuid::Uuid::parse_str(&model.user_id).map_err(|_| VoiceModelError::Database)?;
        sqlx::query("INSERT INTO voice_models (id, user_id, name, provider_voice_id, status, language_mode, source_languages, target_languages, mode, source, has_stored_sample, consent, created_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,to_timestamp($13))")
            .bind(&model.id).bind(user_id).bind(&model.name).bind(&model.provider_voice_id)
            .bind(enum_value(model.status)?).bind(enum_value(model.language_mode)?)
            .bind(&model.source_languages).bind(&model.target_languages).bind(enum_value(model.mode)?)
            .bind(enum_value(model.source)?).bind(model.has_stored_sample)
            .bind(serde_json::to_value(&model.consent)?).bind(model.created_at)
            .execute(pool).await.map_err(|_| VoiceModelError::Database)?;
        Ok(())
    }

    pub async fn persistent_list_for_user(&self, user_id: &str) -> Result<Vec<VoiceModel>, VoiceModelError> {
        let Some(pool) = &self.db else { return Ok(self.list_for_user(user_id)); };
        let user_id = uuid::Uuid::parse_str(user_id).map_err(|_| VoiceModelError::Database)?;
        let rows = sqlx::query_as::<_, VoiceModelRow>("SELECT id, user_id::text, name, provider_voice_id, status, language_mode, source_languages, target_languages, mode, source, has_stored_sample, consent, EXTRACT(EPOCH FROM created_at)::BIGINT FROM voice_models WHERE user_id=$1 ORDER BY created_at")
            .bind(user_id).fetch_all(pool).await.map_err(|_| VoiceModelError::Database)?;
        rows.into_iter().map(VoiceModel::try_from).collect()
    }

    pub async fn persistent_list_all(&self) -> Result<Vec<VoiceModel>, VoiceModelError> {
        let Some(pool) = &self.db else { return Ok(self.list_all()); };
        let rows = sqlx::query_as::<_, VoiceModelRow>("SELECT id, user_id::text, name, provider_voice_id, status, language_mode, source_languages, target_languages, mode, source, has_stored_sample, consent, EXTRACT(EPOCH FROM created_at)::BIGINT FROM voice_models ORDER BY created_at")
            .fetch_all(pool).await.map_err(|_| VoiceModelError::Database)?;
        rows.into_iter().map(VoiceModel::try_from).collect()
    }

    pub async fn persistent_set_status(&self, id: &str, status: VoiceModelStatus) -> Result<Option<VoiceModel>, VoiceModelError> {
        let Some(pool) = &self.db else { return Ok(self.set_status(id, status)); };
        let changed = sqlx::query("UPDATE voice_models SET status=$2 WHERE id=$1").bind(id).bind(enum_value(status)?).execute(pool).await.map_err(|_| VoiceModelError::Database)?;
        if changed.rows_affected() == 0 { return Ok(None); }
        self.persistent_get(id).await
    }

    pub async fn persistent_get(&self, id: &str) -> Result<Option<VoiceModel>, VoiceModelError> {
        let Some(pool) = &self.db else { return Ok(self.get(id)); };
        let row = sqlx::query_as::<_, VoiceModelRow>("SELECT id, user_id::text, name, provider_voice_id, status, language_mode, source_languages, target_languages, mode, source, has_stored_sample, consent, EXTRACT(EPOCH FROM created_at)::BIGINT FROM voice_models WHERE id=$1")
            .bind(id).fetch_optional(pool).await.map_err(|_| VoiceModelError::Database)?;
        row.map(VoiceModel::try_from).transpose()
    }

    pub async fn persistent_set_has_stored_sample(&self, id: &str, has_sample: bool) -> Result<(), VoiceModelError> {
        let Some(pool) = &self.db else {
            self.persist_with(|models| { if let Some(model) = models.get_mut(id) { model.has_stored_sample = has_sample; } });
            return Ok(());
        };
        sqlx::query("UPDATE voice_models SET has_stored_sample=$2 WHERE id=$1").bind(id).bind(has_sample).execute(pool).await.map_err(|_| VoiceModelError::Database)?;
        Ok(())
    }
}

type VoiceModelRow = (String, String, String, String, String, String, Vec<String>, Vec<String>, String, String, bool, serde_json::Value, i64);

impl TryFrom<VoiceModelRow> for VoiceModel {
    type Error = VoiceModelError;
    fn try_from(row: VoiceModelRow) -> Result<Self, Self::Error> {
        let (id, user_id, name, provider_voice_id, status, language_mode, source_languages, target_languages, mode, source, has_stored_sample, consent, created_at) = row;
        Ok(Self { id, user_id, name, provider_voice_id,
            status: serde_json::from_value(serde_json::Value::String(status))?,
            language_mode: serde_json::from_value(serde_json::Value::String(language_mode))?, source_languages, target_languages,
            mode: serde_json::from_value(serde_json::Value::String(mode))?, source: serde_json::from_value(serde_json::Value::String(source))?, has_stored_sample,
            consent: serde_json::from_value(consent)?, created_at })
    }
}

fn enum_value<T: Serialize>(value: T) -> Result<String, VoiceModelError> {
    serde_json::to_value(value)?.as_str().map(str::to_owned).ok_or(VoiceModelError::Database)
}

// ── HTTP handler'ları ────────────────────────────────────────────────────

/// `POST /api/voice/clone` gövdesi.
#[derive(Debug, Deserialize)]
pub struct CloneVoiceRequest {
    pub name: String,
    /// Base64 kodlu ses örneği (WAV/MP3 — sağlayıcıya WAV olarak iletilir).
    pub sample_base64: String,
    pub language_mode: LanguageMode,
    /// Eğitim örneğinin dili (yayıncının kendi panel dilinde okuduğu cümleler
    /// — roadmap madde 2). Hedef dil(ler) burada YOK, admin sonradan
    /// `set_active_for_language` ile atar.
    pub source_languages: Vec<String>,
    #[serde(default)]
    pub mode: VoiceMode,
    #[serde(default)]
    pub source: VoiceSource,
    /// Onaylanan rıza metninin versiyonu — boşsa istek reddedilir.
    pub consent_text_version: String,
    /// "Bu benim kendi sesim" beyanı — `false` ise istek reddedilir.
    pub is_own_voice: bool,
}

/// `POST /api/voice/clone` — ses örneğinden klonlanmış bir model oluşturur.
/// Plan `has_voice_clone` şartı (free/starter'da 403); model `pending_review`
/// ile başlar — admin onaylamadan Faz 2.2 canlı TTS akışında kullanılamaz.
/// Ham örnek (varsa) `VoiceSampleStore`'a şifreli kaydedilir — bu adım
/// best-effort'tur (başarısız olsa da model oluşturma engellenmez,
/// `has_stored_sample=false` kalır).
pub async fn clone_voice(
    user: AuthUser,
    State(gateway): State<Arc<Gateway>>,
    State(plans): State<Arc<PlanRegistry>>,
    State(store): State<Arc<VoiceModelStore>>,
    State(samples): State<Arc<VoiceSampleStore>>,
    Json(req): Json<CloneVoiceRequest>,
) -> Result<Json<VoiceModel>, (StatusCode, String)> {
    let allowed = plans
        .persistent_get(&user.claims.plan)
        .await
        .map_err(|err| {
            tracing::warn!(%err, "plans: voice clone authorization read failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "plan service unavailable".to_string(),
            )
        })?
        .map(|p| p.has_voice_clone)
        .unwrap_or(false);
    if !allowed {
        return Err((
            StatusCode::FORBIDDEN,
            "voice cloning is not available on your plan".to_string(),
        ));
    }
    if req.consent_text_version.trim().is_empty() {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "consent_text_version must not be empty".to_string(),
        ));
    }
    if !req.is_own_voice {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "voice cloning requires confirming this is your own voice (is_own_voice)".to_string(),
        ));
    }
    if req.name.trim().is_empty() {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "name must not be empty".to_string(),
        ));
    }
    if req.source_languages.is_empty() {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "source_languages must not be empty".to_string(),
        ));
    }
    match req.language_mode {
        LanguageMode::SingleToSingle | LanguageMode::SingleToMulti => {
            if req.source_languages.len() != 1 {
                return Err((
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "single_to_single and single_to_multi require exactly one source language"
                        .to_string(),
                ));
            }
        }
        LanguageMode::MultiToSingle => {
            if req.source_languages.len() < 2 {
                return Err((
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "multi_to_single requires at least two source languages".to_string(),
                ));
            }
        }
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

    let id = uuid::Uuid::new_v4().to_string();
    let has_stored_sample = match samples.put(&id, &sample) {
        Ok(()) => true,
        Err(err) => {
            tracing::warn!(error = %err, model_id = %id, "failed to persist encrypted voice sample");
            false
        }
    };

    let now = chrono::Utc::now().timestamp();
    let model = VoiceModel {
        id,
        user_id: user.claims.sub.clone(),
        name: req.name,
        provider_voice_id,
        status: VoiceModelStatus::PendingReview,
        created_at: now,
        language_mode: req.language_mode,
        source_languages: req.source_languages,
        target_languages: Vec::new(),
        mode: req.mode,
        source: req.source,
        has_stored_sample,
        consent: VoiceConsent {
            text_version: req.consent_text_version,
            is_own_voice: req.is_own_voice,
            accepted_at: now,
        },
    };
    store.persistent_insert(&model).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "voice model service unavailable".to_string()))?;
    if has_stored_sample {
        if samples.persistent_put(&model.id, &model.user_id, &sample).await.is_err() {
            store.persistent_set_has_stored_sample(&model.id, false).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "voice model service unavailable".to_string()))?;
        }
    }
    Ok(Json(model))
}

/// `GET /api/voice/models` — yayıncının kendi ses modelleri.
pub async fn list_my_voice_models(
    user: AuthUser,
    State(store): State<Arc<VoiceModelStore>>,
) -> Result<Json<Vec<VoiceModel>>, StatusCode> {
    store.persistent_list_for_user(&user.claims.sub).await.map(Json).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// `GET /admin/voice/models` — onay kuyruğu (tüm kullanıcılar, tüm durumlar;
/// panel `pending_review` olanları öne çıkarır).
pub async fn list_all_voice_models(
    _admin: AdminUser,
    State(store): State<Arc<VoiceModelStore>>,
) -> Result<Json<Vec<VoiceModel>>, StatusCode> {
    store.persistent_list_all().await.map(Json).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// `POST /admin/voice/models/{id}/approve`.
pub async fn approve_voice_model(
    _admin: AdminUser,
    State(store): State<Arc<VoiceModelStore>>,
    Path(id): Path<String>,
) -> Result<Json<VoiceModel>, StatusCode> {
    store
        .persistent_set_status(&id, VoiceModelStatus::Approved).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
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
        .persistent_set_status(&id, VoiceModelStatus::Rejected).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// `POST /admin/voice/models/{id}/assign` gövdesi.
#[derive(Debug, Deserialize)]
pub struct AssignVoiceModelRequest {
    pub target_language: String,
}

/// `POST /admin/voice/models/{id}/assign` — admin bu modeli, sahibinin
/// belirtilen hedef dili için aktif model yapar (Faz 2.2 — yayıncı dilini
/// seçtiğinde admin panelden o dil için curate edilen "en iyi model"
/// budur). `user_id` request'te YOK — modelin kendi sahibi kullanılır,
/// spoofing'e kapalı.
pub async fn assign_voice_model(
    _admin: AdminUser,
    State(store): State<Arc<VoiceModelStore>>,
    Path(id): Path<String>,
    Json(req): Json<AssignVoiceModelRequest>,
) -> Result<Json<VoiceModel>, (StatusCode, String)> {
    if req.target_language.trim().is_empty() {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "target_language must not be empty".to_string(),
        ));
    }
    store
        .set_active_for_language(&id, &req.target_language)
        .map(Json)
        .map_err(|err| match err {
            SetActiveError::NotFound => (StatusCode::NOT_FOUND, err.to_string()),
            SetActiveError::NotApproved => (StatusCode::UNPROCESSABLE_ENTITY, err.to_string()),
            SetActiveError::LanguageModeConflict => (StatusCode::CONFLICT, err.to_string()),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(user_id: &str, created_at: i64) -> VoiceModel {
        model_with_mode(user_id, created_at, LanguageMode::SingleToMulti)
    }

    fn model_with_mode(user_id: &str, created_at: i64, language_mode: LanguageMode) -> VoiceModel {
        VoiceModel {
            id: uuid::Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            name: "test voice".to_string(),
            provider_voice_id: "provider-voice-1".to_string(),
            status: VoiceModelStatus::PendingReview,
            created_at,
            language_mode,
            source_languages: vec!["tr".to_string()],
            target_languages: Vec::new(),
            mode: VoiceMode::Dubbing,
            source: VoiceSource::UserCloned,
            has_stored_sample: false,
            consent: VoiceConsent {
                text_version: "v1".to_string(),
                is_own_voice: true,
                accepted_at: created_at,
            },
        }
    }

    /// Rıza alanı eklenmeden önce yazılmış bir kayıt (JSON'da `consent` yok)
    /// fail-closed geri yüklenmeli: `is_own_voice=false`, asla geriye dönük
    /// "onaylı rıza" varsaymamalı.
    #[test]
    fn missing_consent_field_deserializes_as_not_consented() {
        let legacy_json = r#"{
            "id": "legacy-1",
            "user_id": "u1",
            "name": "legacy voice",
            "provider_voice_id": "provider-voice-1",
            "status": "pending_review",
            "created_at": 1,
            "language_mode": "single_to_multi",
            "source_languages": ["tr"]
        }"#;
        let m: VoiceModel = serde_json::from_str(legacy_json).expect("legacy deserialize");
        assert!(!m.consent.is_own_voice);
        assert_eq!(m.consent.text_version, "");
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

    // ── set_active_for_language / active_model_for (Faz 2.2) ─────────────

    #[test]
    fn assigning_unapproved_model_is_rejected() {
        let store = VoiceModelStore::in_memory();
        let m = model("u1", 1);
        let id = m.id.clone();
        store.insert(m);
        assert_eq!(
            store.set_active_for_language(&id, "en").unwrap_err(),
            SetActiveError::NotApproved
        );
    }

    #[test]
    fn assigning_unknown_model_is_not_found() {
        let store = VoiceModelStore::in_memory();
        assert_eq!(
            store.set_active_for_language("ghost", "en").unwrap_err(),
            SetActiveError::NotFound
        );
    }

    #[test]
    fn single_to_single_becomes_active_and_locks_to_that_language() {
        let store = VoiceModelStore::in_memory();
        let m = model_with_mode("u1", 1, LanguageMode::SingleToSingle);
        let id = m.id.clone();
        store.insert(m);
        store.set_status(&id, VoiceModelStatus::Approved);

        let updated = store.set_active_for_language(&id, "de").unwrap();
        assert_eq!(updated.target_languages, vec!["de".to_string()]);
        assert_eq!(store.active_model_for("u1", "de").unwrap().id, id);
        assert!(store.active_model_for("u1", "fr").is_none());

        // Aynı dile tekrar atama idempotent kabul edilir.
        assert!(store.set_active_for_language(&id, "de").is_ok());
        // Farklı bir dile atamaya çalışmak reddedilir (model tek hedefe kilitli).
        assert_eq!(
            store.set_active_for_language(&id, "fr").unwrap_err(),
            SetActiveError::LanguageModeConflict
        );
    }

    #[test]
    fn single_to_multi_can_be_assigned_to_several_languages() {
        let store = VoiceModelStore::in_memory();
        let m = model_with_mode("u1", 1, LanguageMode::SingleToMulti);
        let id = m.id.clone();
        store.insert(m);
        store.set_status(&id, VoiceModelStatus::Approved);

        store.set_active_for_language(&id, "de").unwrap();
        let updated = store.set_active_for_language(&id, "fr").unwrap();
        assert_eq!(updated.target_languages, vec!["de".to_string(), "fr".to_string()]);
        assert_eq!(store.active_model_for("u1", "de").unwrap().id, id);
        assert_eq!(store.active_model_for("u1", "fr").unwrap().id, id);
    }

    #[test]
    fn active_model_for_ignores_models_rejected_after_assignment() {
        let store = VoiceModelStore::in_memory();
        let m = model_with_mode("u1", 1, LanguageMode::SingleToSingle);
        let id = m.id.clone();
        store.insert(m);
        store.set_status(&id, VoiceModelStatus::Approved);
        store.set_active_for_language(&id, "de").unwrap();
        assert!(store.active_model_for("u1", "de").is_some());

        // Admin sonradan reddederse dispatch artık bu modeli görmemeli.
        store.set_status(&id, VoiceModelStatus::Rejected);
        assert!(store.active_model_for("u1", "de").is_none());
    }

    #[test]
    fn active_assignment_is_scoped_to_owner_and_persists() {
        let path = std::env::temp_dir().join(format!(
            "sd-voice-models-active-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_file(&path);

        let id = {
            let store = VoiceModelStore::with_file(&path).unwrap();
            let m = model_with_mode("u1", 1, LanguageMode::SingleToSingle);
            let id = m.id.clone();
            store.insert(m);
            store.set_status(&id, VoiceModelStatus::Approved);
            store.set_active_for_language(&id, "de").unwrap();
            id
        };

        let reloaded = VoiceModelStore::with_file(&path).unwrap();
        assert_eq!(reloaded.active_model_for("u1", "de").unwrap().id, id);
        // Başka bir kullanıcı için hiç atama yok.
        assert!(reloaded.active_model_for("u2", "de").is_none());

        let _ = std::fs::remove_file(&path);
    }
}
