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
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::auth::jwt::Claims;
use crate::crypto::Cipher;
use crate::logging::LogLevel;
use crate::state::AppState;

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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRecord {
    pub id: String,
    pub email: String,
    pub password_hash: String,
    pub plan: String,
    pub email_verified: bool,
    pub created_at_ms: i64,
}

/// Kullanıcı deposu hataları.
#[derive(Debug, thiserror::Error)]
pub enum UserError {
    #[error("email already registered")]
    EmailTaken,
    #[error("invalid email address")]
    InvalidEmail,
    #[error("password must be at least {MIN_PASSWORD_LEN} characters")]
    WeakPassword,
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
            password_hash: hash_password(password)?,
            plan: plan.to_string(),
            email_verified: false,
            created_at_ms: chrono::Utc::now().timestamp_millis(),
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

    /// E-posta + şifre doğrulaması. Kullanıcı yokken de argon2 koşulur —
    /// zamanlamadan hesap varlığı sızmaz.
    pub fn verify_login(&self, email: &str, password: &str) -> Option<UserRecord> {
        let record = self
            .entries
            .read()
            .ok()
            .and_then(|m| m.get(&email.trim().to_lowercase()).cloned());
        match record {
            Some(record) if verify_password(&record.password_hash, password) => Some(record),
            Some(_) => None,
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
        let updated = self.update(email, |r| r.password_hash = hash.clone())?;
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
        "INSERT INTO users (id, email, password_hash, plan_id, role, email_verified)
         VALUES ($1::uuid, $2, $3, (SELECT id FROM plans WHERE name = $4), 'broadcaster', $5)
         ON CONFLICT (email) DO UPDATE SET
             password_hash = EXCLUDED.password_hash,
             plan_id = EXCLUDED.plan_id,
             email_verified = EXCLUDED.email_verified,
             updated_at = NOW()",
    )
    .bind(&record.id)
    .bind(record.email.to_lowercase())
    .bind(&record.password_hash)
    .bind(&record.plan)
    .bind(record.email_verified)
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
    Json(req): Json<SignupRequest>,
) -> Result<(StatusCode, Json<OkResponse>), (StatusCode, String)> {
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
    },
}

/// `POST /api/auth/login` — tek giriş ucu.
///
/// Önce env-bootstrap admin denenir (davranış V2'den beri değişmedi: admin'e
/// 2FA yok, 12 saatlik `role=admin` token). Sonra kullanıcı deposu:
/// `auth.email_2fa` config'ine göre 6 haneli kod maillenir veya token döner.
/// Kimlik uyuşmazsa 401 — hangi alanın yanlış olduğu söylenmez.
pub async fn login(
    State(app): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, String)> {
    // 1) Bootstrap admin (varsa).
    if let Some(admin) = app.bootstrap_admin.as_ref() {
        if admin.verify(&req.email, &req.password) {
            let expires_at = chrono::Utc::now().timestamp() + 12 * 3600;
            let claims = Claims {
                sub: "admin-bootstrap".to_string(),
                role: "admin".to_string(),
                plan: "streamer".to_string(),
                exp: expires_at,
            };
            let token = app
                .jwt
                .issue(&claims)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            app.logger.log(
                LogLevel::Info,
                "auth",
                "admin login succeeded",
                Some("admin-bootstrap"),
                None,
            );
            return Ok(Json(LoginResponse::Token {
                token,
                expires_at,
                role: "admin".to_string(),
            }));
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

    let mode = app
        .service_config
        .get("auth.email_2fa")
        .unwrap_or_else(|| "auto".to_string());
    if mode == "off" {
        return issue_user_token(&app, &user).map(Json);
    }

    // 2FA kodu üret ve maille.
    let code = match app.twofactor.issue(&user.email, CodePurpose::Login) {
        Ok(code) => code,
        Err(TwoFactorError::TooSoon) => {
            // Kod zaten yolda — frontend verify ekranında bekliyor olmalı.
            return Ok(Json(LoginResponse::TwoFactorPending {
                two_factor_required: true,
                expires_in_secs: app.twofactor.ttl_secs(),
            }));
        }
        Err(err) => return Err((StatusCode::INTERNAL_SERVER_ERROR, err.to_string())),
    };
    let mail_result = crate::mail::send(
        app.secrets.as_ref(),
        &app.service_config,
        &user.email,
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
                Some(&user.id),
                None,
            );
            Ok(Json(LoginResponse::TwoFactorPending {
                two_factor_required: true,
                expires_in_secs: app.twofactor.ttl_secs(),
            }))
        }
        Err(crate::mail::MailError::NotConfigured) if mode != "required" => {
            // Dev/kurulum modu: SMTP panelden girilene kadar 2FA atlanır.
            app.twofactor.cancel(&user.email, CodePurpose::Login);
            app.logger.log(
                LogLevel::Warn,
                "auth",
                "smtp not configured — 2fa skipped (auth.email_2fa=auto)",
                Some(&user.id),
                None,
            );
            issue_user_token(&app, &user).map(Json)
        }
        Err(crate::mail::MailError::NotConfigured) => {
            app.twofactor.cancel(&user.email, CodePurpose::Login);
            Err((
                StatusCode::SERVICE_UNAVAILABLE,
                "email 2fa required but smtp is not configured".to_string(),
            ))
        }
        Err(err) => {
            app.twofactor.cancel(&user.email, CodePurpose::Login);
            app.logger.log(
                LogLevel::Error,
                "auth",
                &format!("2fa mail send failed: {err}"),
                Some(&user.id),
                None,
            );
            Err((
                StatusCode::BAD_GATEWAY,
                "could not send verification code".to_string(),
            ))
        }
    }
}

fn issue_user_token(
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
    Json(req): Json<TwoFactorRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, String)> {
    verify_code_or_status(&app, &req.email, CodePurpose::Login, &req.code)?;
    let Some(user) = app.users.get(&req.email) else {
        return Err((StatusCode::UNAUTHORIZED, "invalid or expired code".to_string()));
    };
    if let Err(err) = app.users.mark_verified(&user.email) {
        tracing::warn!(error = %err, "mark_verified failed");
    }
    issue_user_token(&app, &user).map(Json)
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
        assert!(user.password_hash.starts_with("$argon2"));
        assert!(!user.password_hash.contains("super-secret-pass"));
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
