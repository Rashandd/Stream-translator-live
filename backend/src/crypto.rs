//! At-rest şifreleme — AES-256-GCM.
//!
//! Kullanım alanları (bkz. `docs/schema.md`, `docs/security.md`):
//! - DB'deki `*_encrypted` kolonları (TOTP secret, OAuth token'ları)
//! - Ses modelleri (kullanıcı bazlı izolasyon)
//! - Lokal stream key'ler
//!
//! NOT: Ses *akışı* AES ile şifrelenmez — transit için TLS 1.3 yeterli
//! (double encryption gereksiz CPU). Bu modül yalnızca at-rest içindir.
//!
//! Format: `base64( nonce[12] || ciphertext_with_tag )`. Her şifreleme için
//! rastgele nonce üretilir; aynı düz metin her seferinde farklı çıktı verir.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

/// AES-256 anahtar uzunluğu (byte).
pub const KEY_LEN: usize = 32;
/// GCM nonce uzunluğu (byte).
pub const NONCE_LEN: usize = 12;

/// Şifreleme/çözme hataları.
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("invalid key length: expected {KEY_LEN}, got {0}")]
    KeyLength(usize),
    #[error("encryption failed")]
    Encrypt,
    #[error("decryption failed (wrong key or corrupt data)")]
    Decrypt,
    #[error("malformed ciphertext: {0}")]
    Malformed(&'static str),
    #[error("failed to generate nonce: {0}")]
    Nonce(#[from] getrandom::Error),
}

/// AES-256-GCM şifreleyici. Anahtar bellekte tutulur (kaynağı Vault/env).
pub struct Cipher {
    inner: Aes256Gcm,
}

impl Cipher {
    /// 32 byte anahtarla bir şifreleyici kurar.
    pub fn new(key: &[u8]) -> Result<Self, CryptoError> {
        if key.len() != KEY_LEN {
            return Err(CryptoError::KeyLength(key.len()));
        }
        let key = Key::<Aes256Gcm>::from_slice(key);
        Ok(Cipher {
            inner: Aes256Gcm::new(key),
        })
    }

    /// Düz metni şifreler ve base64(`nonce || ciphertext`) döndürür.
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<String, CryptoError> {
        let mut nonce_bytes = [0u8; NONCE_LEN];
        getrandom::getrandom(&mut nonce_bytes)?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .inner
            .encrypt(nonce, plaintext)
            .map_err(|_| CryptoError::Encrypt)?;

        let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&ciphertext);
        Ok(B64.encode(out))
    }

    /// `encrypt` çıktısını çözer.
    pub fn decrypt(&self, encoded: &str) -> Result<Vec<u8>, CryptoError> {
        let bytes = B64
            .decode(encoded)
            .map_err(|_| CryptoError::Malformed("invalid base64"))?;
        if bytes.len() < NONCE_LEN {
            return Err(CryptoError::Malformed("ciphertext shorter than nonce"));
        }
        let (nonce_bytes, ciphertext) = bytes.split_at(NONCE_LEN);
        let nonce = Nonce::from_slice(nonce_bytes);
        self.inner
            .decrypt(nonce, ciphertext)
            .map_err(|_| CryptoError::Decrypt)
    }

    /// Bir UTF-8 string'i şifreler (kolay kullanım).
    pub fn encrypt_str(&self, plaintext: &str) -> Result<String, CryptoError> {
        self.encrypt(plaintext.as_bytes())
    }

    /// Çözülen veriyi UTF-8 string olarak döndürür.
    pub fn decrypt_str(&self, encoded: &str) -> Result<String, CryptoError> {
        let bytes = self.decrypt(encoded)?;
        String::from_utf8(bytes).map_err(|_| CryptoError::Malformed("not valid utf-8"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> [u8; KEY_LEN] {
        let mut k = [0u8; KEY_LEN];
        for (i, b) in k.iter_mut().enumerate() {
            *b = i as u8;
        }
        k
    }

    #[test]
    fn roundtrip() {
        let c = Cipher::new(&key()).expect("cipher");
        let enc = c.encrypt_str("rtmp://secret-stream-key").expect("encrypt");
        let dec = c.decrypt_str(&enc).expect("decrypt");
        assert_eq!(dec, "rtmp://secret-stream-key");
    }

    #[test]
    fn ciphertext_differs_each_time() {
        // Rastgele nonce → aynı düz metin farklı şifreli metin verir.
        let c = Cipher::new(&key()).expect("cipher");
        let a = c.encrypt_str("same").expect("a");
        let b = c.encrypt_str("same").expect("b");
        assert_ne!(a, b);
        assert_eq!(c.decrypt_str(&a).unwrap(), c.decrypt_str(&b).unwrap());
    }

    #[test]
    fn rejects_wrong_key_length() {
        assert!(matches!(Cipher::new(&[0u8; 16]), Err(CryptoError::KeyLength(16))));
    }

    #[test]
    fn wrong_key_fails_to_decrypt() {
        let c1 = Cipher::new(&key()).expect("c1");
        let enc = c1.encrypt_str("data").expect("encrypt");

        let mut other = key();
        other[0] ^= 0xFF;
        let c2 = Cipher::new(&other).expect("c2");
        assert!(matches!(c2.decrypt(&enc), Err(CryptoError::Decrypt)));
    }

    #[test]
    fn rejects_tampered_ciphertext() {
        let c = Cipher::new(&key()).expect("cipher");
        let enc = c.encrypt_str("data").expect("encrypt");
        // Son karakteri boz → GCM tag doğrulaması başarısız olmalı.
        let mut bytes = B64.decode(&enc).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0x01;
        let tampered = B64.encode(bytes);
        assert!(c.decrypt(&tampered).is_err());
    }

    #[test]
    fn rejects_too_short() {
        let c = Cipher::new(&key()).expect("cipher");
        let short = B64.encode([0u8; 4]);
        assert!(matches!(c.decrypt(&short), Err(CryptoError::Malformed(_))));
    }
}
