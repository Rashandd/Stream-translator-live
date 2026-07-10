//! Yayıncı kullanıcı sistemi (1.4) — signup, login, email 2FA, şifre sıfırlama.
//!
//! Depolama diğer store'larla aynı deseni izler: bellek içi kaynak-of-truth,
//! `USERS_FILE` verilirse AES-256-GCM şifreli dosya kalıcılığı, `DATABASE_URL`
//! varsa `users` tablosuna best-effort write-behind (dosya asıl, DB yansıma —
//! Stripe webhook eşleşmesi `users.stripe_customer_id` üzerinden DB'de çalışır).
//!
//! Login akışı `auth.email_2fa` config'ine bağlıdır (restart'sız değişir):
//! - `auto` (varsayılan): SMTP panelden yapılandırıldıysa 6 haneli kod maillenir,
//!   yapılandırılmadıysa (dev) 2FA atlanır ve token doğrudan döner.
//! - `required`: SMTP yoksa login 503 — üretim sıkılaştırması.
//! - `off`: 2FA hiç yapılmaz.
//!
//! `POST /api/auth/login` tek uçtur: önce env-bootstrap admin (değişmedi),
//! sonra kullanıcı deposu denenir. Şifreler argon2id PHC olarak saklanır ve
//! hiçbir yerde loglanmaz; 2FA kodları da loglanmaz.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant};

use argon2::password_hash::{PasswordHash, SaltString};
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use axum::extract::{Path as RoutePath, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::auth::authz::AdminUser;
use crate::auth::jwt::Claims;
use crate::auth::session::{create_browser_session, SessionPrincipal};
use crate::crypto::Cipher;
use crate::logging::{LogLevel, Logger};
use crate::plans::PlanRegistry;
use crate::rate_limit::client_key;
use crate::state::AppState;
use std::sync::Arc;

/// Yayıncı oturum süresi (saniye) — panel token'ı.
const USER_TOKEN_TTL_SECS: i64 = 24 * 3600;
/// 2FA / sıfırlama kodu uzunluğu.
pub const CODE_LEN: usize = 6;
/// Kod geçerlilik süresi.
const CODE_TTL: Duration = Duration::from_secs(600);
/// Aynı e-postaya yeni kod göndermeden önce beklenen süre (mail spam koruması).
const CODE_REISSUE_AFTER: Duration = Duration::from_secs(30);
/// Kod başına maksimum yanlış deneme.
const MAX_CODE_ATTEMPTS: u8 = 5;
/// Asgari şifre uzunluğu (signup formuyla senkron).
const MIN_PASSWORD_LEN: usize = 8;

/// Tek bir yayıncı kaydı. Şifre argon2id PHC dizesi olarak tutulur.
///
/// `password_hash` OAuth-only hesaplarda (Google/Twitch login,
/// `oauth_provider`+`oauth_external_id` doluysa) `None`'dur — hesabın şifresi
/// yok, sadece sağlayıcı kimliğiyle giriş yapılır.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRecord {
    pub id: String,
    pub email: String,
    pub password_hash: Option<String>,
    pub plan: String,
    pub email_verified: bool,
    pub created_at_ms: i64,
    #[serde(default)]
    pub oauth_provider: Option<String>,
    #[serde(default)]
    pub oauth_external_id: Option<String>,
}

/// Kullanıcı deposu hataları.
#[derive(Debug, thiserror::Error)]
pub enum UserError {
    #[error("email already registered")]
    EmailTaken,
    #[error("this provider account is already linked to a different user")]
    OAuthIdentityTaken,
    #[error("invalid email address")]
    InvalidEmail,
    #[error("password must be at least {MIN_PASSWORD_LEN} characters")]
    WeakPassword,
    #[error("user not found")]
    NotFound,
    #[error("user store io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("user store crypto error: {0}")]
    Crypto(String),
    #[error("user store serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("internal user store error: {0}")]
    Internal(String),
}

/// E-posta (lowercase) → kayıt deposu; opsiyonel şifreli dosya + DB yansıması.
pub struct UserStore {
    entries: RwLock<HashMap<String, UserRecord>>,
    persist: Option<(PathBuf, Cipher)>,
    db: Option<sqlx::PgPool>,
}

impl UserStore {
    /// Kalıcılıksız depo (dev/test).
    pub fn in_memory() -> Self {
        UserStore {
            entries: RwLock::new(HashMap::new()),
            persist: None,
            db: None,
        }
    }

    /// Şifreli dosya kalıcılığıyla kurar; dosya varsa kayıtları yükler.
    pub fn with_file(path: impl Into<PathBuf>, key: &[u8]) -> Result<Self, UserError> {
        let path = path.into();
        let cipher = Cipher::new(key).map_err(|e| UserError::Crypto(e.to_string()))?;
        let entries = load_entries(&path, &cipher)?;
        Ok(UserStore {
            entries: RwLock::new(entries),
            persist: Some((path, cipher)),
            db: None,
        })
    }

    /// DB yansımasını bağlar (best-effort write-behind).
    pub fn with_db(mut self, db: Option<sqlx::PgPool>) -> Self {
        self.db = db;
        self
    }

    /// Yeni kullanıcı oluşturur. E-posta benzersizdir (case-insensitive).
    pub fn signup(&self, email: &str, password: &str, plan: &str) -> Result<UserRecord, UserError> {
        let email = email.trim();
        if !email_looks_valid(email) {
            return Err(UserError::InvalidEmail);
        }
        if password.len() < MIN_PASSWORD_LEN {
            return Err(UserError::WeakPassword);
        }
        let record = UserRecord {
            id: uuid::Uuid::new_v4().to_string(),
            email: email.to_string(),
            password_hash: Some(hash_password(password)?),
            plan: plan.to_string(),
            email_verified: false,
            created_at_ms: chrono::Utc::now().timestamp_millis(),
            oauth_provider: None,
            oauth_external_id: None,
        };
        {
            let mut map = self
                .entries
                .write()
                .map_err(|_| UserError::Internal("user lock poisoned".into()))?;
            if map.contains_key(&email.to_lowercase()) {
                return Err(UserError::EmailTaken);
            }
            map.insert(email.to_lowercase(), record.clone());
        }
        self.save()?;
        self.persist_db(record.clone());
        Ok(record)
    }

