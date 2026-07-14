//! Redis bağlantı yöneticisi (BH-04).
//!
//! `db::mod`'un şeklini yansıtır: açık bir `REDIS_URL` verildiğinde bağlanır,
//! ulaşılamıyorsa `?` ile startup'ı düşürür (fail-fast, tıpkı `db::connect`
//! gibi). `REDIS_URL` yoksa çağıran bellek-içi yola düşer (tek-instance).
//!
//! Ephemeral/paylaşımlı state'in Redis'e taşınması planı için bkz.
//! `docs/adr/0001-production-state-ownership.md` ve
//! `docs/backend-hardening-captions-plan.md` (BH-04). Bu ilk artışta yalnız
//! rate limiter'lar Redis'e taşınır.

use redis::aio::ConnectionManager;

/// Redis (cache) katmanı hataları.
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("redis connection failed: {0}")]
    Connect(#[from] redis::RedisError),
}

/// `REDIS_URL` için bir `ConnectionManager` kurar (ilk bağlantı hemen denenir).
///
/// `db::connect` gibi fail-fast: verilmiş ama ulaşılamayan bir `REDIS_URL`
/// startup'ı iptal eder. `ConnectionManager`'ın kendisi sonraki geçici
/// kopmalarda kendiliğinden yeniden bağlanır.
pub async fn connect(redis_url: &str) -> Result<ConnectionManager, CacheError> {
    let client = redis::Client::open(redis_url)?;
    Ok(client.get_connection_manager().await?)
}

/// Redis anahtar ad-alanı: `streamdubber:{env}:{parts...}` (plan §15 formatı).
///
/// `env` segmenti `Environment::as_label()`'dan gelir — namespace için tek
/// kaynak (bkz. `config::Environment::as_label`).
pub fn key(env: &str, parts: &[&str]) -> String {
    let mut out = format!("streamdubber:{env}");
    for part in parts {
        out.push(':');
        out.push_str(part);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_builds_namespaced_colon_path() {
        assert_eq!(
            key("production", &["auth-login", "1.2.3.4"]),
            "streamdubber:production:auth-login:1.2.3.4"
        );
        // Segment yoksa yalnız ad-alanı öneki döner.
        assert_eq!(key("development", &[]), "streamdubber:development");
    }

    #[tokio::test]
    async fn unreachable_redis_url_fails_fast_at_connect() {
        // Canlı Redis gerektirmez: kapalı bir porta (127.0.0.1:1) bağlanmaya
        // çalışır. `ConnectionManager` ilk bağlantıyı deneyip Err döndürür;
        // startup bu Err'i `?` ile yukarı taşır (fail-fast, sessiz-geçiş yok).
        let result = connect("redis://127.0.0.1:1").await;
        assert!(
            result.is_err(),
            "unreachable redis url must return Err, not hang or succeed"
        );
    }
}
