//! Local encrypted pairing-session persistence (desktop shell, 1.3/1.4 follow-up).
//!
//! The desktop shell needs to remember its paired device token across
//! restarts without asking the broadcaster to re-pair every launch (device
//! tokens are valid 30 days — see `backend/src/pairing/mod.rs`). CLAUDE.md
//! requires local stream key material to be AES-256 encrypted at rest, so
//! the session is never written to disk in plaintext.
//!
//! Key management: a random 32-byte key is generated on first save and kept
//! in a sibling file next to the ciphertext. This is weaker than an
//! OS-keychain-backed key (a future improvement — see `docs/sdk-distribution.md`)
//! but meets the at-rest-encryption bar and needs no extra native dependency.

use std::fs;
use std::path::{Path, PathBuf};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use serde::{Deserialize, Serialize};

const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 12;
const KEY_FILE: &str = "device.key";
const SESSION_FILE: &str = "session.enc";

/// A paired desktop shell's persisted session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalSession {
    pub backend_url: String,
    pub token: String,
    /// Live-presence identifiers from the claim response (see
    /// `sdk_session::heartbeat` on the backend and `core::heartbeat` here) —
    /// used to sign periodic heartbeats so the panel's "connected" device
    /// list can't be spoofed or confused with another paired device.
    /// `#[serde(default)]` so a session persisted before this field existed
    /// still loads (falls back to empty — the heartbeat loop just gets 404s,
    /// logged and swallowed like any other network hiccup, until the next
    /// re-pair; it never fails to load the token itself).
    #[serde(default)]
    pub sdk_session_id: String,
    #[serde(default)]
    pub heartbeat_secret: String,
}

/// Local session store errors.
#[derive(Debug, thiserror::Error)]
pub enum LocalStoreError {
    #[error("local store io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("local store crypto error: {0}")]
    Crypto(String),
    #[error("local store serde error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// AES-256-GCM encrypted session store rooted at a directory.
pub struct LocalTokenStore {
    dir: PathBuf,
}

impl LocalTokenStore {
    /// Opens (without touching disk yet) a store rooted at `dir`.
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        LocalTokenStore { dir: dir.into() }
    }

    fn key_path(&self) -> PathBuf {
        self.dir.join(KEY_FILE)
    }

    fn session_path(&self) -> PathBuf {
        self.dir.join(SESSION_FILE)
    }

    /// Loads the persisted session, if any. Missing or undecryptable state
    /// (e.g. key file lost) is treated as "not paired" rather than an error —
    /// the UI just falls back to the pairing screen.
    pub fn load(&self) -> Option<LocalSession> {
        let key = fs::read(self.key_path()).ok()?;
        let sealed = fs::read(self.session_path()).ok()?;
        if key.len() != KEY_LEN || sealed.len() < NONCE_LEN {
            return None;
        }
        let cipher = Aes256Gcm::new_from_slice(&key).ok()?;
        let (nonce_bytes, ciphertext) = sealed.split_at(NONCE_LEN);
        let plaintext = cipher
            .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
            .ok()?;
        serde_json::from_slice(&plaintext).ok()
    }

    /// Encrypts and persists a session, generating a key on first use.
    pub fn save(&self, session: &LocalSession) -> Result<(), LocalStoreError> {
        fs::create_dir_all(&self.dir)?;
        let key = self.load_or_create_key()?;
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| LocalStoreError::Crypto(e.to_string()))?;

        let mut nonce_bytes = [0u8; NONCE_LEN];
        getrandom::getrandom(&mut nonce_bytes)
            .map_err(|e| LocalStoreError::Crypto(e.to_string()))?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        let plaintext = serde_json::to_vec(session)?;
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_slice())
            .map_err(|e| LocalStoreError::Crypto(e.to_string()))?;

        let mut sealed = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        sealed.extend_from_slice(&nonce_bytes);
        sealed.extend_from_slice(&ciphertext);
        write_restricted(&self.session_path(), &sealed)?;
        Ok(())
    }

    /// Removes the persisted session ("unpair"). The key file is left in
    /// place — harmless, and avoids re-deriving key material on next pair.
    pub fn clear(&self) -> Result<(), LocalStoreError> {
        match fs::remove_file(self.session_path()) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    fn load_or_create_key(&self) -> Result<[u8; KEY_LEN], LocalStoreError> {
        let path = self.key_path();
        if let Ok(existing) = fs::read(&path) {
            if existing.len() == KEY_LEN {
                let mut key = [0u8; KEY_LEN];
                key.copy_from_slice(&existing);
                return Ok(key);
            }
        }
        let mut key = [0u8; KEY_LEN];
        getrandom::getrandom(&mut key).map_err(|e| LocalStoreError::Crypto(e.to_string()))?;
        write_restricted(&path, &key)?;
        Ok(key)
    }
}

