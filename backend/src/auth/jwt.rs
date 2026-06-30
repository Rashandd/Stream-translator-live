//! JWT access token üretimi ve doğrulaması (HS256).
//!
//! Secret koda gömülmez — `JWT_SECRET` env var'ından (Faz 1) veya Vault'tan
//! (sonra) gelir. `exp` her doğrulamada kontrol edilir.

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

/// Access token claim'leri.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claims {
    /// Subject — kullanıcı UUID'si.
    pub sub: String,
    /// Rol: `broadcaster` veya `admin`.
    pub role: String,
    /// Abonelik planı adı.
    pub plan: String,
    /// Bitiş zamanı (unix saniye).
    pub exp: i64,
}

/// JWT doğrulama/üretim hataları.
#[derive(Debug, thiserror::Error)]
pub enum JwtError {
    #[error("token expired")]
    Expired,
    #[error("invalid token: {0}")]
    Invalid(String),
    #[error("token encoding failed: {0}")]
    Encode(String),
}

/// HS256 anahtarlarını tutan doğrulayıcı.
#[derive(Clone)]
pub struct Jwt {
    encoding: EncodingKey,
    decoding: DecodingKey,
    validation: Validation,
}

impl Jwt {
    /// Verilen secret ile bir doğrulayıcı kurar.
    pub fn new(secret: &[u8]) -> Self {
        let mut validation = Validation::default(); // HS256, exp doğrulaması açık
        validation.leeway = 0;
        Jwt {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
            validation,
        }
    }

    /// Bir token üretir (test ve dahili kullanım için).
    pub fn issue(&self, claims: &Claims) -> Result<String, JwtError> {
        encode(&Header::default(), claims, &self.encoding).map_err(|e| JwtError::Encode(e.to_string()))
    }

    /// Token'ı doğrular ve claim'leri döndürür. Süresi dolmuşsa `Expired`.
    pub fn verify(&self, token: &str) -> Result<Claims, JwtError> {
        match decode::<Claims>(token, &self.decoding, &self.validation) {
            Ok(data) => Ok(data.claims),
            Err(err) => match err.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => Err(JwtError::Expired),
                _ => Err(JwtError::Invalid(err.to_string())),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claims(exp: i64) -> Claims {
        Claims {
            sub: "11111111-1111-1111-1111-111111111111".into(),
            role: "broadcaster".into(),
            plan: "free".into(),
            exp,
        }
    }

    fn future() -> i64 {
        chrono::Utc::now().timestamp() + 3600
    }

    #[test]
    fn issue_and_verify_roundtrip() {
        let jwt = Jwt::new(b"test-secret");
        let token = jwt.issue(&claims(future())).expect("issue");
        let decoded = jwt.verify(&token).expect("verify");
        assert_eq!(decoded.sub, "11111111-1111-1111-1111-111111111111");
        assert_eq!(decoded.role, "broadcaster");
    }

    #[test]
    fn rejects_expired_token() {
        let jwt = Jwt::new(b"test-secret");
        let past = chrono::Utc::now().timestamp() - 10;
        let token = jwt.issue(&claims(past)).expect("issue");
        assert!(matches!(jwt.verify(&token), Err(JwtError::Expired)));
    }

    #[test]
    fn rejects_wrong_secret() {
        let signer = Jwt::new(b"secret-a");
        let verifier = Jwt::new(b"secret-b");
        let token = signer.issue(&claims(future())).expect("issue");
        assert!(matches!(verifier.verify(&token), Err(JwtError::Invalid(_))));
    }

    #[test]
    fn rejects_garbage() {
        let jwt = Jwt::new(b"test-secret");
        assert!(matches!(jwt.verify("not.a.token"), Err(JwtError::Invalid(_))));
    }
}
