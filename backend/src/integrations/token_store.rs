//! OAuth token'larının AES-256-GCM şifreli kalıcılığı.
//!
//! Doğrulama listesi karşılıkları: "Streamlabs OAuth — token persist" ve
//! "AES-256 şifrelemesinin `_encrypted` kolonlara uygulanması"nın token
//! ayağı. Kayıtlar `TOKENS_FILE`'da tek şifreli blob olarak saklanır
//! (`base64(nonce || ciphertext)` içinde JSON map) — düz metin token diske
//! asla yazılmaz. DB `platform_accounts` tablosuna geçiş, `users` FK'sı
//! gerektirdiği için kullanıcı sistemiyle (1.4) birlikte yapılır; arayüz
//! aynı kalır.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::crypto::Cipher;

use super::oauth::TokenResponse;

/// Kalıcı token kaydı. `expires_at` unix saniye (`expires_in`'den hesaplanır).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredToken {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_at: Option<i64>,
}

/// Token depolama hataları.
#[derive(Debug, thiserror::Error)]
pub enum TokenStoreError {
    #[error("token store io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("token store crypto error: {0}")]
    Crypto(String),
    #[error("token store serde error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// (kullanıcı, sağlayıcı) → şifreli token deposu.
pub struct TokenVault {
    entries: RwLock<HashMap<String, StoredToken>>,
    persist: Option<(PathBuf, Cipher)>,
}

fn entry_key(user_id: &str, provider: &str) -> String {
    // '\n' ne kullanıcı kimliğinde ne sağlayıcı adında geçer — çakışmasız ayraç.
    format!("{user_id}\n{provider}")
}

impl TokenVault {
    /// Kalıcılıksız depo (dev/test — restart'ta kaybolur).
    pub fn in_memory() -> Self {
        TokenVault {
            entries: RwLock::new(HashMap::new()),
            persist: None,
        }
    }

    /// Şifreli dosya kalıcılığıyla kurar; dosya varsa kayıtları yükler.
    pub fn with_file(path: impl Into<PathBuf>, key: &[u8]) -> Result<Self, TokenStoreError> {
        let path = path.into();
        let cipher = Cipher::new(key).map_err(|e| TokenStoreError::Crypto(e.to_string()))?;
        let entries = load_entries(&path, &cipher)?;
        Ok(TokenVault {
            entries: RwLock::new(entries),
            persist: Some((path, cipher)),
        })
    }

    /// Token değişimi sonucunu saklar (varsa eskisinin üstüne yazar).
    pub fn store(
        &self,
        user_id: &str,
        provider: &str,
        token: &TokenResponse,
    ) -> Result<(), TokenStoreError> {
        let stored = StoredToken {
            access_token: token.access_token.clone(),
            refresh_token: token.refresh_token.clone(),
            expires_at: token
                .expires_in
                .map(|secs| chrono::Utc::now().timestamp() + secs as i64),
        };
        {
            let mut map = self
                .entries
                .write()
                .map_err(|_| TokenStoreError::Crypto("token store lock poisoned".into()))?;
            map.insert(entry_key(user_id, provider), stored);
        }
        self.save()
    }

    /// Bir refresh-token değişiminin sonucunu saklar. Sağlayıcı yeni bir
    /// `refresh_token` döndürmediyse (Google'ın tipik davranışı) eskisini
    /// korur — aksi halde düz `store()` çağrısı sessizce eldeki refresh
    /// token'ı `None`'a düşürür ve bir sonraki yenileme imkansızlaşır.
    pub fn store_refreshed(
        &self,
        user_id: &str,
        provider: &str,
        new_token: &TokenResponse,
    ) -> Result<(), TokenStoreError> {
        let previous_refresh = self.get(user_id, provider).and_then(|t| t.refresh_token);
        let stored = StoredToken {
            access_token: new_token.access_token.clone(),
            refresh_token: new_token.refresh_token.clone().or(previous_refresh),
            expires_at: new_token
                .expires_in
                .map(|secs| chrono::Utc::now().timestamp() + secs as i64),
        };
        {
            let mut map = self
                .entries
                .write()
                .map_err(|_| TokenStoreError::Crypto("token store lock poisoned".into()))?;
            map.insert(entry_key(user_id, provider), stored);
        }
        self.save()
    }

    /// Kayıtlı token (varsa). Çağıran süresini `expires_at` ile denetler.
    pub fn get(&self, user_id: &str, provider: &str) -> Option<StoredToken> {
        self.entries
            .read()
            .ok()
            .and_then(|m| m.get(&entry_key(user_id, provider)).cloned())
    }

    /// Yayıncının bu sağlayıcıya bağlı görünüp görünmeyeceği.
    pub fn connected(&self, user_id: &str, provider: &str) -> bool {
        self.get(user_id, provider).is_some()
    }

