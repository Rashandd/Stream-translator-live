//! SDK ↔ hesap eşleştirme (pairing) backend ucu.
//!
//! Akış (bkz. `sdk/src/core/pairing.rs`): panel 6 haneli kod gösterir →
//! yayıncı kodu SDK'ya yazar → SDK `POST /api/pairing/claim` ile kod +
//! cihaz bilgisini gönderir → backend kodu doğrular ve SDK'ya uzun ömürlü
//! bir yayıncı token'ı (JWT) verir. Kodlar tek kullanımlık ve kısa
//! ömürlüdür (varsayılan 10 dk); bellek içi tutulur — restart bekleyen
//! kodları geçersiz kılar (kabul edilebilir: panel yenisini üretir).
//! `DATABASE_URL` varsa eşleşme `sdk_sessions` tablosuna da yazılır
//! (best-effort; users FK'sı 1.4 kullanıcı sistemiyle doğal kapanır).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::auth::authz::AuthUser;
use crate::auth::jwt::{Claims, Jwt};
use crate::logging::{LogLevel, Logger};

/// Kod uzunluğu — SDK tarafındaki `pairing::CODE_LEN` ile birebir.
pub const CODE_LEN: usize = 6;
/// Kod geçerlilik süresi.
const CODE_TTL: Duration = Duration::from_secs(600);
/// SDK cihaz token'ı ömrü (30 gün) — cihaz yeniden eşleşmeden çalışır.
const DEVICE_TOKEN_TTL_SECS: i64 = 30 * 24 * 3600;

/// `/api/pairing/claim` auth gerektirmez (kod capability'dir), bu yüzden
/// istemci başına deneme limiti şart — 6 haneli kod alanı sadece 1M
/// kombinasyon, limitsiz bırakılırsa 10 dakikalık TTL içinde brute-force
/// edilebilir (bkz. `waitlist::WaitlistLimiter` — aynı desen, ayrı instance).
const CLAIM_RATE_LIMIT_PER_MIN: u32 = 10;
const CLAIM_RATE_WINDOW: Duration = Duration::from_secs(60);
const CLAIM_RATE_PRUNE_THRESHOLD: usize = 4096;

struct PendingPairing {
    user_id: String,
    plan: String,
    created: Instant,
}

/// Bekleyen eşleştirme kodlarını tutan bellek içi depo.
pub struct PairingStore {
    codes: Mutex<HashMap<String, PendingPairing>>,
    ttl: Duration,
}

impl PairingStore {
    pub fn new() -> Self {
        PairingStore {
            codes: Mutex::new(HashMap::new()),
            ttl: CODE_TTL,
        }
    }

    /// Test için kısa TTL.
    pub fn with_ttl(ttl: Duration) -> Self {
        PairingStore {
            codes: Mutex::new(HashMap::new()),
            ttl,
        }
    }

    /// Yayıncı için yeni bir 6 haneli kod üretir (eski kodu geçersiz kılar —
    /// kullanıcı başına tek aktif kod).
    pub fn issue(&self, user_id: &str, plan: &str) -> Result<String, PairingError> {
        let mut map = self
            .codes
            .lock()
            .map_err(|_| PairingError::Internal("pairing lock poisoned"))?;

        // Aynı kullanıcının önceki kodu iptal (tek aktif kod).
        map.retain(|_, pending| pending.user_id != user_id);

        // Çakışma pratikte yok denecek kadar az; yine de birkaç deneme yap.
        for _ in 0..8 {
            let code = random_code()?;
            if map.contains_key(&code) {
                continue;
            }
            map.insert(
                code.clone(),
                PendingPairing {
                    user_id: user_id.to_string(),
                    plan: plan.to_string(),
                    created: Instant::now(),
                },
            );
            return Ok(code);
        }
        Err(PairingError::Internal("could not generate unique code"))
    }

    /// Kodu tüketir (tek kullanımlık). Geçerliyse (user_id, plan) döner.
    pub fn claim(&self, code: &str) -> Result<(String, String), PairingError> {
        let mut map = self
            .codes
            .lock()
            .map_err(|_| PairingError::Internal("pairing lock poisoned"))?;
        let pending = map.remove(code).ok_or(PairingError::UnknownCode)?;
        if pending.created.elapsed() > self.ttl {
            return Err(PairingError::Expired);
        }
        Ok((pending.user_id, pending.plan))
    }

    /// Kodun geçerlilik süresi (saniye) — panel geri sayımı için.
    pub fn ttl_secs(&self) -> u64 {
        self.ttl.as_secs()
    }
}

impl Default for PairingStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Sabit pencereli, bellek içi per-IP rate limiter — `/api/pairing/claim`
/// brute-force koruması (bkz. `CLAIM_RATE_LIMIT_PER_MIN`).
pub struct ClaimLimiter {
    rate: Mutex<HashMap<String, (Instant, u32)>>,
}

