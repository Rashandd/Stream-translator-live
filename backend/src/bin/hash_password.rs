//! `ADMIN_PASSWORD_HASH` üretici — argon2id PHC dizesi basar.
//!
//! Kullanım: `cargo run --bin hash_password` → şifreyi stdin'den okur
//! (satır sonu kırpılır), hash'i stdout'a yazar. Şifre hiçbir yere loglanmaz.

use std::io::{BufRead, Write};

use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHasher};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = std::io::stdin();
    let mut stderr = std::io::stderr();

    // Prompt stderr'e — stdout sadece hash'i taşır (pipe edilebilir).
    write!(stderr, "Password: ")?;
    stderr.flush()?;

    let mut password = String::new();
    stdin.lock().read_line(&mut password)?;
    let password = password.trim_end_matches(['\r', '\n']);
    if password.is_empty() {
        return Err("empty password".into());
    }

    let mut salt_bytes = [0u8; 16];
    getrandom::getrandom(&mut salt_bytes)?;
    let salt = SaltString::encode_b64(&salt_bytes).map_err(|e| e.to_string())?;

    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| e.to_string())?;

    println!("{hash}");
    Ok(())
}
