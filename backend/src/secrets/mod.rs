//! Secret yönetimi — API key'leri ve JWT secret koda gömülmez.
//!
//! Katmanlar:
//! - `EnvSecretStore` (env var, local dev) / `VaultSecretStore` (KV v2) — taban.
//! - [`ManagedSecrets`] — tabanın üstünde admin panelden girilen runtime
//!   override'ları tutar; `SECRETS_FILE` verilirse AES-256-GCM ile şifreli
//!   kalıcıdır. Panelden girilen key restart gerektirmeden devreye girer.
//!
//! Kod her zaman [`SecretStore`] arkasından okur; kaynak değişse de çağrı aynı.
//! API'ye tam secret değeri asla dönmez — sadece baş+son maskeli önizleme
//! ([`mask`]).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use serde::Serialize;

use crate::crypto::Cipher;

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

    // SMTP (2FA/işlem mailleri) — panelden girilir, restart gerekmez.
    pub const SMTP_HOST: &str = "SMTP_HOST";
    pub const SMTP_PORT: &str = "SMTP_PORT";
    pub const SMTP_USERNAME: &str = "SMTP_USERNAME";
    pub const SMTP_PASSWORD: &str = "SMTP_PASSWORD";
    pub const MAIL_FROM: &str = "MAIL_FROM";

    /// Admin panelden yönetilebilen secret'lar. `JWT_SECRET` bilinçli olarak
    /// dışarıda: runtime'da değişmesi tüm oturumları ve secrets dosyası
    /// anahtar türetimini geçersiz kılar — o, deploy zamanı env/Vault işidir.
    pub const MANAGED: &[&str] = &[
        DEEPGRAM_KEY,
        ASSEMBLYAI_KEY,
        DEEPL_KEY,
        GOOGLE_TRANSLATE_KEY,
        ELEVENLABS_KEY,
        STREAMLABS_CLIENT_ID,
        STREAMLABS_CLIENT_SECRET,
        YOUTUBE_CLIENT_ID,
        YOUTUBE_CLIENT_SECRET,
        STRIPE_SECRET_KEY,
        STRIPE_WEBHOOK_SECRET,
        SMTP_HOST,
        SMTP_PORT,
        SMTP_USERNAME,
        SMTP_PASSWORD,
        MAIL_FROM,
    ];
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

/// Bellek içi secret store — test ve sabit kurulumlar için.
#[derive(Debug, Default)]
pub struct MemorySecretStore {
    values: RwLock<HashMap<String, String>>,
}

impl MemorySecretStore {
    /// Boş bir store kurar.
    pub fn new() -> Self {
        Self::default()
    }

    /// `(ad, değer)` çiftleriyle kurar.
    pub fn with(values: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>) -> Self {
        let map = values
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();
        MemorySecretStore {
            values: RwLock::new(map),
        }
    }

    /// Bir değeri ekler/günceller.
    pub fn insert(&self, name: impl Into<String>, value: impl Into<String>) {
        if let Ok(mut m) = self.values.write() {
            m.insert(name.into(), value.into());
        }
    }
}

#[async_trait]
impl SecretStore for MemorySecretStore {
    async fn get(&self, name: &str) -> Result<String, SecretError> {
        self.values
            .read()
            .ok()
            .and_then(|m| m.get(name).cloned())
            .ok_or_else(|| SecretError::NotFound(name.to_string()))
    }
}

/// Çağrı anında çözümlenen secret referansı.
///
/// Sağlayıcı client'ları key'i yapım anında değil her istekte çözer — admin
/// panelden girilen yeni key restart gerektirmeden devreye girer.
#[derive(Clone)]
pub enum SecretRef {
    /// Sabit değer (test/manuel kurulum). `None` = yapılandırılmamış.
    Fixed(Option<String>),
    /// Her çağrıda store'dan okunur.
    Store {
        store: Arc<dyn SecretStore>,
        name: String,
    },
}

impl SecretRef {
    /// Sabit bir değerle kurar.
    pub fn fixed(value: impl Into<String>) -> Self {
        SecretRef::Fixed(Some(value.into()))
    }

    /// Hiç yapılandırılmamış referans.
    pub fn none() -> Self {
        SecretRef::Fixed(None)
    }

    /// Store'a bağlı referans.
    pub fn store(store: Arc<dyn SecretStore>, name: impl Into<String>) -> Self {
        SecretRef::Store {
            store,
            name: name.into(),
        }
    }

