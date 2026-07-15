//! Ham ses örneği deposu — AES-256-GCM şifreli at-rest (2026-07-06 kullanıcı
//! kararı, roadmap "Öz Kaynaklı Ses Klonlama + Speech-to-Speech" bölümü
//! madde 2). Önceki "ham örnek hiç saklanmaz" kararının bilinçli tersi:
//! yeniden eğitim/sağlayıcı değişimi/model iyileştirme için örnek artık
//! saklanıyor. `relay::RelayTargetStore` ile birebir aynı desen — tüm JSON
//! blob'u tek parça şifrelenir, model metadata'sından (`VoiceModelStore`,
//! gizli değil) ayrı bir dosyada durur.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

use crate::crypto::Cipher;

/// Ses örneği depolama hataları.
#[derive(Debug, thiserror::Error)]
pub enum VoiceSampleError {
    #[error("voice sample store io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("voice sample store crypto error: {0}")]
    Crypto(String),
    #[error("voice sample store serde error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// Model id → ham ses örneği deposu (opsiyonel şifreli dosya kalıcılığı).
pub struct VoiceSampleStore {
    entries: RwLock<HashMap<String, String>>,
    persist: Option<(PathBuf, Cipher)>,
    db: Option<sqlx::PgPool>,
    db_cipher: Option<Cipher>,
}

impl VoiceSampleStore {
    /// Kalıcılıksız depo (dev/test).
    pub fn in_memory() -> Self {
        VoiceSampleStore {
            entries: RwLock::new(HashMap::new()),
            persist: None,
            db: None,
            db_cipher: None,
        }
    }

    /// Şifreli dosya kalıcılığıyla kurar; dosya varsa örnekleri yükler.
    pub fn with_file(path: impl Into<PathBuf>, key: &[u8]) -> Result<Self, VoiceSampleError> {
        let path = path.into();
        let cipher = Cipher::new(key).map_err(|e| VoiceSampleError::Crypto(e.to_string()))?;
        let entries = load_entries(&path, &cipher)?;
        Ok(VoiceSampleStore {
            entries: RwLock::new(entries),
            persist: Some((path, cipher)),
            db: None,
            db_cipher: None,
        })
    }

    pub fn with_db(mut self, pool: sqlx::PgPool, key: &[u8]) -> Result<Self, VoiceSampleError> {
        self.db = Some(pool);
        self.db_cipher = Some(Cipher::new(key).map_err(|e| VoiceSampleError::Crypto(e.to_string()))?);
        Ok(self)
    }

    pub async fn persistent_put(&self, model_id: &str, user_id: &str, sample: &[u8]) -> Result<(), VoiceSampleError> {
        let Some(pool) = &self.db else { return self.put(model_id, sample); };
        let user_id = uuid::Uuid::parse_str(user_id).map_err(|_| VoiceSampleError::Crypto("invalid user id".into()))?;
        let encoded = B64.encode(sample);
        let cipher = self.db_cipher.as_ref().ok_or_else(|| VoiceSampleError::Crypto("database voice sample encryption key unavailable".into()))?;
        let sealed = cipher.encrypt_str(&encoded).map_err(|e| VoiceSampleError::Crypto(e.to_string()))?;
        sqlx::query("INSERT INTO voice_sample_references (voice_model_id, user_id, sample_encrypted, byte_length) VALUES ($1,$2,$3,$4) ON CONFLICT (voice_model_id) DO UPDATE SET sample_encrypted=EXCLUDED.sample_encrypted, byte_length=EXCLUDED.byte_length, user_id=EXCLUDED.user_id")
            .bind(model_id).bind(user_id).bind(sealed).bind(sample.len() as i32).execute(pool).await.map_err(|_| VoiceSampleError::Crypto("voice sample database write failed".into()))?;
        Ok(())
    }

    /// Bir modelin ham örneğini (üstüne) yazar.
    pub fn put(&self, model_id: &str, sample: &[u8]) -> Result<(), VoiceSampleError> {
        {
            let mut map = self
                .entries
                .write()
                .map_err(|_| VoiceSampleError::Crypto("voice sample lock poisoned".into()))?;
            map.insert(model_id.to_string(), B64.encode(sample));
        }
        self.save()
    }

    /// Bir modelin ham örneği (yoksa `None`).
    pub fn get(&self, model_id: &str) -> Option<Vec<u8>> {
        let encoded = self.entries.read().ok().and_then(|m| m.get(model_id).cloned())?;
        B64.decode(encoded).ok()
    }

    /// Bir modelin örneğini siler (model silinirse/reddedilirse çağrılabilir).
    pub fn remove(&self, model_id: &str) -> Result<(), VoiceSampleError> {
        {
            let mut map = self
                .entries
                .write()
                .map_err(|_| VoiceSampleError::Crypto("voice sample lock poisoned".into()))?;
            map.remove(model_id);
        }
        self.save()
    }

    fn save(&self) -> Result<(), VoiceSampleError> {
        let Some((path, cipher)) = &self.persist else {
            return Ok(());
        };
        let snapshot: HashMap<String, String> = self
            .entries
            .read()
            .map_err(|_| VoiceSampleError::Crypto("voice sample lock poisoned".into()))?
            .clone();
        let json = serde_json::to_string(&snapshot)?;
        let sealed = cipher
            .encrypt_str(&json)
            .map_err(|e| VoiceSampleError::Crypto(e.to_string()))?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, sealed)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

fn load_entries(path: &Path, cipher: &Cipher) -> Result<HashMap<String, String>, VoiceSampleError> {
    let sealed = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(e) => return Err(VoiceSampleError::Io(e)),
    };
    let json = cipher
        .decrypt_str(sealed.trim())
        .map_err(|e| VoiceSampleError::Crypto(format!("voice sample file unreadable: {e}")))?;
    Ok(serde_json::from_str(&json)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_and_get_roundtrip_in_memory() {
        let store = VoiceSampleStore::in_memory();
        store.put("m1", &[1, 2, 3, 4]).unwrap();
        assert_eq!(store.get("m1"), Some(vec![1, 2, 3, 4]));
        assert_eq!(store.get("ghost"), None);
    }

    #[test]
    fn remove_clears_entry() {
        let store = VoiceSampleStore::in_memory();
        store.put("m1", &[9, 9]).unwrap();
        store.remove("m1").unwrap();
        assert_eq!(store.get("m1"), None);
    }

    #[test]
    fn persists_encrypted_and_reloads_without_plaintext_sample() {
        let path = std::env::temp_dir().join(format!("sd-voice-sample-{}.enc", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let key = crate::crypto::derive_key(b"jwt", "streamdubber.voice_samples.v1");

        {
            let store = VoiceSampleStore::with_file(&path, &key).unwrap();
            store.put("m1", b"raw-audio-bytes-super-secret").unwrap();
        }

        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("raw-audio-bytes-super-secret"));

        let reloaded = VoiceSampleStore::with_file(&path, &key).unwrap();
        assert_eq!(reloaded.get("m1"), Some(b"raw-audio-bytes-super-secret".to_vec()));

        let wrong = crate::crypto::derive_key(b"other", "streamdubber.voice_samples.v1");
        assert!(VoiceSampleStore::with_file(&path, &wrong).is_err());

        let _ = std::fs::remove_file(&path);
    }
}
