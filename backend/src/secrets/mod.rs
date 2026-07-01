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
    pub const ASSEMBLYAI_KEY: &str = "ASSEMBLYAI_KEY";
    pub const DEEPL_KEY: &str = "DEEPL_KEY";
    pub const GOOGLE_TRANSLATE_KEY: &str = "GOOGLE_TRANSLATE_KEY";
    pub const ELEVENLABS_KEY: &str = "ELEVENLABS_KEY";

    // Multistream OAuth istemci kimlikleri (Faz 1.6, opsiyonel Streamlabs bağlantısı).
    // Client secret koda gömülmez.
    pub const STREAMLABS_CLIENT_ID: &str = "STREAMLABS_CLIENT_ID";
    pub const STREAMLABS_CLIENT_SECRET: &str = "STREAMLABS_CLIENT_SECRET";

    // YouTube Native CC (Faz 1.10) — OAuth istemci kimlikleri.
    pub const YOUTUBE_CLIENT_ID: &str = "YOUTUBE_CLIENT_ID";
    pub const YOUTUBE_CLIENT_SECRET: &str = "YOUTUBE_CLIENT_SECRET";

    // Stripe (Faz 1.11). Secret key sunucu-taraflı API çağrıları için;
    // webhook secret imza doğrulaması için. Koda asla gömülmez.
    pub const STRIPE_SECRET_KEY: &str = "STRIPE_SECRET_KEY";
    pub const STRIPE_WEBHOOK_SECRET: &str = "STRIPE_WEBHOOK_SECRET";
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

/// HashiCorp Vault (KV v2) tabanlı secret store.
///
/// `VAULT_ADDR` ve `VAULT_TOKEN` ile yapılandırılır (`VAULT_MOUNT` opsiyonel,
/// varsayılan `secret`). Her secret KV v2'de `<name>` yolunda, `value` alanında
/// saklanır; okuma `GET {addr}/v1/{mount}/data/{name}` ile yapılır ve
/// `.data.data.value` döndürülür. `EnvSecretStore` ile aynı arayüz — çağıran kod
/// değişmez.
pub struct VaultSecretStore {
    addr: String,
    token: String,
    mount: String,
    client: reqwest::Client,
}

impl VaultSecretStore {
    /// Verilen bağlantı bilgileriyle bir Vault store kurar.
    pub fn new(addr: impl Into<String>, token: impl Into<String>, mount: impl Into<String>) -> Self {
        VaultSecretStore {
            addr: addr.into(),
            token: token.into(),
            mount: mount.into(),
            client: reqwest::Client::new(),
        }
    }

    /// Env'den (`VAULT_ADDR`, `VAULT_TOKEN`) yapılandırır. İkisi de yoksa `None`.
    pub fn from_env() -> Option<Self> {
        let addr = std::env::var("VAULT_ADDR").ok()?;
        let token = std::env::var("VAULT_TOKEN").ok()?;
        let mount = std::env::var("VAULT_MOUNT").unwrap_or_else(|_| "secret".to_string());
        Some(VaultSecretStore::new(addr, token, mount))
    }
}

#[async_trait]
impl SecretStore for VaultSecretStore {
    async fn get(&self, name: &str) -> Result<String, SecretError> {
        let url = format!(
            "{}/v1/{}/data/{}",
            self.addr.trim_end_matches('/'),
            self.mount,
            name
        );

        let resp = self
            .client
            .get(&url)
            .header("X-Vault-Token", &self.token)
            .send()
            .await
            .map_err(|e| SecretError::Backend(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(SecretError::NotFound(name.to_string()));
        }
        if !resp.status().is_success() {
            return Err(SecretError::Backend(format!(
                "vault returned status {}",
                resp.status()
            )));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SecretError::Backend(e.to_string()))?;

        body["data"]["data"]["value"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| SecretError::NotFound(name.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::Path;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::get;
    use axum::{Json, Router};

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

    /// KV v2 okuma davranışını taklit eden yerel bir Vault mock'u başlatır.
    /// Sadece `JWT_SECRET` yolunu ve doğru token'ı tanır.
    async fn spawn_vault_mock() -> String {
        async fn read(headers: HeaderMap, Path((_mount, name)): Path<(String, String)>) -> (StatusCode, Json<serde_json::Value>) {
            if headers.get("X-Vault-Token").and_then(|v| v.to_str().ok()) != Some("dev-token") {
                return (StatusCode::FORBIDDEN, Json(serde_json::json!({"errors": ["permission denied"]})));
            }
            if name == "JWT_SECRET" {
                return (
                    StatusCode::OK,
                    Json(serde_json::json!({ "data": { "data": { "value": "s3cr3t" }, "metadata": {} } })),
                );
            }
            (StatusCode::NOT_FOUND, Json(serde_json::json!({"errors": []})))
        }

        let app = Router::new().route("/v1/{mount}/data/{name}", get(read));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn vault_reads_kv_v2_value() {
        let addr = spawn_vault_mock().await;
        let store = VaultSecretStore::new(addr, "dev-token", "secret");
        assert_eq!(store.get("JWT_SECRET").await.unwrap(), "s3cr3t");
    }

    #[tokio::test]
    async fn vault_missing_path_is_not_found() {
        let addr = spawn_vault_mock().await;
        let store = VaultSecretStore::new(addr, "dev-token", "secret");
        let err = store.get("NOPE").await.unwrap_err();
        assert!(matches!(err, SecretError::NotFound(_)));
    }

    #[tokio::test]
    async fn vault_wrong_token_is_backend_error() {
        let addr = spawn_vault_mock().await;
        let store = VaultSecretStore::new(addr, "bad-token", "secret");
        let err = store.get("JWT_SECRET").await.unwrap_err();
        assert!(matches!(err, SecretError::Backend(_)));
    }
}