    /// OAuth (Google/Twitch) ile giriş yapan, şifresi olmayan yeni bir hesap
    /// oluşturur. Çağıran taraf (`oauth_login`) bu hesabı açmadan önce ne bu
    /// (provider, external_id) kimliğinin ne de e-postanın kayıtlı
    /// olmadığını kontrol etmiş olmalı — burada aynı yazma kilidi altında
    /// tekrar doğrulanır (signup() ile aynı desen), böylece aynı anda gelen
    /// iki istek aynı yeni kimlik için iki hesap açamaz.
    pub fn create_oauth_user(
        &self,
        provider: &str,
        external_id: &str,
        email: &str,
        plan: &str,
    ) -> Result<UserRecord, UserError> {
        let email = email.trim();
        if !email_looks_valid(email) {
            return Err(UserError::InvalidEmail);
        }
        let record = UserRecord {
            id: uuid::Uuid::new_v4().to_string(),
            email: email.to_string(),
            password_hash: None,
            plan: plan.to_string(),
            // Login akışının kendisi provider hesabına erişimi kanıtlıyor;
            // e-posta çakışması ayrı bir kontrol (bkz. oauth_login modülü).
            email_verified: true,
            created_at_ms: chrono::Utc::now().timestamp_millis(),
            oauth_provider: Some(provider.to_string()),
            oauth_external_id: Some(external_id.to_string()),
        };
        {
            let mut map = self
                .entries
                .write()
                .map_err(|_| UserError::Internal("user lock poisoned".into()))?;
            if map.contains_key(&email.to_lowercase()) {
                return Err(UserError::EmailTaken);
            }
            let identity_taken = map.values().any(|r| {
                r.oauth_provider.as_deref() == Some(provider)
                    && r.oauth_external_id.as_deref() == Some(external_id)
            });
            if identity_taken {
                return Err(UserError::OAuthIdentityTaken);
            }
            map.insert(email.to_lowercase(), record.clone());
        }
        self.save()?;
        self.persist_db(record.clone());
        Ok(record)
    }

    /// Sağlayıcı kimliğiyle (ör. Google `sub`, Twitch kullanıcı ID'si) kayıt
    /// arar. Küçük kullanıcı sayısında (Faz 1) doğrusal tarama yeterli —
    /// e-posta anahtarlı map'e ikinci bir index eklemeye değmez.
    pub fn find_by_oauth(&self, provider: &str, external_id: &str) -> Option<UserRecord> {
        self.entries.read().ok().and_then(|m| {
            m.values()
                .find(|r| {
                    r.oauth_provider.as_deref() == Some(provider)
                        && r.oauth_external_id.as_deref() == Some(external_id)
                })
                .cloned()
        })
    }

    /// E-posta + şifre doğrulaması. Kullanıcı yokken de, ya da hesabın
    /// şifresi yokken (OAuth-only) de argon2 koşulur — zamanlamadan hesap
    /// varlığı/türü sızmaz.
    pub fn verify_login(&self, email: &str, password: &str) -> Option<UserRecord> {
        let record = self
            .entries
            .read()
            .ok()
            .and_then(|m| m.get(&email.trim().to_lowercase()).cloned());
        match record {
            Some(record) if record.password_hash.as_deref().is_some_and(|hash| verify_password(hash, password)) => {
                Some(record)
            }
            Some(_) => {
                let _ = verify_password(dummy_hash(), password);
                None
            }
            None => {
                let _ = verify_password(dummy_hash(), password);
                None
            }
        }
    }

    /// Kayıt okur (varsa).
    pub fn get(&self, email: &str) -> Option<UserRecord> {
        self.entries
            .read()
            .ok()
            .and_then(|m| m.get(&email.trim().to_lowercase()).cloned())
    }

    /// Tüm kullanıcılar, e-postaya göre sıralı (admin liste ekranı için).
    pub fn list(&self) -> Vec<UserRecord> {
        let mut records: Vec<UserRecord> = self
            .entries
            .read()
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default();
        records.sort_by(|a, b| a.email.cmp(&b.email));
        records
    }

    /// Id'ye göre tek kullanıcı (admin detay ekranı için) — `entries` e-posta
    /// anahtarlı olduğundan tarama gerekir; kullanıcı sayısı Faz 1 ölçeğinde
    /// bunu haklı çıkarır.
    pub fn get_by_id(&self, id: &str) -> Option<UserRecord> {
        self.entries.read().ok()?.values().find(|r| r.id == id).cloned()
    }

    /// Admin panelden bir kullanıcının planını değiştirir (restart gerekmez;
    /// `set_password`/`mark_verified` ile aynı yaz-sonra-persist deseni).
    pub fn set_plan_by_id(&self, id: &str, plan: &str) -> Result<UserRecord, UserError> {
        let updated = {
            let mut map = self
                .entries
                .write()
                .map_err(|_| UserError::Internal("user lock poisoned".into()))?;
            map.values_mut().find(|r| r.id == id).map(|record| {
                record.plan = plan.to_string();
                record.clone()
            })
        };
        match updated {
            Some(record) => {
                self.save()?;
                self.persist_db(record.clone());
                Ok(record)
            }
            None => Err(UserError::NotFound),
        }
    }

    /// E-postayı doğrulanmış işaretler (ilk başarılı e-posta 2FA'sı mailbox
    /// sahipliğini kanıtlar).
    pub fn mark_verified(&self, email: &str) -> Result<(), UserError> {
        let updated = self.update(email, |r| r.email_verified = true)?;
        if let Some(record) = updated {
            self.persist_db(record);
        }
        Ok(())
    }

    /// Şifreyi değiştirir (sıfırlama akışı).
    pub fn set_password(&self, email: &str, new_password: &str) -> Result<(), UserError> {
        if new_password.len() < MIN_PASSWORD_LEN {
            return Err(UserError::WeakPassword);
        }
        let hash = hash_password(new_password)?;
        let updated = self.update(email, |r| r.password_hash = Some(hash.clone()))?;
        match updated {
            Some(record) => {
                self.persist_db(record);
                Ok(())
            }
            None => Err(UserError::Internal("user not found".into())),
        }
    }