impl ClaimLimiter {
    pub fn new() -> Self {
        ClaimLimiter {
            rate: Mutex::new(HashMap::new()),
        }
    }

    fn check(&self, key: &str) -> bool {
        let Ok(mut map) = self.rate.lock() else {
            return false;
        };
        if map.len() > CLAIM_RATE_PRUNE_THRESHOLD {
            map.retain(|_, (start, _)| start.elapsed() < CLAIM_RATE_WINDOW);
        }
        let now = Instant::now();
        let entry = map.entry(key.to_string()).or_insert((now, 0));
        if entry.0.elapsed() >= CLAIM_RATE_WINDOW {
            *entry = (now, 0);
        }
        entry.1 += 1;
        entry.1 <= CLAIM_RATE_LIMIT_PER_MIN
    }
}

impl Default for ClaimLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// İstemci anahtarı: `X-Forwarded-For` → `X-Real-IP` → "unknown" (bkz.
/// `waitlist::client_key` — aynı desen).
fn client_key(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| headers.get("x-real-ip").and_then(|v| v.to_str().ok()))
        .unwrap_or("unknown")
        .to_string()
}

/// 6 haneli rastgele numerik kod (kriptografik RNG).
fn random_code() -> Result<String, PairingError> {
    let mut bytes = [0u8; CODE_LEN];
    getrandom::getrandom(&mut bytes).map_err(|_| PairingError::Internal("rng failure"))?;
    Ok(bytes.iter().map(|b| char::from(b'0' + (b % 10))).collect())
}

/// Pairing hataları.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PairingError {
    #[error("unknown or already used pairing code")]
    UnknownCode,
    #[error("pairing code expired")]
    Expired,
    #[error("pairing code must be {CODE_LEN} digits")]
    BadFormat,
    #[error("internal pairing error: {0}")]
    Internal(&'static str),
}

fn pairing_status(err: &PairingError) -> StatusCode {
    match err {
        PairingError::UnknownCode => StatusCode::NOT_FOUND,
        PairingError::Expired => StatusCode::GONE,
        PairingError::BadFormat => StatusCode::BAD_REQUEST,
        PairingError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// `POST /api/pairing/code` yanıtı — panel bu kodu gösterir.
#[derive(Debug, Serialize)]
pub struct PairingCodeResponse {
    pub code: String,
    pub expires_in_secs: u64,
}

/// `POST /api/pairing/code` (yayıncı, auth) — SDK'ya girilecek kodu üretir.
pub async fn create_code(
    user: AuthUser,
    State(pairing): State<Arc<PairingStore>>,
    State(logger): State<Arc<Logger>>,
) -> Result<Json<PairingCodeResponse>, (StatusCode, String)> {
    let code = pairing
        .issue(&user.claims.sub, &user.claims.plan)
        .map_err(|e| (pairing_status(&e), e.to_string()))?;
    logger.log(
        LogLevel::Info,
        "pairing",
        "pairing code issued",
        Some(&user.claims.sub),
        None,
    );
    Ok(Json(PairingCodeResponse {
        code,
        expires_in_secs: pairing.ttl_secs(),
    }))
}

/// `POST /api/pairing/claim` gövdesi — SDK'nın `PairingRequest`'iyle birebir.
#[derive(Debug, Deserialize)]
pub struct ClaimRequest {
    pub pairing_code: String,
    pub device_name: String,
    #[serde(default)]
    pub sdk_version: String,
}

/// Başarılı eşleşme: SDK bu token'la bulut WS'e bağlanır ve relay
/// hedeflerini çeker. `sdk_session_id`+`heartbeat_secret`, canlı bağlantı
/// durumu için (`sdk_session` modülü) — secret burada BİR KEZ düz metin
/// döner, SDK yerel şifreli deposunda saklar ve heartbeat imzalamak için
/// kullanır; sunucu bir daha göstermez.
#[derive(Debug, Serialize)]
pub struct ClaimResponse {
    pub token: String,
    pub user_id: String,
    pub expires_at: i64,
    pub sdk_session_id: String,
    pub heartbeat_secret: String,
}

/// `POST /api/pairing/claim` (SDK — auth başlığı yok, kod capability'dir).
#[allow(clippy::too_many_arguments)]
pub async fn claim(
    State(pairing): State<Arc<PairingStore>>,
    State(jwt): State<Arc<Jwt>>,
    State(db): State<Option<sqlx::PgPool>>,
    State(logger): State<Arc<Logger>>,
    State(limiter): State<Arc<ClaimLimiter>>,
    State(sessions): State<Arc<crate::sdk_session::SdkSessionStore>>,
    headers: HeaderMap,
    Json(req): Json<ClaimRequest>,
) -> Result<Json<ClaimResponse>, (StatusCode, String)> {
    if !limiter.check(&client_key(&headers)) {
        logger.log(
            LogLevel::Warn,
            "pairing",
            "pairing claim rate limited",
            None,
            None,
        );
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "too many pairing attempts, try again later".to_string(),
        ));
    }

    let code = req.pairing_code.trim();
    if code.len() != CODE_LEN || !code.bytes().all(|b| b.is_ascii_digit()) {
        let err = PairingError::BadFormat;
        return Err((pairing_status(&err), err.to_string()));
    }

    let (user_id, plan) = pairing
        .claim(code)
        .map_err(|e| (pairing_status(&e), e.to_string()))?;

    let expires_at = chrono::Utc::now().timestamp() + DEVICE_TOKEN_TTL_SECS;
    let claims = Claims {
        sub: user_id.clone(),
        role: "broadcaster".to_string(),
        plan,
        exp: expires_at,
    };
    let token = jwt
        .issue(&claims)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    logger.log(
        LogLevel::Info,
        "pairing",
        &format!("sdk paired: {}", req.device_name),
        Some(&user_id),
        None,
    );

    // Best-effort DB kaydı (sdk_sessions tablosu — kalıcı denetim izi;
    // `sdk_session::SdkSessionStore`'dan FARKLI: o canlı presence/heartbeat
    // kaynağı, DB'siz kurulumlarda da çalışır).
    if let Some(pool) = &db {
        persist_session(pool, &user_id, &req, &logger).await;
    }

    let (sdk_session_id, heartbeat_secret) = sessions
        .create(&user_id, &req.device_name, &req.sdk_version)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(ClaimResponse {
        token,
        user_id,
        expires_at,
        sdk_session_id,
        heartbeat_secret,
    }))
}

