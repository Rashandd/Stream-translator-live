//! Çalışma zamanı konfigürasyonu — tüm değerler env var'dan okunur.
//!
//! Hiçbir adres/port koda gömülmez. Eksik env var için makul varsayılan
//! kullanılır; üretimde değerler ortamdan gelir.

use std::net::SocketAddr;

/// Sunucu konfigürasyonu.
#[derive(Debug, Clone)]
pub struct Config {
    /// WebSocket/HTTP sunucusunun bağlanacağı adres.
    pub bind_addr: SocketAddr,
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
    pub fn from_env() -> Result<Self, ConfigError> {
        const BIND_VAR: &str = "BIND_ADDR";
        let raw = std::env::var(BIND_VAR).unwrap_or_else(|_| "0.0.0.0:8080".to_string());
        let bind_addr = raw.parse().map_err(|source| ConfigError::InvalidAddr {
            var: BIND_VAR,
            value: raw,
            source,
        })?;
        Ok(Config { bind_addr })
    }
}