    fn update(
        &self,
        email: &str,
        apply: impl FnOnce(&mut UserRecord),
    ) -> Result<Option<UserRecord>, UserError> {
        let updated = {
            let mut map = self
                .entries
                .write()
                .map_err(|_| UserError::Internal("user lock poisoned".into()))?;
            map.get_mut(&email.trim().to_lowercase()).map(|record| {
                apply(record);
                record.clone()
            })
        };
        if updated.is_some() {
            self.save()?;
        }
        Ok(updated)
    }

    fn save(&self) -> Result<(), UserError> {
        let Some((path, cipher)) = &self.persist else {
            return Ok(());
        };
        let snapshot: HashMap<String, UserRecord> = self
            .entries
            .read()
            .map_err(|_| UserError::Internal("user lock poisoned".into()))?
            .clone();
        let json = serde_json::to_string(&snapshot)?;
        let sealed = cipher
            .encrypt_str(&json)
            .map_err(|e| UserError::Crypto(e.to_string()))?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, sealed)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// DB yansıması — pool varsa arka planda upsert; hata pipeline'ı düşürmez.
    fn persist_db(&self, record: UserRecord) {
        let Some(pool) = self.db.clone() else {
            return;
        };
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            tracing::warn!("user db persist skipped: no tokio runtime");
            return;
        };
        handle.spawn(async move {
            if let Err(err) = upsert_user(&pool, &record).await {
                tracing::warn!(error = %err, user = %record.id, "user db persist failed");
            }
        });
    }
}

/// `users` tablosuna upsert. Plan satırı yoksa asgari değerlerle eklenir —
/// Faz 1'de kaynak-of-truth PlanRegistry/panel'dir, DB satırı FK içindir.
async fn upsert_user(pool: &sqlx::PgPool, record: &UserRecord) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO plans (name, price_cents, monthly_hours, max_languages, max_platforms)
         VALUES ($1, 0, 5, 1, 1) ON CONFLICT (name) DO NOTHING",
    )
    .bind(&record.plan)
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT INTO users (id, email, password_hash, plan_id, role, email_verified, oauth_provider, oauth_external_id)
         VALUES ($1::uuid, $2, $3, (SELECT id FROM plans WHERE name = $4), 'broadcaster', $5, $6, $7)
         ON CONFLICT (email) DO UPDATE SET
             password_hash = EXCLUDED.password_hash,
             plan_id = EXCLUDED.plan_id,
             email_verified = EXCLUDED.email_verified,
             oauth_provider = EXCLUDED.oauth_provider,
             oauth_external_id = EXCLUDED.oauth_external_id,
             updated_at = NOW()",
    )
    .bind(&record.id)
    .bind(record.email.to_lowercase())
    .bind(&record.password_hash)
    .bind(&record.plan)
    .bind(record.email_verified)
    .bind(&record.oauth_provider)
    .bind(&record.oauth_external_id)
    .execute(pool)
    .await?;
    Ok(())
}

fn load_entries(path: &Path, cipher: &Cipher) -> Result<HashMap<String, UserRecord>, UserError> {
    let sealed = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(e) => return Err(UserError::Io(e)),
    };
    let json = cipher
        .decrypt_str(sealed.trim())
        .map_err(|e| UserError::Crypto(format!("users file unreadable: {e}")))?;
    Ok(serde_json::from_str(&json)?)
}

/// Kaba e-posta doğrulaması — tam RFC değil, bariz bozukları eler.
fn email_looks_valid(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !email.contains(char::is_whitespace)
        && email.len() <= 254
}

fn hash_password(password: &str) -> Result<String, UserError> {
    let mut salt_bytes = [0u8; 16];
    getrandom::getrandom(&mut salt_bytes).map_err(|_| UserError::Internal("rng failure".into()))?;
    let salt =
        SaltString::encode_b64(&salt_bytes).map_err(|e| UserError::Internal(e.to_string()))?;
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| UserError::Internal(e.to_string()))?
        .to_string())
}

fn verify_password(phc_hash: &str, password: &str) -> bool {
    PasswordHash::new(phc_hash)
        .map(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
        .unwrap_or(false)
}

/// Bilinmeyen e-posta denemelerinde zamanlamayı eşitlemek için sabit hash.
fn dummy_hash() -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| hash_password("timing-equalizer-dummy").unwrap_or_default())
}

// ---------------------------------------------------------------------------
// 2FA / sıfırlama kodları
// ---------------------------------------------------------------------------

/// Kod amacı — login 2FA'sı ile şifre sıfırlama ayrı ad alanlarıdır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CodePurpose {
    Login,
    Reset,
}

struct PendingCode {
    code: String,
    created: Instant,
    attempts: u8,
}

/// 2FA kod hataları.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TwoFactorError {
    #[error("invalid or expired code")]
    Invalid,
    #[error("too many attempts — request a new code")]
    TooManyAttempts,
    #[error("a code was sent recently — wait before requesting another")]
    TooSoon,
    #[error("internal 2fa error: {0}")]
    Internal(&'static str),
}

/// (email, amaç) → bekleyen kod deposu. Bellek içi ve kısa ömürlü —
/// restart bekleyen kodları düşürür (kullanıcı yeniden login olur).
pub struct TwoFactorStore {
    codes: Mutex<HashMap<(String, CodePurpose), PendingCode>>,
    ttl: Duration,
    reissue_after: Duration,
}

impl TwoFactorStore {
    pub fn new() -> Self {
        Self::with_limits(CODE_TTL, CODE_REISSUE_AFTER)
    }

    /// Test için özel süreler.
    pub fn with_limits(ttl: Duration, reissue_after: Duration) -> Self {
        TwoFactorStore {
            codes: Mutex::new(HashMap::new()),
            ttl,
            reissue_after,
        }
    }

