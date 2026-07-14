//! Sabit pencereli per-anahtar rate limiter — `pairing::ClaimLimiter` ve
//! `AuthLimiter` bu genel amaçlı çekirdeği (`RateLimiter`) paylaşır (Faz 1.13:
//! login/signup brute-force koruması için eklendi).
//!
//! BH-04: `REDIS_URL` yapılandırılmışsa sayaç Redis'te tutulur ve backend
//! instance'ları arasında paylaşılır (tek atomik Lua script'i). Redis yoksa
//! bugünkü bellek-içi (tek-instance) yol aynen kullanılır. Backend hatası
//! **asla** sessiz "izin ver" değildir — 503'e çevrilir (fail-closed, bkz.
//! `docs/adr/0001-production-state-ownership.md` §2).
//!
//! Not: `waitlist::WaitlistLimiter` ve `ext_twitch::ExtTwitch::check_rate` de
//! aynı sabit-pencere mantığını bağımsızca kopyalar — kapsam dışı; ileride bu
//! çekirdeğe katlanabilirler (fold-in).

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use axum::http::HeaderMap;
use redis::aio::ConnectionManager;

/// Rate map bu boyutu aşarsa süresi geçmiş pencereler ayıklanır (bellek yolu).
const DEFAULT_PRUNE_THRESHOLD: usize = 4096;

/// Atomik sabit-pencere sayacı: `INCR` + (yalnız ilk artışta) `EXPIRE`, tek
/// round-trip'te. Tek script olması, `INCR`-sonra-`EXPIRE` arasında bir çökme
/// olsa bile TTL'siz (kalıcı) bir anahtar bırakma ihtimalini ortadan kaldırır.
static WINDOW_SCRIPT: LazyLock<redis::Script> = LazyLock::new(|| {
    redis::Script::new(
        r"local count = redis.call('INCR', KEYS[1])
if count == 1 then redis.call('EXPIRE', KEYS[1], ARGV[1]) end
return count",
    )
});

/// Rate limiter backend (Redis) hataları.
#[derive(Debug, thiserror::Error)]
pub enum RateLimitError {
    #[error("redis rate limiter backend error: {0}")]
    Redis(#[from] redis::RedisError),
}

/// Bir rate-limit backend hatasını HTTP 503'e çevirir — sessiz "izin ver"
/// yerine fail-closed. Rate limiter bir güvenlik kontrolüdür; backend
/// ulaşılamıyorsa istek reddedilir (bkz. ADR-0001 §2).
pub fn to_http_error(err: &RateLimitError) -> (axum::http::StatusCode, String) {
    tracing::error!(%err, "rate limiter backend unavailable");
    (
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "rate limiting temporarily unavailable, try again shortly".to_string(),
    )
}

/// Genel amaçlı sabit-pencere sayacı. Redis varsa paylaşımlı, yoksa bellek-içi.
pub struct RateLimiter {
    rate: Mutex<HashMap<String, (Instant, u32)>>,
    window: Duration,
    prune_threshold: usize,
    /// `Some` → Redis-destekli (instance'lar arası paylaşımlı sayaç).
    redis: Option<ConnectionManager>,
    /// Redis anahtar ad-alanı segmenti (`cache::key`); bellek yolunda kullanılmaz.
    env_label: String,
}

impl RateLimiter {
    /// Bellek-içi (tek-instance) rate limiter.
    pub fn new(window: Duration) -> Self {
        RateLimiter {
            rate: Mutex::new(HashMap::new()),
            window,
            prune_threshold: DEFAULT_PRUNE_THRESHOLD,
            redis: None,
            env_label: String::new(),
        }
    }

    /// Redis-destekli (instance'lar arası paylaşımlı) rate limiter.
    pub fn with_redis(window: Duration, redis: ConnectionManager, env_label: &str) -> Self {
        RateLimiter {
            rate: Mutex::new(HashMap::new()),
            window,
            prune_threshold: DEFAULT_PRUNE_THRESHOLD,
            redis: Some(redis),
            env_label: env_label.to_string(),
        }
    }

    /// `endpoint`+`key` için bu pencerede `limit`e kadar izin verir.
    ///
    /// `endpoint`, eskiden bellek anahtarına gömülen `"login:"`/`"signup:"`
    /// önekini açık bir parametreye çıkarır ve Redis anahtarında da aynı
    /// ayrımı sağlar. Redis yapılandırılmışsa tek atomik Lua script'i çalışır;
    /// her backend hatası `Err`'dir (çağıran 503'e çevirir), asla sessiz izin.
    pub async fn check(
        &self,
        endpoint: &str,
        key: &str,
        limit: u32,
    ) -> Result<bool, RateLimitError> {
        if let Some(conn) = &self.redis {
            let redis_key = crate::cache::key(&self.env_label, &[endpoint, key]);
            let mut conn = conn.clone();
            let count: i64 = WINDOW_SCRIPT
                .key(&redis_key)
                .arg(self.window.as_secs())
                .invoke_async(&mut conn)
                .await?;
            return Ok(count <= i64::from(limit));
        }
        Ok(self.check_memory(&format!("{endpoint}:{key}"), limit))
    }

