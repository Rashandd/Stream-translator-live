//! Faz 2/3 "coming soon" kataloğu için minimal lead-capture (Faz 2 hazırlığı).
//!
//! Tek public, auth gerektirmeyen endpoint: `POST /api/waitlist { email, interest }`.
//! `/solutions` sayfasındaki her `coming_soon`/`waitlist` kartı bu uca yazar —
//! amaç gerçekten lead toplamak olduğu için (dashboard'un geri kalanının aksine)
//! bu uç dekoratif değil, gerçekten Postgres'e yazar. Aynı email aynı `interest`e
//! iki kez kaydolamaz (DB unique constraint, bkz. `migrations/0003_waitlist.sql`).
//! `DATABASE_URL` yoksa istek kabul edilir ama sadece loglanır (diğer best-effort
//! persist yolları — `pairing`, `usage` — ile aynı davranış).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::logging::{LogLevel, Logger};

/// Aynı istemci anahtarı (bkz. [`client_key`]) için dakikada izin verilen istek.
const RATE_LIMIT_PER_MIN: u32 = 5;
const RATE_WINDOW: Duration = Duration::from_secs(60);
/// Rate map bu boyutu aşarsa süresi geçmiş pencereler ayıklanır.
const RATE_PRUNE_THRESHOLD: usize = 4096;
const MAX_EMAIL_LEN: usize = 254;
const MAX_INTEREST_LEN: usize = 120;

/// Sabit pencereli, bellek içi per-IP rate limiter (bkz. `ext_twitch::ExtTwitch::check_rate`
/// — aynı desen, ayrı instance).
pub struct WaitlistLimiter {
    rate: Mutex<HashMap<String, (Instant, u32)>>,
}

impl WaitlistLimiter {
    pub fn new() -> Self {
        WaitlistLimiter {
            rate: Mutex::new(HashMap::new()),
        }
    }

    fn check(&self, key: &str, limit: u32) -> bool {
        let Ok(mut map) = self.rate.lock() else {
            return false;
        };
        if map.len() > RATE_PRUNE_THRESHOLD {
            map.retain(|_, (start, _)| start.elapsed() < RATE_WINDOW);
        }
        let now = Instant::now();
        let entry = map.entry(key.to_string()).or_insert((now, 0));
        if entry.0.elapsed() >= RATE_WINDOW {
            *entry = (now, 0);
        }
        entry.1 += 1;
        entry.1 <= limit
    }
}

impl Default for WaitlistLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// `POST /api/waitlist` gövdesi.
#[derive(Debug, Deserialize)]
pub struct WaitlistRequest {
    pub email: String,
    pub interest: String,
}

#[derive(Debug, Serialize)]
pub struct WaitlistResponse {
    pub status: &'static str,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WaitlistError {
    #[error("invalid email address")]
    InvalidEmail,
    #[error("interest must not be empty")]
    InvalidInterest,
    #[error("too many requests, try again later")]
    RateLimited,
    #[error("already on the waitlist for this feature")]
    Duplicate,
    #[error("internal waitlist error")]
    Internal,
}

