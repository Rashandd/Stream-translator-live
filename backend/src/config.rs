//! Çalışma zamanı konfigürasyonu.
//!
//! İki katman:
//! - [`Config`] — başlangıç/dağıtım değerleri (adres, secret), env var'dan.
//! - [`ServiceConfig`] — sağlayıcı toggle'ları (`stt.provider` vb.). Runtime'da
//!   değişir, restart gerektirmez. Faz 1 iskeletinde bellek içi; ileride
//!   `service_config` tablosundan beslenir.

use std::collections::HashMap;
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
}
