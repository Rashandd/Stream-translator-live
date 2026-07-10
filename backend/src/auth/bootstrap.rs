//! Env-bootstrap admin kimliği (Faz 1).
//!
//! Cloud deploy'da admin panelin (`/admin/secrets`, `/admin/config`, ...)
//! kullanılabilmesi için tek bir bootstrap admin env'den tanımlanır:
//!
//! - `ADMIN_EMAIL` — admin e-postası
//! - `ADMIN_PASSWORD_HASH` — argon2id PHC dizesi (önerilen; üretmek için
//!   `cargo run --bin hash_password`)
//! - `ADMIN_PASSWORD` — düz metin (SADECE dev; startup'ta warn loglanır)
//!
//! `POST /api/auth/login` ucu artık `users::login`'dedir (1.4): önce bu
//! bootstrap admin, sonra yayıncı deposu denenir. Başarılı admin girişi
//! `role=admin` JWT üretir (12 saat). Şifre asla loglanmaz.

use argon2::password_hash::PasswordHash;
use argon2::{Argon2, PasswordVerifier};

/// Şifre doğrulama biçimi.
enum PasswordCheck {
    /// argon2id PHC dizesi (`$argon2id$v=19$...`).
    Argon2(String),
    /// Düz metin — sadece dev. `from_env` bunu seçerse warn loglar.
    Plain(String),
}

/// Env'den tanımlanan tek bootstrap admin.
pub struct BootstrapAdmin {
    email: String,
    check: PasswordCheck,
}

impl BootstrapAdmin {
    /// `ADMIN_EMAIL` + (`ADMIN_PASSWORD_HASH` | `ADMIN_PASSWORD`) env'inden
    /// kurar. Eksikse `None` — login endpoint'i 503 döner.
    pub fn from_env() -> Option<Self> {
        let read = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        let email = read("ADMIN_EMAIL")?;
        if let Some(hash) = read("ADMIN_PASSWORD_HASH") {
            return Some(BootstrapAdmin {
                email,
                check: PasswordCheck::Argon2(hash),
            });
        }
        if let Some(plain) = read("ADMIN_PASSWORD") {
            tracing::warn!(
                "ADMIN_PASSWORD (plain) in use — set ADMIN_PASSWORD_HASH (argon2id) in production"
            );
            return Some(BootstrapAdmin {
                email,
                check: PasswordCheck::Plain(plain),
            });
        }
        None
    }

    /// Test/manuel kurulum: argon2id hash ile.
    pub fn with_hash(email: impl Into<String>, phc_hash: impl Into<String>) -> Self {
        BootstrapAdmin {
            email: email.into(),
            check: PasswordCheck::Argon2(phc_hash.into()),
        }
    }

    /// Test/dev kurulum: düz şifre ile.
    pub fn with_plain(email: impl Into<String>, password: impl Into<String>) -> Self {
        BootstrapAdmin {
            email: email.into(),
            check: PasswordCheck::Plain(password.into()),
        }
    }

    /// E-posta (case-insensitive) + şifre doğrulaması.
    pub fn verify(&self, email: &str, password: &str) -> bool {
        let email_ok = constant_time_eq(
            self.email.to_lowercase().as_bytes(),
            email.to_lowercase().as_bytes(),
        );
        let password_ok = match &self.check {
            PasswordCheck::Argon2(hash) => PasswordHash::new(hash)
                .map(|parsed| {
                    Argon2::default()
                        .verify_password(password.as_bytes(), &parsed)
                        .is_ok()
                })
                .unwrap_or(false),
            PasswordCheck::Plain(expected) => {
                constant_time_eq(expected.as_bytes(), password.as_bytes())
            }
        };
        // Şifre her durumda doğrulanır (email yanlışsa bile) — zamanlamadan
        // hangi alanın yanlış olduğu sızmaz.
        email_ok && password_ok
    }

    /// Canonical delivery address for the second factor. It is exposed only
    /// inside the backend and never included in session payloads.
    pub fn email(&self) -> &str {
        &self.email
    }

    /// Case-insensitive, constant-time identity check used after a 2FA code
    /// has been consumed.
    pub fn matches_email(&self, email: &str) -> bool {
        constant_time_eq(
            self.email.to_lowercase().as_bytes(),
            email.trim().to_lowercase().as_bytes(),
        )
    }
}

/// Zamanlama saldırılarına karşı sabit zamanlı karşılaştırma.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use argon2::password_hash::SaltString;
    use argon2::PasswordHasher;

    fn hash(password: &str) -> String {
        let mut salt_bytes = [0u8; 16];
        getrandom::getrandom(&mut salt_bytes).expect("salt");
        let salt = SaltString::encode_b64(&salt_bytes).expect("salt encode");
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .expect("hash")
            .to_string()
    }

    #[test]
    fn argon2_hash_verifies_correct_password_only() {
        let admin = BootstrapAdmin::with_hash("ops@sd.io", hash("hunter2-strong"));
        assert!(admin.verify("ops@sd.io", "hunter2-strong"));
        assert!(admin.verify("OPS@SD.IO", "hunter2-strong")); // email case-insensitive
        assert!(!admin.verify("ops@sd.io", "wrong"));
        assert!(!admin.verify("other@sd.io", "hunter2-strong"));
    }

    #[test]
    fn plain_password_verifies_exactly() {
        let admin = BootstrapAdmin::with_plain("ops@sd.io", "dev-pass");
        assert!(admin.verify("ops@sd.io", "dev-pass"));
        assert!(!admin.verify("ops@sd.io", "dev-pas"));
        assert!(!admin.verify("ops@sd.io", "dev-passX"));
    }

    #[test]
    fn malformed_hash_never_verifies() {
        let admin = BootstrapAdmin::with_hash("ops@sd.io", "not-a-phc-string");
        assert!(!admin.verify("ops@sd.io", "anything"));
    }

    #[test]
    fn from_env_reads_bootstrap_config() {
        // SAFETY: testler tek süreçte env paylaşır; benzersiz değerler kullan
        // ve sonunda temizle.
        std::env::remove_var("ADMIN_PASSWORD_HASH");
        std::env::set_var("ADMIN_EMAIL", "boot@sd.io");
        std::env::set_var("ADMIN_PASSWORD", "boot-pass");
        let admin = BootstrapAdmin::from_env().expect("configured");
        assert!(admin.verify("boot@sd.io", "boot-pass"));
        std::env::remove_var("ADMIN_EMAIL");
        std::env::remove_var("ADMIN_PASSWORD");
        assert!(BootstrapAdmin::from_env().is_none());
    }
}
