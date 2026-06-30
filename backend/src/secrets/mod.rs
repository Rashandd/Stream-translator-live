//! Secret yönetimi — API key'leri ve JWT secret koda gömülmez.
//!
//! Faz 1: `EnvSecretStore` (env var). Üretim hedefi: HashiCorp Vault
//! (`VaultSecretStore`, infra'da `vault` servisi sağlanınca aktive edilir).
//! Kod her zaman [`SecretStore`] arkasından okur; kaynak değişse de çağrı aynı.

use async_trait::async_trait;

/// Bilinen secret adları (env-stili anahtarlar).
pub mod names {
    pub const JWT_SECRET: &str = "JWT_SECRET";
    pub const DEEPGRAM_KEY: &str = "DEEPGRAM_KEY";
    pub const DEEPL_KEY: &str = "DEEPL_KEY";
    pub const ELEVENLABS_KEY: &str = "ELEVENLABS_KEY";
}

/// Secret erişim hataları.
#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("secret not found: {0}")]
    NotFound(String),
    #[error("secret backend error: {0}")]
    Backend(String),
}

/// Secret kaynağı soyutlaması.
#[async_trait]
pub trait SecretStore: Send + Sync {
    /// Adı verilen secret'ı döndürür.
    async fn get(&self, name: &str) -> Result<String, SecretError>;
}

/// Env var tabanlı secret store (Faz 1 / local dev).
#[derive(Debug, Default)]
pub struct EnvSecretStore;

#[async_trait]
impl SecretStore for EnvSecretStore {
    async fn get(&self, name: &str) -> Result<String, SecretError> {
        std::env::var(name).map_err(|_| SecretError::NotFound(name.to_string()))
    }
}

/// HashiCorp Vault tabanlı secret store — iskelet.
///
/// `VAULT_ADDR` ve `VAULT_TOKEN` ile yapılandırılır. KV v2 HTTP okuması, infra'da
/// `vault` servisi ve policy'ler sağlanınca eklenecek (Faz 1.2 infra adımı).
/// O zamana dek `EnvSecretStore` kullanılır.
pub struct VaultSecretStore {
    pub addr: String,
    pub token: String,
    pub mount: String,
}

impl VaultSecretStore {
    /// Env'den (`VAULT_ADDR`, `VAULT_TOKEN`) yapılandırır. İkisi de yoksa `None`.
    pub fn from_env() -> Option<Self> {
        let addr = std::env::var("VAULT_ADDR").ok()?;
        let token = std::env::var("VAULT_TOKEN").ok()?;
        let mount = std::env::var("VAULT_MOUNT").unwrap_or_else(|_| "secret".to_string());
        Some(VaultSecretStore { addr, token, mount })
    }
}

#[async_trait]
impl SecretStore for VaultSecretStore {
    async fn get(&self, _name: &str) -> Result<String, SecretError> {
        // Henüz wire edilmedi — Vault KV v2 HTTP okuması infra adımında gelecek.
        Err(SecretError::Backend(
            "vault backend not yet wired; use EnvSecretStore until infra/vault is provisioned"
                .to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn env_store_reads_var() {
        // SAFETY: tek thread'li test, başka test bu var'ı kullanmıyor.
        std::env::set_var("SD_TEST_SECRET_XYZ", "topsecret");
        let store = EnvSecretStore;
        assert_eq!(store.get("SD_TEST_SECRET_XYZ").await.unwrap(), "topsecret");
        std::env::remove_var("SD_TEST_SECRET_XYZ");
    }

    #[tokio::test]
    async fn env_store_missing_is_not_found() {
        let store = EnvSecretStore;
        let err = store.get("SD_DEFINITELY_MISSING_VAR").await.unwrap_err();
        assert!(matches!(err, SecretError::NotFound(_)));
    }

    #[tokio::test]
    async fn vault_skeleton_reports_not_wired() {
        let store = VaultSecretStore {
            addr: "http://vault:8200".into(),
            token: "dev".into(),
            mount: "secret".into(),
        };
        assert!(matches!(store.get("JWT_SECRET").await, Err(SecretError::Backend(_))));
    }
}