/// `sdk_sessions` satırını yazar; her hata warn (pipeline'ı etkilemez).
async fn persist_session(pool: &sqlx::PgPool, user_id: &str, req: &ClaimRequest, logger: &Logger) {
    let Ok(uuid) = uuid::Uuid::parse_str(user_id) else {
        return;
    };
    let result = sqlx::query(
        "INSERT INTO sdk_sessions (user_id, device_name, sdk_version, paired_at, last_seen_at) \
         VALUES ($1, $2, $3, NOW(), NOW())",
    )
    .bind(uuid)
    .bind(&req.device_name)
    .bind(&req.sdk_version)
    .execute(pool)
    .await;
    if let Err(err) = result {
        logger.log(
            LogLevel::Warn,
            "pairing",
            &format!("sdk_sessions persist failed: {err}"),
            Some(user_id),
            None,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issued_code_is_six_digits_and_claimable_once() {
        let store = PairingStore::new();
        let code = store.issue("u1", "free").unwrap();
        assert_eq!(code.len(), CODE_LEN);
        assert!(code.bytes().all(|b| b.is_ascii_digit()));

        let (user, plan) = store.claim(&code).unwrap();
        assert_eq!(user, "u1");
        assert_eq!(plan, "free");
        // Tek kullanımlık.
        assert_eq!(store.claim(&code), Err(PairingError::UnknownCode));
    }

    #[test]
    fn new_code_invalidates_previous_for_same_user() {
        let store = PairingStore::new();
        let first = store.issue("u1", "free").unwrap();
        let second = store.issue("u1", "free").unwrap();
        assert_eq!(store.claim(&first), Err(PairingError::UnknownCode));
        assert!(store.claim(&second).is_ok());
    }

    #[test]
    fn expired_code_is_rejected() {
        let store = PairingStore::with_ttl(Duration::from_millis(0));
        let code = store.issue("u1", "free").unwrap();
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(store.claim(&code), Err(PairingError::Expired));
    }

    #[test]
    fn unknown_code_is_rejected() {
        let store = PairingStore::new();
        assert_eq!(store.claim("000000"), Err(PairingError::UnknownCode));
    }

    #[test]
    fn claim_limiter_allows_up_to_limit_then_blocks() {
        let limiter = ClaimLimiter::new();
        for _ in 0..CLAIM_RATE_LIMIT_PER_MIN {
            assert!(limiter.check("1.2.3.4"));
        }
        assert!(!limiter.check("1.2.3.4"));
    }

    #[test]
    fn claim_limiter_tracks_clients_independently() {
        let limiter = ClaimLimiter::new();
        for _ in 0..CLAIM_RATE_LIMIT_PER_MIN {
            assert!(limiter.check("1.2.3.4"));
        }
        assert!(!limiter.check("1.2.3.4"));
        // Farklı istemci kendi bütçesini kullanır.
        assert!(limiter.check("5.6.7.8"));
    }
}