    /// Geçerli değeri çözer. Yok/boşsa `None`.
    pub async fn resolve(&self) -> Option<String> {
        let value = match self {
            SecretRef::Fixed(v) => v.clone(),
            SecretRef::Store { store, name } => store.get(name).await.ok(),
        };
        value.filter(|v| !v.is_empty())
    }
}

/// Bir secret değerinin sadece başını ve sonunu gösteren maske.
///
/// Panel tam değeri asla görmez: uzun değerlerde ilk 4 + son 4 karakter,
/// kısa değerlerde daha azı, çok kısa değerlerde hiçbir şey görünür.
pub fn mask(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    let (head, tail) = match chars.len() {
        0..=7 => return "••••".to_string(),
        8..=11 => (2, 2),
        _ => (4, 4),
    };
    let start: String = chars[..head].iter().collect();
    let end: String = chars[chars.len() - tail..].iter().collect();
    format!("{start}…{end}")
}

/// Panele dönen secret durumu. **Değerin kendisi asla yer almaz.**
#[derive(Debug, Clone, Serialize)]
pub struct SecretStatus {
    pub name: String,
    pub configured: bool,
    /// `panel` (admin girdisi) | taban etiketi (`env`/`vault`) | `unset`.
    pub source: String,
    /// Baş+son maskeli önizleme; yapılandırılmamışsa `None`.
    pub masked: Option<String>,
}

/// Şifreli kalıcılık dosyası (`SECRETS_FILE`).
struct PersistFile {
    path: PathBuf,
    cipher: Cipher,
}

/// Taban store + admin panel override'ları.
///
/// Okuma önceliği: panel override → taban (env/Vault). Yazma sadece
/// [`names::MANAGED`] listesindeki adlara izinlidir; kalıcılık dosyası
/// AES-256-GCM ile şifrelidir (`base64(nonce || ciphertext)` içinde JSON map).
pub struct ManagedSecrets {
    base: Box<dyn SecretStore>,
    /// Taban kaynağının paneldeki etiketi ("env" | "vault").
    base_label: &'static str,
    overrides: RwLock<HashMap<String, String>>,
    persist: Option<PersistFile>,
}

impl ManagedSecrets {
    /// Kalıcılık olmadan kurar (override'lar restart'ta kaybolur).
    pub fn new(base: Box<dyn SecretStore>, base_label: &'static str) -> Self {
        ManagedSecrets {
            base,
            base_label,
            overrides: RwLock::new(HashMap::new()),
            persist: None,
        }
    }

    /// Env tabanlı, kalıcılıksız store — test ve iskelet kurulumları için.
    pub fn env() -> Self {
        ManagedSecrets::new(Box::new(EnvSecretStore), "env")
    }

    /// Şifreli dosya kalıcılığıyla kurar; dosya varsa override'ları yükler.
    pub fn with_file(
        base: Box<dyn SecretStore>,
        base_label: &'static str,
        path: impl Into<PathBuf>,
        key: &[u8],
    ) -> Result<Self, SecretError> {
        let path = path.into();
        let cipher = Cipher::new(key).map_err(|e| SecretError::Backend(e.to_string()))?;
        let overrides = load_overrides(&path, &cipher)?;
        Ok(ManagedSecrets {
            base,
            base_label,
            overrides: RwLock::new(overrides),
            persist: Some(PersistFile { path, cipher }),
        })
    }

    /// Ad panelden yönetilebilir mi?
    pub fn is_managed(name: &str) -> bool {
        names::MANAGED.contains(&name)
    }

    /// Bir secret'ı panelden ayarlar. Bilinmeyen ad veya boş değer reddedilir.
    pub fn set(&self, name: &str, value: &str) -> Result<(), SecretError> {
        if !Self::is_managed(name) {
            return Err(SecretError::NotFound(name.to_string()));
        }
        if value.trim().is_empty() {
            return Err(SecretError::Backend("empty secret value".to_string()));
        }
        {
            let mut map = self
                .overrides
                .write()
                .map_err(|_| SecretError::Backend("secrets lock poisoned".to_string()))?;
            map.insert(name.to_string(), value.to_string());
        }
        self.save()
    }