    /// Yeni kod üretir (eskisini geçersiz kılar). Çok sık istenirse `TooSoon`.
    pub fn issue(&self, email: &str, purpose: CodePurpose) -> Result<String, TwoFactorError> {
        let key = (email.trim().to_lowercase(), purpose);
        let mut map = self
            .codes
            .lock()
            .map_err(|_| TwoFactorError::Internal("2fa lock poisoned"))?;
        if let Some(pending) = map.get(&key) {
            if pending.created.elapsed() < self.reissue_after {
                return Err(TwoFactorError::TooSoon);
            }
        }
        let code = random_code()?;
        map.insert(
            key,
            PendingCode {
                code: code.clone(),
                created: Instant::now(),
                attempts: 0,
            },
        );
        Ok(code)
    }

    /// Kodu doğrular ve tüketir (tek kullanımlık). Yanlış denemeler sayılır;
    /// limit aşılınca kod iptal edilir.
    pub fn verify(&self, email: &str, purpose: CodePurpose, code: &str) -> Result<(), TwoFactorError> {
        let key = (email.trim().to_lowercase(), purpose);
        let mut map = self
            .codes
            .lock()
            .map_err(|_| TwoFactorError::Internal("2fa lock poisoned"))?;
        let Some(pending) = map.get_mut(&key) else {
            return Err(TwoFactorError::Invalid);
        };
        if pending.created.elapsed() > self.ttl {
            map.remove(&key);
            return Err(TwoFactorError::Invalid);
        }
        if constant_time_eq(pending.code.as_bytes(), code.trim().as_bytes()) {
            map.remove(&key);
            return Ok(());
        }
        pending.attempts += 1;
        if pending.attempts >= MAX_CODE_ATTEMPTS {
            map.remove(&key);
            return Err(TwoFactorError::TooManyAttempts);
        }
        Err(TwoFactorError::Invalid)
    }

    /// Bekleyen kodu iptal eder (mail gönderilemediyse).
    pub fn cancel(&self, email: &str, purpose: CodePurpose) {
        if let Ok(mut map) = self.codes.lock() {
            map.remove(&(email.trim().to_lowercase(), purpose));
        }
    }

    /// Kod geçerlilik süresi (saniye) — frontend geri sayımı.
    pub fn ttl_secs(&self) -> u64 {
        self.ttl.as_secs()
    }
}

impl Default for TwoFactorStore {
    fn default() -> Self {
        Self::new()
    }
}

fn random_code() -> Result<String, TwoFactorError> {
    let mut bytes = [0u8; CODE_LEN];
    getrandom::getrandom(&mut bytes).map_err(|_| TwoFactorError::Internal("rng failure"))?;
    Ok(bytes.iter().map(|b| char::from(b'0' + (b % 10))).collect())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

// ---------------------------------------------------------------------------
// HTTP handler'ları
// ---------------------------------------------------------------------------

/// `POST /api/auth/signup` gövdesi.
#[derive(Debug, Deserialize)]
pub struct SignupRequest {
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub legal_terms_accepted: bool,
    #[serde(default)]
    pub marketing_consent: bool,
}

/// Basit onay yanıtı.
#[derive(Debug, Serialize)]
pub struct OkResponse {
    pub ok: bool,
}

/// `POST /api/auth/signup` — ücretsiz planla hesap açar. Frontend başarıdan
/// sonra normal login akışını çağırır (2FA kararını login verir).
pub async fn signup(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SignupRequest>,
) -> Result<(StatusCode, Json<OkResponse>), (StatusCode, String)> {
    let signup_limit: u32 = app
        .service_config
        .get("auth.signup.rate_limit_per_min")
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);
    if !app.auth_limiter.check_signup(&client_key(&headers), signup_limit) {
        app.logger.log(
            LogLevel::Warn,
            "auth",
            "signup rate limited",
            None,
            None,
        );
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "too many signup attempts, try again later".to_string(),
        ));
    }

    if !req.legal_terms_accepted {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "required legal terms must be accepted".to_string(),
        ));
    }
    let _marketing_consent = req.marketing_consent;

    let plan = app
        .service_config
        .get("auth.default_plan")
        .unwrap_or_else(|| "free".to_string());
    let record = app.users.signup(&req.email, &req.password, &plan).map_err(|err| {
        let status = match err {
            UserError::EmailTaken => StatusCode::CONFLICT,
            UserError::InvalidEmail | UserError::WeakPassword => StatusCode::UNPROCESSABLE_ENTITY,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, err.to_string())
    })?;
    app.logger.log(
        LogLevel::Info,
        "auth",
        "user signup",
        Some(&record.id),
        None,
    );
    Ok((StatusCode::CREATED, Json(OkResponse { ok: true })))
}

/// `POST /api/auth/login` gövdesi.
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// Login yanıtı: ya doğrudan token, ya 2FA beklemesi.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum LoginResponse {
    Token {
        token: String,
        /// Token bitişi (unix saniye) — frontend süresi dolunca yeniden ister.
        expires_at: i64,
        role: String,
    },
    TwoFactorPending {
        two_factor_required: bool,
        expires_in_secs: u64,
        role: String,
    },
}

/// `POST /api/auth/login` — tek giriş ucu.
///
/// Önce env-bootstrap admin denenir. Admin ve yayıncı aynı e-posta 2FA
/// politikasından geçer; doğrulama tamamlanmadan hiçbir session üretilmez.
/// Sonra kullanıcı deposu:
/// `auth.email_2fa` config'ine göre 6 haneli kod maillenir veya token döner.
/// Kimlik uyuşmazsa 401 — hangi alanın yanlış olduğu söylenmez.
pub async fn login(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<LoginRequest>,
) -> Result<Response, (StatusCode, String)> {
    let login_limit: u32 = app
        .service_config
        .get("auth.login.rate_limit_per_min")
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    if !app.auth_limiter.check_login(&client_key(&headers), login_limit) {
        app.logger.log(LogLevel::Warn, "auth", "login rate limited", None, None);
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "too many login attempts, try again later".to_string(),
        ));
    }

    // 1) Bootstrap admin (varsa).
    if let Some(admin) = app.bootstrap_admin.as_ref() {
        if admin.verify(&req.email, &req.password) {
            let admin_email = admin.email().to_string();
            let principal = SessionPrincipal {
                sub: "admin-bootstrap".to_string(),
                role: "admin".to_string(),
                plan: "streamer".to_string(),
            };
            if let Some(pending) = begin_login_twofactor(
                &app,
                &admin_email,
                "admin-bootstrap",
                "admin",
            )
            .await?
            {
                return Ok(pending);
            }
            return finish_principal_login(&app, principal, &headers).await;
        }
    }

    // 2) Yayıncı deposu.
    let Some(user) = app.users.verify_login(&req.email, &req.password) else {
        app.logger.log(
            LogLevel::Warn,
            "auth",
            &format!("login failed for {}", req.email),
            None,
            None,
        );
        return Err((StatusCode::UNAUTHORIZED, "invalid credentials".to_string()));
    };

    if let Some(pending) = begin_login_twofactor(
        &app,
        &user.email,
        &user.id,
        "broadcaster",
    )
    .await?
    {
        return Ok(pending);
    }
    finish_user_login(&app, &user, &headers).await
}

