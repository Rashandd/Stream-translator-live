//! Sabit pencereli, bellek içi per-anahtar rate limiter — `pairing::ClaimLimiter`,
//! `waitlist::WaitlistLimiter` ve `ext_twitch::ExtTwitch::check_rate` ile aynı
//! deseni paylaşan genel amaçlı çekirdek (Faz 1.13: login/signup brute-force
//! koruması için eklendi — `AuthLimiter` burayı sarar).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::http::HeaderMap;

/// Rate map bu boyutu aşarsa süresi geçmiş pencereler ayıklanır.
const DEFAULT_PRUNE_THRESHOLD: usize = 4096;

pub struct RateLimiter {
    rate: Mutex<HashMap<String, (Instant, u32)>>,
    window: Duration,
    prune_threshold: usize,
}

impl RateLimiter {
    pub fn new(window: Duration) -> Self {
        RateLimiter {
            rate: Mutex::new(HashMap::new()),
            window,
            prune_threshold: DEFAULT_PRUNE_THRESHOLD,
        }
    }

    /// `key` için bu pencerede `limit`e kadar izin verir. Kilit zehirlenirse
    /// (panic sonrası) güvenli tarafta kal: reddet.
    pub fn check(&self, key: &str, limit: u32) -> bool {
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

/// `POST /api/auth/login` ve `/api/auth/signup` için istemci başına
/// dakikalık deneme limiti. İki uç ayrı sayaç tutar (aynı IP'den art arda
/// login denemesi signup'ı etkilemez) ama tek `RateLimiter` çekirdeğini
/// anahtar ön ekiyle paylaşır. Limitler `auth.login.rate_limit_per_min` /
/// `auth.signup.rate_limit_per_min` config anahtarlarından okunur — admin
/// panelden restart'sız değiştirilir.
pub struct AuthLimiter {
    inner: RateLimiter,
}

impl AuthLimiter {
    pub fn new() -> Self {
        AuthLimiter {
            inner: RateLimiter::new(Duration::from_secs(60)),
        }
    }

    pub fn check_login(&self, key: &str, limit: u32) -> bool {
        self.inner.check(&format!("login:{key}"), limit)
    }

    pub fn check_signup(&self, key: &str, limit: u32) -> bool {
        self.inner.check(&format!("signup:{key}"), limit)
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

    #[test]
    fn allows_up_to_limit_then_blocks() {
        let limiter = RateLimiter::new(Duration::from_secs(60));
        assert!(limiter.check("1.2.3.4", 3));
        assert!(limiter.check("1.2.3.4", 3));
        assert!(limiter.check("1.2.3.4", 3));
        assert!(!limiter.check("1.2.3.4", 3));
    }

    #[test]
    fn tracks_keys_independently() {
        let limiter = RateLimiter::new(Duration::from_secs(60));
        assert!(limiter.check("1.2.3.4", 1));
        assert!(!limiter.check("1.2.3.4", 1));
        assert!(limiter.check("5.6.7.8", 1));
    }

    #[test]
    fn resets_after_window_elapses() {
        let limiter = RateLimiter::new(Duration::from_millis(20));
        assert!(limiter.check("1.2.3.4", 1));
        assert!(!limiter.check("1.2.3.4", 1));
        std::thread::sleep(Duration::from_millis(30));
        assert!(limiter.check("1.2.3.4", 1));
    }

    #[test]
    fn auth_limiter_tracks_login_and_signup_independently_for_the_same_key() {
        let limiter = AuthLimiter::new();
        assert!(limiter.check_login("1.2.3.4", 1));
        assert!(!limiter.check_login("1.2.3.4", 1));
        // Aynı IP'den signup denemesi login sayacından etkilenmez.
        assert!(limiter.check_signup("1.2.3.4", 1));
        assert!(!limiter.check_signup("1.2.3.4", 1));
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
}