    /// Panel override'ını kaldırır (taban kaynağa geri düşülür).
    /// Override yoksa `Ok(false)` döner.
    pub fn clear(&self, name: &str) -> Result<bool, SecretError> {
        if !Self::is_managed(name) {
            return Err(SecretError::NotFound(name.to_string()));
        }
        let removed = {
            let mut map = self
                .overrides
                .write()
                .map_err(|_| SecretError::Backend("secrets lock poisoned".to_string()))?;
            map.remove(name).is_some()
        };
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    /// Tüm yönetilen secret'ların paneldeki durumu (maskeli, değer yok).
    pub async fn status(&self) -> Vec<SecretStatus> {
        let mut out = Vec::with_capacity(names::MANAGED.len());
        for &name in names::MANAGED {
            let override_value = self
                .overrides
                .read()
                .ok()
                .and_then(|m| m.get(name).cloned());
            let (source, value) = match override_value {
                Some(v) => ("panel".to_string(), Some(v)),
                None => match self.base.get(name).await.ok().filter(|v| !v.is_empty()) {
                    Some(v) => (self.base_label.to_string(), Some(v)),
                    None => ("unset".to_string(), None),
                },
            };
            out.push(SecretStatus {
                name: name.to_string(),
                configured: value.is_some(),
                source,
                masked: value.as_deref().map(mask),
            });
        }
        out
    }

    /// Override map'ini şifreleyip dosyaya yazar (tmp + rename, yarım yazım yok).
    fn save(&self) -> Result<(), SecretError> {
        let Some(persist) = &self.persist else {
            return Ok(());
        };
        let snapshot: BTreeMap<String, String> = self
            .overrides
            .read()
            .map_err(|_| SecretError::Backend("secrets lock poisoned".to_string()))?
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let json = serde_json::to_string(&snapshot)
            .map_err(|e| SecretError::Backend(e.to_string()))?;
        let sealed = persist
            .cipher
            .encrypt_str(&json)
            .map_err(|e| SecretError::Backend(e.to_string()))?;

        let tmp = persist.path.with_extension("tmp");
        std::fs::write(&tmp, sealed).map_err(|e| SecretError::Backend(e.to_string()))?;
        std::fs::rename(&tmp, &persist.path).map_err(|e| SecretError::Backend(e.to_string()))?;
        Ok(())
    }
}

/// Şifreli dosyadan override map'ini yükler. Dosya yoksa boş map.
fn load_overrides(path: &Path, cipher: &Cipher) -> Result<HashMap<String, String>, SecretError> {
    let sealed = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(e) => return Err(SecretError::Backend(e.to_string())),
    };
    let json = cipher
        .decrypt_str(sealed.trim())
        .map_err(|e| SecretError::Backend(format!("secrets file unreadable: {e}")))?;
    serde_json::from_str(&json).map_err(|e| SecretError::Backend(e.to_string()))
}