/// Start the common email challenge for either role. `None` means the policy
/// explicitly disabled 2FA or `auto` mode skipped it because SMTP is not yet
/// configured; no session is issued inside this helper.
async fn begin_login_twofactor(
    app: &AppState,
    email: &str,
    subject: &str,
    role: &str,
) -> Result<Option<Response>, (StatusCode, String)> {
    let mode = app
        .service_config
        .get("auth.email_2fa")
        .unwrap_or_else(|| "auto".to_string());
    if mode == "off" {
        return Ok(None);
    }

    let pending_response = || {
        Json(LoginResponse::TwoFactorPending {
            two_factor_required: true,
            expires_in_secs: app.twofactor.ttl_secs(),
            role: role.to_string(),
        })
        .into_response()
    };
    let code = match app.twofactor.issue(email, CodePurpose::Login) {
        Ok(code) => code,
        Err(TwoFactorError::TooSoon) => {
            return Ok(Some(pending_response()));
        }
        Err(err) => return Err((StatusCode::INTERNAL_SERVER_ERROR, err.to_string())),
    };
    let mail_result = crate::mail::send(
        app.secrets.as_ref(),
        &app.service_config,
        email,
        "Your Strovak verification code",
        &format!("Your sign-in verification code is: {code}\n\nIt expires in 10 minutes. If you didn't try to sign in, you can ignore this email."),
    )
    .await;

    match mail_result {
        Ok(()) => {
            app.logger.log(
                LogLevel::Info,
                "auth",
                "2fa code sent",
                Some(subject),
                None,
            );
            Ok(Some(pending_response()))
        }
        Err(crate::mail::MailError::NotConfigured) if mode != "required" => {
            app.twofactor.cancel(email, CodePurpose::Login);
            app.logger.log(
                LogLevel::Warn,
                "auth",
                "smtp not configured — 2fa skipped (auth.email_2fa=auto)",
                Some(subject),
                None,
            );
            Ok(None)
        }
        Err(crate::mail::MailError::NotConfigured) => {
            app.twofactor.cancel(email, CodePurpose::Login);
            Err((
                StatusCode::SERVICE_UNAVAILABLE,
                "email 2fa required but smtp is not configured".to_string(),
            ))
        }
        Err(err) => {
            app.twofactor.cancel(email, CodePurpose::Login);
            app.logger.log(
                LogLevel::Error,
                "auth",
                &format!("2fa mail send failed: {err}"),
                Some(subject),
                None,
            );
            Err((
                StatusCode::BAD_GATEWAY,
                "could not send verification code".to_string(),
            ))
        }
    }
}

/// E-posta/şifre ve OAuth login akışlarının ikisi de aynı JWT şeklini üretir
/// — `oauth_login` bunu doğrudan reuse eder (bkz. `crate::auth::oauth_login`).
pub(crate) fn issue_user_token(
    app: &AppState,
    user: &UserRecord,
) -> Result<LoginResponse, (StatusCode, String)> {
    let expires_at = chrono::Utc::now().timestamp() + USER_TOKEN_TTL_SECS;
    let claims = Claims {
        sub: user.id.clone(),
        role: "broadcaster".to_string(),
        plan: user.plan.clone(),
        exp: expires_at,
    };
    let token = app
        .jwt
        .issue(&claims)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    app.logger.log(
        LogLevel::Info,
        "auth",
        "user login succeeded",
        Some(&user.id),
        None,
    );
    Ok(LoginResponse::Token {
        token,
        expires_at,
        role: "broadcaster".to_string(),
    })
}

fn user_principal(user: &UserRecord) -> SessionPrincipal {
    SessionPrincipal {
        sub: user.id.clone(),
        role: "broadcaster".to_string(),
        plan: user.plan.clone(),
    }
}

async fn finish_user_login(
    app: &AppState,
    user: &UserRecord,
    headers: &HeaderMap,
) -> Result<Response, (StatusCode, String)> {
    finish_principal_login(app, user_principal(user), headers).await
}

async fn finish_principal_login(
    app: &AppState,
    principal: SessionPrincipal,
    headers: &HeaderMap,
) -> Result<Response, (StatusCode, String)> {
    // Browser requests always carry Origin (a forbidden request header that
    // application JavaScript cannot suppress). CLI/SDK callers keep the
    // legacy bearer response during migration; browser JavaScript never sees
    // an access token in JSON.
    if headers.contains_key(axum::http::header::ORIGIN) {
        return create_browser_session(app, principal).await;
    }

    let expires_at = chrono::Utc::now().timestamp() + USER_TOKEN_TTL_SECS;
    let claims = Claims {
        sub: principal.sub,
        role: principal.role.clone(),
        plan: principal.plan,
        exp: expires_at,
    };
    let token = app
        .jwt
        .issue(&claims)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(LoginResponse::Token {
        token,
        expires_at,
        role: principal.role,
    })
    .into_response())
}

/// `POST /api/auth/2fa` gövdesi.
#[derive(Debug, Deserialize)]
pub struct TwoFactorRequest {
    pub email: String,
    pub code: String,
}

