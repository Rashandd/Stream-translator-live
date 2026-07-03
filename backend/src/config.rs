//! Çalışma zamanı konfigürasyonu.
//!
//! İki katman:
//! - [`Config`] — başlangıç/dağıtım değerleri (adres, secret), env var'dan.
//! - [`ServiceConfig`] — sağlayıcı toggle'ları (`stt.provider` vb.). Runtime'da
//!   değişir, restart gerektirmez. Faz 1 iskeletinde bellek içi; ileride
//!   `service_config` tablosundan beslenir.

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::sync::RwLock;

/// Sunucu konfigürasyonu. Secret'lar burada DEĞİL — `secrets::SecretStore`'da.
#[derive(Debug, Clone)]
pub struct Config {
    /// WebSocket/HTTP sunucusunun bağlanacağı adres.
    pub bind_addr: SocketAddr,
    /// PostgreSQL bağlantı dizesi (varsa migration/pool için).
    pub database_url: Option<String>,
}

/// Konfigürasyon yükleme hataları.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("invalid {var}={value}: {source}")]
    InvalidAddr {
        var: &'static str,
        value: String,
        source: std::net::AddrParseError,
    },
}

impl Config {
    /// Konfigürasyonu ortam değişkenlerinden yükler.
    ///
    /// - `BIND_ADDR` — varsayılan `0.0.0.0:8080`
    /// - `DATABASE_URL` — opsiyonel
    pub fn from_env() -> Result<Self, ConfigError> {
        const BIND_VAR: &str = "BIND_ADDR";
        let raw = std::env::var(BIND_VAR).unwrap_or_else(|_| "0.0.0.0:8080".to_string());
        let bind_addr = raw.parse().map_err(|source| ConfigError::InvalidAddr {
            var: BIND_VAR,
            value: raw,
            source,
        })?;

        let database_url = std::env::var("DATABASE_URL").ok();

        Ok(Config {
            bind_addr,
            database_url,
        })
    }
}

/// Runtime sağlayıcı konfigürasyonu — `config::get("stt.provider")` deseni.
///
/// Değerler kod yerine bu store'dan okunur; hiçbir sağlayıcı adı hard-coded
/// değildir. Başlangıç değerleri `docs/schema.md`'deki `service_config`
/// seed'iyle birebirdir.
pub struct ServiceConfig {
    values: RwLock<HashMap<String, String>>,
}

impl ServiceConfig {
    /// `service_config` seed değerleriyle bir store kurar.
    pub fn with_defaults() -> Self {
        let defaults = [
            ("stt.provider", "deepgram"),
            ("stt.fallback", "assemblyai"),
            ("translate.provider", "deepl"),
            ("translate.fallback", "google"),
            ("tts.provider", "disabled"),
            ("stream.max_bitrate", "128000"),
            ("vad.enabled", "true"),
            // Dağıtım yöntemi → plan erişimi. Virgülle ayrılmış plan listesi.
            // Admin panel bunları değiştirir; yayıncı paneli okuyup kilitler.
            ("dist.service.plans", "free,pro"),
            ("dist.cloud.plans", "pro"),
            // Multistream OAuth (1.6, opsiyonel Streamlabs bağlantısı): hangi
            // planda açık + callback redirect tabanı. Admin panelden değiştirilebilir.
            ("integration.streamlabs.plans", "pro"),
            ("integration.youtube.plans", "free,pro"),
            ("integration.redirect_base", "http://localhost:8080/api/integrations"),
            // Stripe (1.11): plan → Stripe Price ID eşlemesi. Boş = henüz Stripe'ta
            // ürün/fiyat oluşturulmadı; checkout o plan için 503 döner. Admin panelden
            // Stripe Dashboard'da oluşturulan gerçek price_id'lerle doldurulur.
            ("billing.plan.free.stripe_price_id", ""),
            ("billing.plan.starter.stripe_price_id", ""),
            ("billing.plan.pro.stripe_price_id", ""),
            ("billing.plan.streamer.stripe_price_id", ""),
            ("billing.success_url", "http://localhost:3000/dashboard/billing?checkout=success"),
            ("billing.cancel_url", "http://localhost:3000/dashboard/billing?checkout=canceled"),
            // Loglama (1.12): asgari seviye admin panelden değişir, restart gerekmez.
            ("log.level", "info"),
            // Sağlayıcı key'i yokken mock'a düşülsün mü? Dev/CI: true.
            // Üretimde false yapılır — key eksikse sessiz mock yerine açık hata.
            ("providers.mock_fallback", "true"),
        ];
        let map = defaults
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        ServiceConfig {
            values: RwLock::new(map),
        }
    }

    /// Bir anahtarın değerini okur.
    pub fn get(&self, key: &str) -> Option<String> {
        self.values.read().ok().and_then(|m| m.get(key).cloned())
    }

    /// Bir anahtarı günceller (admin toggle). Restart gerekmez.
    pub fn set(&self, key: impl Into<String>, value: impl Into<String>) {
        if let Ok(mut m) = self.values.write() {
            m.insert(key.into(), value.into());
        }
    }

    /// Yalnızca seed'de tanımlı (bilinen) bir anahtarı günceller.
    ///
    /// Admin API'sinin gelişigüzel anahtar enjekte etmesini engeller —
    /// şema seed ile sabittir. Anahtar yoksa `false` döner, hiçbir şey yazılmaz.
    pub fn set_existing(&self, key: &str, value: impl Into<String>) -> bool {
        match self.values.write() {
            Ok(mut m) if m.contains_key(key) => {
                m.insert(key.to_string(), value.into());
                true
            }
            _ => false,
        }
    }

    /// Tüm config'in sıralı bir kopyasını döndürür (deterministik JSON için).
    pub fn snapshot(&self) -> BTreeMap<String, String> {
        self.values
            .read()
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default()
    }
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_provider_defaults() {
        let cfg = ServiceConfig::with_defaults();
        assert_eq!(cfg.get("stt.provider").as_deref(), Some("deepgram"));
        assert_eq!(cfg.get("translate.provider").as_deref(), Some("deepl"));
        assert_eq!(cfg.get("tts.provider").as_deref(), Some("disabled"));
    }

    #[test]
    fn set_overrides_without_restart() {
        let cfg = ServiceConfig::with_defaults();
        cfg.set("stt.provider", "whisper");
        assert_eq!(cfg.get("stt.provider").as_deref(), Some("whisper"));
    }

    #[test]
    fn missing_key_is_none() {
        let cfg = ServiceConfig::with_defaults();
        assert_eq!(cfg.get("nonexistent"), None);
    }

    #[test]
    fn set_existing_updates_known_key() {
        let cfg = ServiceConfig::with_defaults();
        assert!(cfg.set_existing("stt.provider", "whisper"));
        assert_eq!(cfg.get("stt.provider").as_deref(), Some("whisper"));
    }

    #[test]
    fn set_existing_rejects_unknown_key() {
        let cfg = ServiceConfig::with_defaults();
        assert!(!cfg.set_existing("attacker.key", "x"));
        assert_eq!(cfg.get("attacker.key"), None);
    }

    #[test]
    fn snapshot_is_sorted_and_complete() {
        let cfg = ServiceConfig::with_defaults();
        let snap = cfg.snapshot();
        assert_eq!(snap.get("dist.service.plans").map(String::as_str), Some("free,pro"));
        assert_eq!(snap.get("dist.cloud.plans").map(String::as_str), Some("pro"));
        // BTreeMap garantili sıralı.
        let keys: Vec<&String> = snap.keys().collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted);
    }
}