    /// Bağlantıyı kaldırır (kayıt siler). Kayıt yoksa `Ok(false)`.
    pub fn remove(&self, user_id: &str, provider: &str) -> Result<bool, TokenStoreError> {
        let removed = {
            let mut map = self
                .entries
                .write()
                .map_err(|_| TokenStoreError::Crypto("token store lock poisoned".into()))?;
            map.remove(&entry_key(user_id, provider)).is_some()
        };
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    /// Tüm map'i şifreleyip dosyaya yazar (tmp + rename).
    fn save(&self) -> Result<(), TokenStoreError> {
        let Some((path, cipher)) = &self.persist else {
            return Ok(());
        };
        let snapshot: HashMap<String, StoredToken> = self
            .entries
            .read()
            .map_err(|_| TokenStoreError::Crypto("token store lock poisoned".into()))?
            .clone();
        let json = serde_json::to_string(&snapshot)?;
        let sealed = cipher
            .encrypt_str(&json)
            .map_err(|e| TokenStoreError::Crypto(e.to_string()))?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, sealed)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

fn load_entries(
    path: &Path,
    cipher: &Cipher,
) -> Result<HashMap<String, StoredToken>, TokenStoreError> {
    let sealed = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(e) => return Err(TokenStoreError::Io(e)),
    };
    let json = cipher
        .decrypt_str(sealed.trim())
        .map_err(|e| TokenStoreError::Crypto(format!("tokens file unreadable: {e}")))?;
    Ok(serde_json::from_str(&json)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(access: &str) -> TokenResponse {
        TokenResponse {
            access_token: access.into(),
            refresh_token: Some(format!("refresh-{access}")),
            expires_in: Some(3600),
            token_type: Some("Bearer".into()),
        }
    }

    #[test]
    fn in_memory_store_and_connected() {
        let vault = TokenVault::in_memory();
        assert!(!vault.connected("u1", "streamlabs"));
        vault.store("u1", "streamlabs", &token("at-1")).unwrap();
        assert!(vault.connected("u1", "streamlabs"));
        assert!(!vault.connected("u1", "youtube"));
        assert!(!vault.connected("u2", "streamlabs"));

        let stored = vault.get("u1", "streamlabs").unwrap();
        assert_eq!(stored.access_token, "at-1");
        assert_eq!(stored.refresh_token.as_deref(), Some("refresh-at-1"));
        assert!(stored.expires_at.unwrap() > chrono::Utc::now().timestamp());

        assert!(vault.remove("u1", "streamlabs").unwrap());
        assert!(!vault.connected("u1", "streamlabs"));
        assert!(!vault.remove("u1", "streamlabs").unwrap());
    }

    #[test]
    fn store_refreshed_keeps_old_refresh_token_when_response_omits_it() {
        let vault = TokenVault::in_memory();
        vault.store("u1", "youtube", &token("at-1")).unwrap();
        let original_refresh = vault.get("u1", "youtube").unwrap().refresh_token;
        assert!(original_refresh.is_some());

        let refreshed = TokenResponse {
            access_token: "at-2".into(),
            refresh_token: None,
            expires_in: Some(3600),
            token_type: Some("Bearer".into()),
        };
        vault.store_refreshed("u1", "youtube", &refreshed).unwrap();

        let stored = vault.get("u1", "youtube").unwrap();
        assert_eq!(stored.access_token, "at-2");
        assert_eq!(stored.refresh_token, original_refresh);
    }

    #[test]
    fn store_refreshed_replaces_refresh_token_when_response_includes_it() {
        let vault = TokenVault::in_memory();
        vault.store("u1", "youtube", &token("at-1")).unwrap();

        let refreshed = TokenResponse {
            access_token: "at-2".into(),
            refresh_token: Some("brand-new-refresh".into()),
            expires_in: Some(3600),
            token_type: Some("Bearer".into()),
        };
        vault.store_refreshed("u1", "youtube", &refreshed).unwrap();

        let stored = vault.get("u1", "youtube").unwrap();
        assert_eq!(stored.refresh_token.as_deref(), Some("brand-new-refresh"));
    }

    #[test]
    fn persists_encrypted_and_reloads() {
        let path = std::env::temp_dir().join(format!("sd-tokens-{}.enc", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let key = crate::crypto::derive_key(b"jwt", "streamdubber.tokens.v1");

        {
            let vault = TokenVault::with_file(&path, &key).unwrap();
            vault
                .store("u1", "youtube", &token("at-secret-youtube"))
                .unwrap();
        }

        // Dosyada düz metin token/refresh yok.
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("at-secret-youtube"));
        assert!(!raw.contains("refresh-at-secret-youtube"));
        assert!(!raw.contains("youtube"));

        // Aynı anahtarla yeniden açılır, kayıt durur.
        let reloaded = TokenVault::with_file(&path, &key).unwrap();
        assert!(reloaded.connected("u1", "youtube"));
        assert_eq!(
            reloaded.get("u1", "youtube").unwrap().access_token,
            "at-secret-youtube"
        );

        // Yanlış anahtar reddedilir.
        let wrong = crate::crypto::derive_key(b"other", "streamdubber.tokens.v1");
        assert!(TokenVault::with_file(&path, &wrong).is_err());

        let _ = std::fs::remove_file(&path);
    }
}