/// Writes `bytes` to `path`, restricting permissions to the owner where the
/// platform supports it (unix). Best-effort elsewhere — NTFS ACLs differ
/// enough that a chmod-shaped call has no equivalent worth hand-rolling here.
fn write_restricted(path: &Path, bytes: &[u8]) -> Result<(), LocalStoreError> {
    fs::write(path, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "streamdubber-local-store-test-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn missing_session_loads_as_none() {
        let store = LocalTokenStore::at(temp_dir("missing"));
        assert!(store.load().is_none());
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = temp_dir("roundtrip");
        let store = LocalTokenStore::at(&dir);
        let session = LocalSession {
            backend_url: "https://api.strovak.com".to_string(),
            token: "device-jwt-abc".to_string(),
            sdk_session_id: "sess-1".to_string(),
            heartbeat_secret: "aabbcc".to_string(),
        };
        store.save(&session).expect("save");
        assert_eq!(store.load(), Some(session));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn session_file_is_not_plaintext_on_disk() {
        let dir = temp_dir("not-plaintext");
        let store = LocalTokenStore::at(&dir);
        let session = LocalSession {
            backend_url: "https://api.strovak.com".to_string(),
            token: "super-secret-device-jwt".to_string(),
            sdk_session_id: "sess-1".to_string(),
            heartbeat_secret: "aabbcc".to_string(),
        };
        store.save(&session).expect("save");
        let raw = fs::read(dir.join(SESSION_FILE)).expect("session file exists");
        let raw_text = String::from_utf8_lossy(&raw);
        assert!(!raw_text.contains("super-secret-device-jwt"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn clear_removes_session_but_keeps_key() {
        let dir = temp_dir("clear");
        let store = LocalTokenStore::at(&dir);
        store
            .save(&LocalSession {
                backend_url: "https://api.strovak.com".to_string(),
                token: "t".to_string(),
                sdk_session_id: "sess-1".to_string(),
                heartbeat_secret: "aabbcc".to_string(),
            })
            .expect("save");
        store.clear().expect("clear");
        assert!(store.load().is_none());
        assert!(dir.join(KEY_FILE).exists());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn clear_on_never_paired_store_is_ok() {
        let store = LocalTokenStore::at(temp_dir("clear-empty"));
        assert!(store.clear().is_ok());
    }

    #[test]
    fn wrong_key_cannot_decrypt_another_stores_session() {
        let dir_a = temp_dir("store-a");
        let dir_b = temp_dir("store-b");
        let store_a = LocalTokenStore::at(&dir_a);
        let store_b = LocalTokenStore::at(&dir_b);
        store_a
            .save(&LocalSession {
                backend_url: "https://api.strovak.com".to_string(),
                token: "a-token".to_string(),
                sdk_session_id: "sess-a".to_string(),
                heartbeat_secret: "aabbcc".to_string(),
            })
            .expect("save a");
        store_b
            .save(&LocalSession {
                backend_url: "https://api.strovak.com".to_string(),
                token: "b-token".to_string(),
                sdk_session_id: "sess-b".to_string(),
                heartbeat_secret: "ddeeff".to_string(),
            })
            .expect("save b");

        // Swap in store A's ciphertext but keep store B's key — must not decrypt.
        fs::copy(dir_a.join(SESSION_FILE), dir_b.join(SESSION_FILE)).unwrap();
        assert!(store_b.load().is_none());

        fs::remove_dir_all(&dir_a).ok();
        fs::remove_dir_all(&dir_b).ok();
    }

    #[test]
    fn session_without_heartbeat_fields_still_loads() {
        // Pre-heartbeat-feature format: only backend_url + token. `#[serde(default)]`
        // must let this still deserialize instead of losing the paired session.
        let dir = temp_dir("legacy-format");
        fs::create_dir_all(&dir).unwrap();
        let store = LocalTokenStore::at(&dir);
        let key = store.load_or_create_key().expect("key");
        let cipher = aes_gcm::Aes256Gcm::new_from_slice(&key).unwrap();
        let legacy_json = r#"{"backend_url":"https://api.strovak.com","token":"old-jwt"}"#;
        let mut nonce_bytes = [0u8; NONCE_LEN];
        getrandom::getrandom(&mut nonce_bytes).unwrap();
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce_bytes), legacy_json.as_bytes())
            .unwrap();
        let mut sealed = Vec::new();
        sealed.extend_from_slice(&nonce_bytes);
        sealed.extend_from_slice(&ciphertext);
        fs::write(store.session_path(), sealed).unwrap();

        let loaded = store.load().expect("legacy session still loads");
        assert_eq!(loaded.token, "old-jwt");
        assert_eq!(loaded.sdk_session_id, "");
        assert_eq!(loaded.heartbeat_secret, "");
        fs::remove_dir_all(&dir).ok();
    }
}