    /// Bellek-içi sabit-pencere sayacı (Redis yokken). Kilit zehirlenirse
    /// (panic sonrası) güvenli tarafta kal: reddet.
    fn check_memory(&self, key: &str, limit: u32) -> bool {
        let Ok(mut map) = self.rate.lock() else {
            return false;
        };
        if map.len() > self.prune_threshold {
            map.retain(|_, (start, _)| start.elapsed() < self.window);
        }
        let now = Instant::now();
        let entry = map.entry(key.to_string()).or_insert((now, 0));
        if entry.0.elapsed() >= self.window {
            *entry = (now, 0);
        }
        entry.1 += 1;
        entry.1 <= limit
    }
}

/// `POST /api/auth/login` ve `/api/auth/signup` için istemci başına dakikalık
/// deneme limiti. İki uç ayrı sayaç tutar (aynı IP'den art arda login denemesi
/// signup'ı etkilemez) ama tek `RateLimiter` çekirdeğini `endpoint` parametresi
/// ile paylaşır. Limitler `auth.login.rate_limit_per_min` /
/// `auth.signup.rate_limit_per_min` config anahtarlarından okunur.
pub struct AuthLimiter {
    inner: RateLimiter,
}

impl AuthLimiter {
    pub fn new() -> Self {
        AuthLimiter {
            inner: RateLimiter::new(Duration::from_secs(60)),
        }
    }

    /// Redis-destekli varyant (BH-04) — sayaç instance'lar arası paylaşılır.
    pub fn with_redis(redis: ConnectionManager, env_label: &str) -> Self {
        AuthLimiter {
            inner: RateLimiter::with_redis(Duration::from_secs(60), redis, env_label),
        }
    }

    pub async fn check_login(&self, key: &str, limit: u32) -> Result<bool, RateLimitError> {
        self.inner.check("auth-login", key, limit).await
    }

    pub async fn check_signup(&self, key: &str, limit: u32) -> Result<bool, RateLimitError> {
        self.inner.check("auth-signup", key, limit).await
    }
}

impl Default for AuthLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// İstemci anahtarı: `X-Forwarded-For` (edge/proxy set eder) → `X-Real-IP` →
/// bilinmiyorsa ortak "unknown" kovası. `ConnectInfo<SocketAddr>` kullanılmaz
/// çünkü router `into_make_service()` ile sunuluyor (connect-info olmadan);
/// production'da edge zaten forwarded header basar.
pub fn client_key(headers: &HeaderMap) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn allows_up_to_limit_then_blocks() {
        let limiter = RateLimiter::new(Duration::from_secs(60));
        assert!(limiter.check("auth-login", "1.2.3.4", 3).await.unwrap());
        assert!(limiter.check("auth-login", "1.2.3.4", 3).await.unwrap());
        assert!(limiter.check("auth-login", "1.2.3.4", 3).await.unwrap());
        assert!(!limiter.check("auth-login", "1.2.3.4", 3).await.unwrap());
    }

    #[tokio::test]
    async fn tracks_keys_independently() {
        let limiter = RateLimiter::new(Duration::from_secs(60));
        assert!(limiter.check("auth-login", "1.2.3.4", 1).await.unwrap());
        assert!(!limiter.check("auth-login", "1.2.3.4", 1).await.unwrap());
        assert!(limiter.check("auth-login", "5.6.7.8", 1).await.unwrap());
    }

    #[tokio::test]
    async fn endpoints_are_isolated_for_the_same_key() {
        // Aynı anahtar farklı `endpoint`'lerde ayrı sayaç tutar.
        let limiter = RateLimiter::new(Duration::from_secs(60));
        assert!(limiter.check("auth-login", "1.2.3.4", 1).await.unwrap());
        assert!(!limiter.check("auth-login", "1.2.3.4", 1).await.unwrap());
        assert!(limiter.check("auth-signup", "1.2.3.4", 1).await.unwrap());
    }

    #[tokio::test]
    async fn resets_after_window_elapses() {
        let limiter = RateLimiter::new(Duration::from_millis(20));
        assert!(limiter.check("auth-login", "1.2.3.4", 1).await.unwrap());
        assert!(!limiter.check("auth-login", "1.2.3.4", 1).await.unwrap());
        std::thread::sleep(Duration::from_millis(30));
        assert!(limiter.check("auth-login", "1.2.3.4", 1).await.unwrap());
    }