/// `POST /api/auth/2fa` — maillenen kodu doğrular, yayıncı token'ı döner.
/// Başarılı doğrulama e-postayı da doğrulanmış işaretler.
pub async fn twofa_verify(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<TwoFactorRequest>,
) -> Result<Response, (StatusCode, String)> {
    verify_code_or_status(&app, &req.email, CodePurpose::Login, &req.code)?;
    if app
        .bootstrap_admin
        .as_ref()
        .as_ref()
        .is_some_and(|admin| admin.matches_email(&req.email))
    {
        return finish_principal_login(
            &app,
            SessionPrincipal {
                sub: "admin-bootstrap".to_string(),
                role: "admin".to_string(),
                plan: "streamer".to_string(),
            },
            &headers,
        )
        .await;
    }
    let Some(user) = app.users.get(&req.email) else {
        return Err((StatusCode::UNAUTHORIZED, "invalid or expired code".to_string()));
    };
    if let Err(err) = app.users.mark_verified(&user.email) {
        tracing::warn!(error = %err, "mark_verified failed");
    }
    finish_user_login(&app, &user, &headers).await
}

fn verify_code_or_status(
    app: &AppState,
    email: &str,
    purpose: CodePurpose,
    code: &str,
) -> Result<(), (StatusCode, String)> {
    app.twofactor.verify(email, purpose, code).map_err(|err| {
        let status = match err {
            TwoFactorError::TooManyAttempts => StatusCode::TOO_MANY_REQUESTS,
            TwoFactorError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            _ => StatusCode::UNAUTHORIZED,
        };
        (status, err.to_string())
    })
}

/// `POST /api/auth/reset/request` gövdesi.
#[derive(Debug, Deserialize)]
pub struct ResetRequest {
    pub email: String,
}

/// `POST /api/auth/reset/request` — hesap varsa sıfırlama kodu mailler.
/// Yanıt her durumda 200 — hesap varlığı sızdırılmaz.
pub async fn reset_request(
    State(app): State<AppState>,
    Json(req): Json<ResetRequest>,
) -> Json<OkResponse> {
    let Some(user) = app.users.get(&req.email) else {
        return Json(OkResponse { ok: true });
    };
    let code = match app.twofactor.issue(&user.email, CodePurpose::Reset) {
        Ok(code) => code,
        Err(_) => return Json(OkResponse { ok: true }), // TooSoon dahil — sessiz
    };
    let result = crate::mail::send(
        app.secrets.as_ref(),
        &app.service_config,
        &user.email,
        "Your Strovak password reset code",
        &format!("Your password reset code is: {code}\n\nIt expires in 10 minutes. If you didn't request a reset, you can ignore this email."),
    )
    .await;
    match result {
        Ok(()) => app.logger.log(
            LogLevel::Info,
            "auth",
            "password reset code sent",
            Some(&user.id),
            None,
        ),
        Err(err) => {
            app.twofactor.cancel(&user.email, CodePurpose::Reset);
            app.logger.log(
                LogLevel::Warn,
                "auth",
                &format!("password reset mail failed: {err}"),
                Some(&user.id),
                None,
            );
        }
    }
    Json(OkResponse { ok: true })
}

/// `POST /api/auth/reset/confirm` gövdesi.
#[derive(Debug, Deserialize)]
pub struct ResetConfirmRequest {
    pub email: String,
    pub code: String,
    pub new_password: String,
}

/// `POST /api/auth/reset/confirm` — kodu doğrular, şifreyi değiştirir.
pub async fn reset_confirm(
    State(app): State<AppState>,
    Json(req): Json<ResetConfirmRequest>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    // Şifre kod tüketiminden ÖNCE doğrulanır — zayıf şifre denemesi tek
    // kullanımlık kodu yakmamalı (kullanıcı aynı kodla tekrar dener).
    if req.new_password.len() < MIN_PASSWORD_LEN {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            UserError::WeakPassword.to_string(),
        ));
    }
    verify_code_or_status(&app, &req.email, CodePurpose::Reset, &req.code)?;
    app.users
        .set_password(&req.email, &req.new_password)
        .map_err(|err| match err {
            UserError::WeakPassword => (StatusCode::UNPROCESSABLE_ENTITY, err.to_string()),
            other => (StatusCode::INTERNAL_SERVER_ERROR, other.to_string()),
        })?;
    if let Some(user) = app.users.get(&req.email) {
        app.logger.log(
            LogLevel::Info,
            "auth",
            "password reset completed",
            Some(&user.id),
            None,
        );
    }
    Ok(Json(OkResponse { ok: true }))
}

// ---------------------------------------------------------------------------
// Admin kullanıcı yönetimi (Faz 1.14) — liste, detay, plan değiştirme.
// Parola hash'i hiçbir admin yanıtında dönmez.
// ---------------------------------------------------------------------------

/// Admin liste/detay yanıtı — `UserRecord`'un parola içermeyen izdüşümü.
#[derive(Debug, Serialize)]
pub struct AdminUserSummary {
    pub id: String,
    pub email: String,
    pub plan: String,
    pub email_verified: bool,
    pub created_at_ms: i64,
    pub oauth_provider: Option<String>,
}

impl From<UserRecord> for AdminUserSummary {
    fn from(record: UserRecord) -> Self {
        AdminUserSummary {
            id: record.id,
            email: record.email,
            plan: record.plan,
            email_verified: record.email_verified,
            created_at_ms: record.created_at_ms,
            oauth_provider: record.oauth_provider,
        }
    }
}