fn status_for(err: &WaitlistError) -> StatusCode {
    match err {
        WaitlistError::InvalidEmail | WaitlistError::InvalidInterest => StatusCode::BAD_REQUEST,
        WaitlistError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
        WaitlistError::Duplicate => StatusCode::CONFLICT,
        WaitlistError::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Basit, bağımlılıksız e-posta format kontrolü: tam bir `@`, boş olmayan
/// local/domain parçaları, domain içinde `.` ve baştaki/sondaki `.` yok,
/// boşluk yok. Tam RFC 5322 doğrulaması değil — sadece bariz hatalı girdiyi
/// (boş, `@` yok, `a@b`) elemek için yeterli asgari kontrol.
pub fn is_valid_email(email: &str) -> bool {
    if email.is_empty() || email.len() > MAX_EMAIL_LEN || email.chars().any(char::is_whitespace) {
        return false;
    }
    let mut parts = email.split('@');
    let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
}

/// İstemci anahtarı: `X-Forwarded-For` (Railway edge'i set eder) → `X-Real-IP`
/// → bilinmiyorsa ortak "unknown" kovası. `ConnectInfo<SocketAddr>` kullanmadık
/// çünkü router `into_make_service()` ile sunuluyor (connect-info olmadan);
/// production'da edge zaten forwarded header basıyor.
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

/// `POST /api/waitlist` (public, auth yok — herkese açık lead-capture).
pub async fn submit(
    State(limiter): State<Arc<WaitlistLimiter>>,
    State(db): State<Option<sqlx::PgPool>>,
    State(logger): State<Arc<Logger>>,
    headers: HeaderMap,
    Json(req): Json<WaitlistRequest>,
) -> Result<(StatusCode, Json<WaitlistResponse>), (StatusCode, String)> {
    if !limiter.check(&client_key(&headers), RATE_LIMIT_PER_MIN) {
        let err = WaitlistError::RateLimited;
        return Err((status_for(&err), err.to_string()));
    }

    let email = req.email.trim();
    let interest = req.interest.trim();

    if !is_valid_email(email) {
        let err = WaitlistError::InvalidEmail;
        return Err((status_for(&err), err.to_string()));
    }
    if interest.is_empty() || interest.len() > MAX_INTEREST_LEN {
        let err = WaitlistError::InvalidInterest;
        return Err((status_for(&err), err.to_string()));
    }

    let Some(pool) = db else {
        logger.log(
            LogLevel::Warn,
            "waitlist",
            "signup not persisted (DATABASE_URL not set)",
            None,
            None,
        );
        return Ok((StatusCode::CREATED, Json(WaitlistResponse { status: "ok" })));
    };

    match sqlx::query("INSERT INTO waitlist_signups (email, interest) VALUES ($1, $2)")
        .bind(email)
        .bind(interest)
        .execute(&pool)
        .await
    {
        Ok(_) => Ok((StatusCode::CREATED, Json(WaitlistResponse { status: "ok" }))),
        Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => {
            let err = WaitlistError::Duplicate;
            Err((status_for(&err), err.to_string()))
        }
        Err(err) => {
            logger.log(
                LogLevel::Warn,
                "waitlist",
                &format!("insert failed: {err}"),
                None,
                None,
            );
            let err = WaitlistError::Internal;
            Err((status_for(&err), err.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_emails_are_accepted() {
        assert!(is_valid_email("person@example.com"));
        assert!(is_valid_email("a.b+tag@sub.example.co"));
    }

    #[test]
    fn malformed_emails_are_rejected() {
        assert!(!is_valid_email(""));
        assert!(!is_valid_email("no-at-sign.com"));
        assert!(!is_valid_email("two@at@signs.com"));
        assert!(!is_valid_email("@missing-local.com"));
        assert!(!is_valid_email("missing-domain@"));
        assert!(!is_valid_email("no-dot-in-domain@localhost"));
        assert!(!is_valid_email("has space@example.com"));
        assert!(!is_valid_email("trailing-dot@example."));
        assert!(!is_valid_email("leading-dot@.example.com"));
    }

    #[test]
    fn rate_limit_resets_after_window() {
        let limiter = WaitlistLimiter::new();
        assert!(limiter.check("1.2.3.4", 2));
        assert!(limiter.check("1.2.3.4", 2));
        assert!(!limiter.check("1.2.3.4", 2));
        // Farklı anahtar etkilenmez.
        assert!(limiter.check("5.6.7.8", 2));
    }

    #[test]
    fn client_key_prefers_forwarded_for_then_real_ip_then_unknown() {
        let mut headers = HeaderMap::new();
        assert_eq!(client_key(&headers), "unknown");

        headers.insert("x-real-ip", "9.9.9.9".parse().unwrap());
        assert_eq!(client_key(&headers), "9.9.9.9");

        headers.insert("x-forwarded-for", "1.1.1.1, 2.2.2.2".parse().unwrap());
        assert_eq!(client_key(&headers), "1.1.1.1");
    }

    /// Gerçek Postgres ile duplicate-signup davranışı. Çalıştırmak için:
    /// `TEST_DATABASE_URL=postgres://... cargo test -- --ignored waitlist_duplicate`
    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL with a migrated database"]
    async fn waitlist_duplicate_signup_is_rejected_cleanly_against_real_db() {
        let url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL");
        let pool = crate::db::connect(&url).await.expect("connect");
        crate::db::run_migrations(&pool).await.expect("migrations");

        sqlx::query("DELETE FROM waitlist_signups WHERE email = 'dup-test@sd.io'")
            .execute(&pool)
            .await
            .expect("cleanup");

        let insert = || {
            sqlx::query("INSERT INTO waitlist_signups (email, interest) VALUES ($1, $2)")
                .bind("dup-test@sd.io")
                .bind("watch-party")
                .execute(&pool)
        };

        insert().await.expect("first insert succeeds");
        let err = insert().await.expect_err("duplicate insert must fail");
        match err {
            sqlx::Error::Database(db_err) => assert!(db_err.is_unique_violation()),
            other => panic!("expected a database error, got {other:?}"),
        }
    }
}