    #[tokio::test]
    async fn auth_limiter_tracks_login_and_signup_independently_for_the_same_key() {
        let limiter = AuthLimiter::new();
        assert!(limiter.check_login("1.2.3.4", 1).await.unwrap());
        assert!(!limiter.check_login("1.2.3.4", 1).await.unwrap());
        // Aynı IP'den signup denemesi login sayacından etkilenmez.
        assert!(limiter.check_signup("1.2.3.4", 1).await.unwrap());
        assert!(!limiter.check_signup("1.2.3.4", 1).await.unwrap());
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

    // --- Redis backend (canlı Redis gerektirir) --------------------------------
    // Bu ortamda local Redis/docker yok (BH-02'nin DB-gated testleriyle aynı
    // durum) — bu testler `#[ignore]`dır, `TEST_REDIS_URL` verilirse elle
    // çalıştırılır: `TEST_REDIS_URL=redis://127.0.0.1:6379 cargo test -- --ignored`.

    async fn redis_conn() -> ConnectionManager {
        let url = std::env::var("TEST_REDIS_URL")
            .expect("TEST_REDIS_URL must be set for #[ignore]d redis tests");
        crate::cache::connect(&url)
            .await
            .expect("connect to TEST_REDIS_URL")
    }

    #[tokio::test]
    #[ignore = "requires TEST_REDIS_URL (a running Redis instance)"]
    async fn redis_backed_limiter_allows_up_to_limit_then_blocks() {
        let key = format!("parity-{}", uuid::Uuid::new_v4());
        let limiter = RateLimiter::with_redis(Duration::from_secs(60), redis_conn().await, "test");
        assert!(limiter.check("rl", &key, 3).await.unwrap());
        assert!(limiter.check("rl", &key, 3).await.unwrap());
        assert!(limiter.check("rl", &key, 3).await.unwrap());
        assert!(!limiter.check("rl", &key, 3).await.unwrap());
    }

    #[tokio::test]
    #[ignore = "requires TEST_REDIS_URL (a running Redis instance)"]
    async fn two_rate_limiter_instances_share_counter_through_redis() {
        let key = format!("shared-{}", uuid::Uuid::new_v4());
        // İki bağımsız limiter, aynı URL + aynı anahtar → ortak üst sınır.
        let a = RateLimiter::with_redis(Duration::from_secs(60), redis_conn().await, "test");
        let b = RateLimiter::with_redis(Duration::from_secs(60), redis_conn().await, "test");
        assert!(a.check("rl", &key, 3).await.unwrap());
        assert!(b.check("rl", &key, 3).await.unwrap());
        assert!(a.check("rl", &key, 3).await.unwrap());
        assert!(!b.check("rl", &key, 3).await.unwrap());
    }

    #[tokio::test]
    #[ignore = "requires TEST_REDIS_URL (a running Redis instance)"]
    async fn ttl_expiry_clears_the_underlying_redis_key() {
        let id = format!("ttl-{}", uuid::Uuid::new_v4());
        let limiter = RateLimiter::with_redis(Duration::from_secs(1), redis_conn().await, "test");
        assert!(limiter.check("rl", &id, 5).await.unwrap());

        let mut conn = redis_conn().await;
        let redis_key = crate::cache::key("test", &["rl", &id]);
        // İlk check TTL kurmuş olmalı.
        let ttl: i64 = redis::cmd("TTL")
            .arg(&redis_key)
            .query_async(&mut conn)
            .await
            .unwrap();
        assert!(ttl > 0 && ttl <= 1, "expected a ~1s TTL, got {ttl}");

        // Pencere dolunca anahtar kendiliğinden silinir.
        tokio::time::sleep(Duration::from_millis(1200)).await;
        let exists: bool = redis::cmd("EXISTS")
            .arg(&redis_key)
            .query_async(&mut conn)
            .await
            .unwrap();
        assert!(!exists, "key should have expired after the window");
    }

    #[tokio::test]
    #[ignore = "requires TEST_REDIS_URL (a running Redis instance)"]
    async fn concurrent_increments_never_exceed_the_limit() {
        use futures_util::future::join_all;
        let key = format!("concurrent-{}", uuid::Uuid::new_v4());
        let limit = 10u32;
        let limiter = std::sync::Arc::new(RateLimiter::with_redis(
            Duration::from_secs(60),
            redis_conn().await,
            "test",
        ));
        let tasks = (0..50).map(|_| {
            let limiter = std::sync::Arc::clone(&limiter);
            let key = key.clone();
            tokio::spawn(async move { limiter.check("rl", &key, limit).await.unwrap() })
        });
        let allowed = join_all(tasks)
            .await
            .into_iter()
            .filter(|r| *r.as_ref().unwrap())
            .count();
        assert_eq!(
            allowed as u32, limit,
            "exactly `limit` calls may be allowed"
        );
    }
}