/// `GET /admin/users` sorgu parametreleri.
#[derive(Debug, Deserialize)]
pub struct AdminUsersQuery {
    /// E-postada alt dize araması (case-insensitive).
    pub q: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

const ADMIN_USERS_DEFAULT_LIMIT: usize = 50;
const ADMIN_USERS_MAX_LIMIT: usize = 200;

/// `GET /admin/users` (sadece admin) — arama + basit sayfalama.
pub async fn admin_list_users(
    _admin: AdminUser,
    State(users): State<Arc<UserStore>>,
    Query(query): Query<AdminUsersQuery>,
) -> Json<Vec<AdminUserSummary>> {
    let mut records = users.list();
    if let Some(needle) = query.q.as_deref().map(str::to_lowercase).filter(|q| !q.is_empty()) {
        records.retain(|r| r.email.to_lowercase().contains(&needle));
    }
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(ADMIN_USERS_DEFAULT_LIMIT).min(ADMIN_USERS_MAX_LIMIT);
    let page = records
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(AdminUserSummary::from)
        .collect();
    Json(page)
}

/// `GET /admin/users/{id}` (sadece admin).
pub async fn admin_get_user(
    _admin: AdminUser,
    State(users): State<Arc<UserStore>>,
    RoutePath(id): RoutePath<String>,
) -> Result<Json<AdminUserSummary>, StatusCode> {
    users
        .get_by_id(&id)
        .map(|r| Json(r.into()))
        .ok_or(StatusCode::NOT_FOUND)
}

/// `PUT /admin/users/{id}/plan` gövdesi.
#[derive(Debug, Deserialize)]
pub struct AdminSetPlanRequest {
    pub plan: String,
}

/// `PUT /admin/users/{id}/plan` (sadece admin) — plan adı `PlanRegistry`'de
/// kayıtlı olmalı, aksi halde 404 (yazım hatasıyla hayalet plan atanmasın).
pub async fn admin_set_user_plan(
    admin: AdminUser,
    State(users): State<Arc<UserStore>>,
    State(plans): State<Arc<PlanRegistry>>,
    State(logger): State<Arc<Logger>>,
    RoutePath(id): RoutePath<String>,
    Json(req): Json<AdminSetPlanRequest>,
) -> Result<Json<AdminUserSummary>, (StatusCode, String)> {
    if plans.get(&req.plan).is_none() {
        return Err((StatusCode::NOT_FOUND, format!("unknown plan '{}'", req.plan)));
    }
    let record = users.set_plan_by_id(&id, &req.plan).map_err(|err| match err {
        UserError::NotFound => (StatusCode::NOT_FOUND, err.to_string()),
        other => (StatusCode::INTERNAL_SERVER_ERROR, other.to_string()),
    })?;
    logger.log(
        LogLevel::Info,
        "admin",
        &format!("plan changed for {} -> {}", record.email, req.plan),
        Some(&admin.claims.sub),
        None,
    );
    Ok(Json(record.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signup_login_roundtrip_and_duplicate_rejected() {
        let store = UserStore::in_memory();
        let user = store.signup("Streamer@Example.com", "hunter2-strong", "free").unwrap();
        assert_eq!(user.plan, "free");
        assert!(!user.email_verified);

        // Aynı e-posta (case-insensitive) reddedilir.
        assert!(matches!(
            store.signup("streamer@example.com", "other-pass-123", "free"),
            Err(UserError::EmailTaken)
        ));

        // Doğru şifre + case-insensitive e-posta girer, yanlış şifre girmez.
        assert!(store.verify_login("streamer@example.com", "hunter2-strong").is_some());
        assert!(store.verify_login("STREAMER@EXAMPLE.COM", "hunter2-strong").is_some());
        assert!(store.verify_login("streamer@example.com", "wrong").is_none());
        assert!(store.verify_login("nobody@example.com", "hunter2-strong").is_none());
    }

    #[test]
    fn list_and_get_by_id_expose_seeded_users() {
        let store = UserStore::in_memory();
        let a = store.signup("a@example.com", "a-pass-1234", "free").unwrap();
        let b = store.signup("b@example.com", "b-pass-1234", "starter").unwrap();

        let listed = store.list();
        assert_eq!(listed.len(), 2);
        // E-postaya göre sıralı.
        assert_eq!(listed[0].email, "a@example.com");
        assert_eq!(listed[1].email, "b@example.com");

        assert_eq!(store.get_by_id(&a.id).unwrap().email, "a@example.com");
        assert_eq!(store.get_by_id(&b.id).unwrap().plan, "starter");
        assert!(store.get_by_id("no-such-id").is_none());
    }

    #[test]
    fn set_plan_by_id_updates_the_record_and_rejects_unknown_ids() {
        let store = UserStore::in_memory();
        let user = store.signup("plan-change@example.com", "strong-pass-1", "free").unwrap();

        let updated = store.set_plan_by_id(&user.id, "pro").unwrap();
        assert_eq!(updated.plan, "pro");
        assert_eq!(store.get("plan-change@example.com").unwrap().plan, "pro");

        assert!(matches!(store.set_plan_by_id("ghost-id", "pro"), Err(UserError::NotFound)));
    }

    #[test]
    fn create_oauth_user_has_no_password_and_is_pre_verified() {
        let store = UserStore::in_memory();
        let user = store
            .create_oauth_user("google", "g-123", "streamer@example.com", "free")
            .unwrap();
        assert!(user.password_hash.is_none());
        assert!(user.email_verified);
        assert_eq!(user.oauth_provider.as_deref(), Some("google"));
        assert_eq!(user.oauth_external_id.as_deref(), Some("g-123"));

        // No password to check against — any password attempt fails, same
        // as an unknown account (verified by the timing-equalizer test below).
        assert!(store.verify_login("streamer@example.com", "anything").is_none());
    }

    #[test]
    fn find_by_oauth_locates_the_linked_account() {
        let store = UserStore::in_memory();
        store
            .create_oauth_user("twitch", "t-456", "streamer@example.com", "free")
            .unwrap();

        let found = store.find_by_oauth("twitch", "t-456").expect("found");
        assert_eq!(found.email, "streamer@example.com");
        assert!(store.find_by_oauth("twitch", "wrong-id").is_none());
        assert!(store.find_by_oauth("google", "t-456").is_none());
    }

    #[test]
    fn create_oauth_user_rejects_email_already_taken_by_a_password_account() {
        let store = UserStore::in_memory();
        store.signup("streamer@example.com", "hunter2-strong", "free").unwrap();
        assert!(matches!(
            store.create_oauth_user("google", "g-123", "streamer@example.com", "free"),
            Err(UserError::EmailTaken)
        ));
    }

    #[test]
    fn create_oauth_user_rejects_identity_already_linked_to_another_email() {
        let store = UserStore::in_memory();
        store
            .create_oauth_user("google", "g-123", "first@example.com", "free")
            .unwrap();
        assert!(matches!(
            store.create_oauth_user("google", "g-123", "second@example.com", "free"),
            Err(UserError::OAuthIdentityTaken)
        ));
    }

    #[test]
    fn signup_validates_email_and_password() {
        let store = UserStore::in_memory();
        assert!(matches!(
            store.signup("not-an-email", "hunter2-strong", "free"),
            Err(UserError::InvalidEmail)
        ));
        assert!(matches!(
            store.signup("a@b", "hunter2-strong", "free"),
            Err(UserError::InvalidEmail)
        ));
        assert!(matches!(
            store.signup("a@b.co", "short", "free"),
            Err(UserError::WeakPassword)
        ));
    }

    #[test]
    fn password_hash_is_argon2_not_plaintext() {
        let store = UserStore::in_memory();
        let user = store.signup("a@b.co", "super-secret-pass", "free").unwrap();
        let hash = user.password_hash.expect("password-based signup has a hash");
        assert!(hash.starts_with("$argon2"));
        assert!(!hash.contains("super-secret-pass"));
    }

    #[test]
    fn persists_encrypted_and_reloads_without_plaintext() {
        let path = std::env::temp_dir().join(format!("sd-users-{}.enc", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let key = crate::crypto::derive_key(b"jwt", "streamdubber.users.v1");

        {
            let store = UserStore::with_file(&path, &key).unwrap();
            store.signup("persist@example.com", "persist-pass-1", "free").unwrap();
            store.mark_verified("persist@example.com").unwrap();
        }

        // Diskte düz e-posta/hash yok.
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("persist@example.com"));
        assert!(!raw.contains("$argon2"));

        let reloaded = UserStore::with_file(&path, &key).unwrap();
        let user = reloaded.verify_login("persist@example.com", "persist-pass-1").unwrap();
        assert!(user.email_verified);

        let wrong = crate::crypto::derive_key(b"other", "streamdubber.users.v1");
        assert!(UserStore::with_file(&path, &wrong).is_err());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn set_password_rotates_and_old_password_stops_working() {
        let store = UserStore::in_memory();
        store.signup("rot@example.com", "old-password-1", "free").unwrap();
        store.set_password("rot@example.com", "new-password-2").unwrap();
        assert!(store.verify_login("rot@example.com", "old-password-1").is_none());
        assert!(store.verify_login("rot@example.com", "new-password-2").is_some());
        assert!(matches!(
            store.set_password("rot@example.com", "short"),
            Err(UserError::WeakPassword)
        ));
    }

    #[test]
    fn twofactor_codes_are_single_use_with_attempt_limit() {
        let store = TwoFactorStore::new();
        let code = store.issue("a@b.co", CodePurpose::Login).unwrap();
        assert_eq!(code.len(), CODE_LEN);

        // Yanlış kod denemeleri sayılır.
        assert_eq!(
            store.verify("a@b.co", CodePurpose::Login, "000000").unwrap_err(),
            TwoFactorError::Invalid
        );
        // Doğru kod girer ve tüketilir.
        assert!(store.verify("a@b.co", CodePurpose::Login, &code).is_ok());
        assert_eq!(
            store.verify("a@b.co", CodePurpose::Login, &code).unwrap_err(),
            TwoFactorError::Invalid
        );
    }

    #[test]
    fn twofactor_attempt_limit_invalidates_code() {
        let store = TwoFactorStore::with_limits(CODE_TTL, Duration::ZERO);
        let code = store.issue("a@b.co", CodePurpose::Login).unwrap();
        for _ in 0..(MAX_CODE_ATTEMPTS - 1) {
            let _ = store.verify("a@b.co", CodePurpose::Login, "999999");
        }
        assert_eq!(
            store.verify("a@b.co", CodePurpose::Login, "999999").unwrap_err(),
            TwoFactorError::TooManyAttempts
        );
        // Kod artık doğru girilse de geçersiz.
        assert_eq!(
            store.verify("a@b.co", CodePurpose::Login, &code).unwrap_err(),
            TwoFactorError::Invalid
        );
    }

    #[test]
    fn twofactor_expiry_and_reissue_guard() {
        let store = TwoFactorStore::with_limits(Duration::ZERO, Duration::from_secs(60));
        let code = store.issue("a@b.co", CodePurpose::Login).unwrap();
        // TTL sıfır — kod anında süresi geçmiş.
        assert_eq!(
            store.verify("a@b.co", CodePurpose::Login, &code).unwrap_err(),
            TwoFactorError::Invalid
        );
        // Yeni kod istemek reissue guard'a takılır (60 sn).
        let store = TwoFactorStore::with_limits(CODE_TTL, Duration::from_secs(60));
        store.issue("a@b.co", CodePurpose::Login).unwrap();
        assert_eq!(
            store.issue("a@b.co", CodePurpose::Login).unwrap_err(),
            TwoFactorError::TooSoon
        );
        // Farklı amaç ayrı ad alanı — reset kodu istenebilir.
        assert!(store.issue("a@b.co", CodePurpose::Reset).is_ok());
    }

    #[test]
    fn login_and_reset_purposes_are_isolated() {
        let store = TwoFactorStore::with_limits(CODE_TTL, Duration::ZERO);
        let login_code = store.issue("a@b.co", CodePurpose::Login).unwrap();
        let reset_code = store.issue("a@b.co", CodePurpose::Reset).unwrap();
        // Login kodu reset'e geçmez.
        assert!(store.verify("a@b.co", CodePurpose::Reset, &login_code).is_err());
        assert!(store.verify("a@b.co", CodePurpose::Reset, &reset_code).is_ok());
    }

    #[test]
    fn email_validation_rejects_obvious_garbage() {
        assert!(email_looks_valid("a@b.co"));
        assert!(email_looks_valid("first.last+tag@sub.domain.io"));
        assert!(!email_looks_valid("no-at-sign"));
        assert!(!email_looks_valid("@no-local.co"));
        assert!(!email_looks_valid("a@nodot"));
        assert!(!email_looks_valid("a@.leading.dot"));
        assert!(!email_looks_valid("spa ce@b.co"));
    }
}