#[async_trait]
impl SecretStore for ManagedSecrets {
    async fn get(&self, name: &str) -> Result<String, SecretError> {
        let override_value = self
            .overrides
            .read()
            .ok()
            .and_then(|m| m.get(name).cloned());
        match override_value {
            Some(v) => Ok(v),
            None => self.base.get(name).await,
        }
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

    // --- mask ---

    #[test]
    fn mask_shows_only_head_and_tail() {
        assert_eq!(mask("sk_live_abcdef123456"), "sk_l…3456");
        assert_eq!(mask("12345678"), "12…78");
    }

    #[test]
    fn mask_hides_short_values_completely() {
        assert_eq!(mask(""), "••••");
        assert_eq!(mask("short"), "••••");
        assert_eq!(mask("1234567"), "••••");
    }

    #[test]
    fn mask_handles_multibyte_chars() {
        // 12 karakter (çok byte'lı) — panic'siz baş+son.
        assert_eq!(mask("ğüşiöçĞÜŞİÖÇ"), "ğüşi…ŞİÖÇ");
    }

    // --- SecretRef ---

    #[tokio::test]
    async fn secret_ref_resolves_fixed_store_and_empty() {
        assert_eq!(SecretRef::fixed("abc").resolve().await.as_deref(), Some("abc"));
        assert_eq!(SecretRef::none().resolve().await, None);
        // Boş string yapılandırılmamış sayılır (.env'de KEY= bırakılması).
        assert_eq!(SecretRef::Fixed(Some(String::new())).resolve().await, None);

        let store: Arc<dyn SecretStore> =
            Arc::new(MemorySecretStore::with([("DEEPL_KEY", "k-123456789")]));
        let r = SecretRef::store(Arc::clone(&store), "DEEPL_KEY");
        assert_eq!(r.resolve().await.as_deref(), Some("k-123456789"));
        assert_eq!(SecretRef::store(store, "MISSING").resolve().await, None);
    }

    // --- ManagedSecrets ---

    fn managed_with_env_base() -> ManagedSecrets {
        ManagedSecrets::new(Box::new(MemorySecretStore::new()), "env")
    }

    #[tokio::test]
    async fn managed_set_overrides_and_get_prefers_override() {
        let base = MemorySecretStore::with([(names::DEEPGRAM_KEY, "from-env-value")]);
        let ms = ManagedSecrets::new(Box::new(base), "env");
        assert_eq!(ms.get(names::DEEPGRAM_KEY).await.unwrap(), "from-env-value");

        ms.set(names::DEEPGRAM_KEY, "panel-value-123").unwrap();
        assert_eq!(ms.get(names::DEEPGRAM_KEY).await.unwrap(), "panel-value-123");

        assert!(ms.clear(names::DEEPGRAM_KEY).unwrap());
        assert_eq!(ms.get(names::DEEPGRAM_KEY).await.unwrap(), "from-env-value");
        assert!(!ms.clear(names::DEEPGRAM_KEY).unwrap());
    }

    #[test]
    fn managed_rejects_unknown_name_and_empty_value() {
        let ms = managed_with_env_base();
        assert!(matches!(
            ms.set("JWT_SECRET", "x"),
            Err(SecretError::NotFound(_))
        ));
        assert!(matches!(
            ms.set("ATTACKER_KEY", "x"),
            Err(SecretError::NotFound(_))
        ));
        assert!(matches!(
            ms.set(names::DEEPL_KEY, "   "),
            Err(SecretError::Backend(_))
        ));
        assert!(matches!(
            ms.clear("ATTACKER_KEY"),
            Err(SecretError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn managed_status_masks_and_never_leaks_value() {
        let base = MemorySecretStore::with([(names::DEEPL_KEY, "env-key-abcdef")]);
        let ms = ManagedSecrets::new(Box::new(base), "env");
        ms.set(names::DEEPGRAM_KEY, "dg_secret_123456").unwrap();

        let status = ms.status().await;
        let by_name = |n: &str| status.iter().find(|s| s.name == n).unwrap();

        let dg = by_name(names::DEEPGRAM_KEY);
        assert!(dg.configured);
        assert_eq!(dg.source, "panel");
        assert_eq!(dg.masked.as_deref(), Some("dg_s…3456"));

        let dl = by_name(names::DEEPL_KEY);
        assert!(dl.configured);
        assert_eq!(dl.source, "env");
        assert_eq!(dl.masked.as_deref(), Some("env-…cdef"));

        let st = by_name(names::STRIPE_SECRET_KEY);
        assert!(!st.configured);
        assert_eq!(st.source, "unset");
        assert_eq!(st.masked, None);

        // Hiçbir status alanında tam değer geçmemeli.
        let json = serde_json::to_string(&status).unwrap();
        assert!(!json.contains("dg_secret_123456"));
        assert!(!json.contains("env-key-abcdef"));
    }

    #[tokio::test]
    async fn managed_persists_encrypted_and_reloads() {
        let dir = std::env::temp_dir().join(format!("sd-secrets-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("secrets.enc");
        let _ = std::fs::remove_file(&path);
        let key = crate::crypto::derive_key(b"test-jwt", "streamdubber.secrets.v1");

        {
            let ms = ManagedSecrets::with_file(
                Box::new(MemorySecretStore::new()),
                "env",
                &path,
                &key,
            )
            .unwrap();
            ms.set(names::STRIPE_SECRET_KEY, "sk_live_persist_me").unwrap();
        }

        // Dosya düz metin secret içermemeli.
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("sk_live_persist_me"));
        assert!(!raw.contains(names::STRIPE_SECRET_KEY));

        // Aynı anahtar ile yeni instance override'ı geri yükler.
        let reloaded = ManagedSecrets::with_file(
            Box::new(MemorySecretStore::new()),
            "env",
            &path,
            &key,
        )
        .unwrap();
        assert_eq!(
            reloaded.get(names::STRIPE_SECRET_KEY).await.unwrap(),
            "sk_live_persist_me"
        );

        // Yanlış anahtar ile açılmamalı.
        let wrong = crate::crypto::derive_key(b"other-jwt", "streamdubber.secrets.v1");
        assert!(ManagedSecrets::with_file(
            Box::new(MemorySecretStore::new()),
            "env",
            &path,
            &wrong,
        )
        .is_err());

        let _ = std::fs::remove_file(&path);
    }
}
