//! Twitch Extension JWT doğrulaması (Faz 2.4).
//!
//! Twitch Extension Helper'ın `onAuthorized` ile verdiği token HS256'dır ve
//! Twitch developer console'daki **base64 kodlu** shared secret ile imzalanır.
//! Doğrulama sırası: imza (mevcut secret → yoksa önceki secret) → `exp`.
//! Rotasyon: yeni secret `TWITCH_EXT_SECRET`'a girilir, eski değer süresi
//! dolmamış token'lar için `TWITCH_EXT_SECRET_PREVIOUS`'a taşınır — iki değer
//! de Vault/env/panel kaynaklıdır, koda gömülmez ve her çağrıda çözülür
//! (restart'sız devreye girer).

use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::Deserialize;

use crate::secrets::SecretRef;

/// Twitch extension token claim'leri (kullanılan alt küme).
///
/// `role`: `viewer` | `broadcaster` | `moderator` | `external`.
/// `opaque_user_id`: her viewer için Twitch'in verdiği takma kimlik — rate
/// limit anahtarı olarak kullanılır, kişisel veri değildir.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TwitchExtClaims {
    pub exp: i64,
    #[serde(default)]
    pub channel_id: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub opaque_user_id: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
}

/// Doğrulama hataları. Mesajlar istemciye döner — secret/iç detay içermez.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ExtJwtError {
    #[error("twitch extension secret not configured")]
    NotConfigured,
    #[error("token expired")]
    Expired,
    #[error("invalid token")]
    Invalid,
}

/// Rotasyon destekli Twitch extension JWT doğrulayıcısı.
pub struct TwitchExtVerifier {
    current: SecretRef,
    previous: SecretRef,
}

impl TwitchExtVerifier {
    pub fn new(current: SecretRef, previous: SecretRef) -> Self {
        TwitchExtVerifier { current, previous }
    }

    /// Token'ı doğrular. İmza mevcut secret'la tutmazsa (ve önceki secret
    /// yapılandırılmışsa) bir kez de onunla denenir — `Expired` imza geçerli
    /// demektir, ikinci deneme yapılmaz.
    pub async fn verify(&self, token: &str) -> Result<TwitchExtClaims, ExtJwtError> {
        let Some(current) = self.current.resolve().await else {
            return Err(ExtJwtError::NotConfigured);
        };
        match Self::try_decode(token, &current) {
            Err(ExtJwtError::Invalid) => match self.previous.resolve().await {
                Some(previous) => Self::try_decode(token, &previous),
                None => Err(ExtJwtError::Invalid),
            },
            other => other,
        }
    }

    /// Tek bir base64 secret ile decode dener.
    fn try_decode(token: &str, base64_secret: &str) -> Result<TwitchExtClaims, ExtJwtError> {
        let key =
            DecodingKey::from_base64_secret(base64_secret).map_err(|_| ExtJwtError::Invalid)?;
        let mut validation = Validation::default(); // HS256 + exp kontrolü
        validation.leeway = 0;
        match decode::<TwitchExtClaims>(token, &key, &validation) {
            Ok(data) => Ok(data.claims),
            Err(err) => match err.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => Err(ExtJwtError::Expired),
                _ => Err(ExtJwtError::Invalid),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde::Serialize;

    #[derive(Serialize)]
    struct TestClaims {
        exp: i64,
        channel_id: String,
        role: String,
        opaque_user_id: String,
    }

    fn b64(secret: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(secret)
    }

    fn sign(secret_b64: &str, exp: i64) -> String {
        let key = EncodingKey::from_base64_secret(secret_b64).expect("test key");
        let claims = TestClaims {
            exp,
            channel_id: "ch-1".into(),
            role: "viewer".into(),
            opaque_user_id: "U-opaque".into(),
        };
        encode(&Header::default(), &claims, &key).expect("sign")
    }

    fn future() -> i64 {
        chrono::Utc::now().timestamp() + 300
    }

    #[tokio::test]
    async fn verifies_token_signed_with_current_secret() {
        let secret = b64(b"current-secret");
        let verifier = TwitchExtVerifier::new(SecretRef::fixed(secret.clone()), SecretRef::none());
        let claims = verifier.verify(&sign(&secret, future())).await.expect("verify");
        assert_eq!(claims.channel_id.as_deref(), Some("ch-1"));
        assert_eq!(claims.role.as_deref(), Some("viewer"));
        assert_eq!(claims.opaque_user_id.as_deref(), Some("U-opaque"));
    }

    #[tokio::test]
    async fn falls_back_to_previous_secret_after_rotation() {
        let old = b64(b"old-secret");
        let new = b64(b"new-secret");
        let verifier =
            TwitchExtVerifier::new(SecretRef::fixed(new), SecretRef::fixed(old.clone()));
        // Rotasyondan önce imzalanmış token hâlâ geçer.
        assert!(verifier.verify(&sign(&old, future())).await.is_ok());
    }

    #[tokio::test]
    async fn rejects_expired_token_without_previous_fallback() {
        let secret = b64(b"current-secret");
        let verifier =
            TwitchExtVerifier::new(SecretRef::fixed(secret.clone()), SecretRef::fixed(secret.clone()));
        let past = chrono::Utc::now().timestamp() - 10;
        assert_eq!(
            verifier.verify(&sign(&secret, past)).await,
            Err(ExtJwtError::Expired)
        );
    }

    #[tokio::test]
    async fn rejects_wrong_secret_and_garbage() {
        let verifier =
            TwitchExtVerifier::new(SecretRef::fixed(b64(b"right")), SecretRef::none());
        let wrong = sign(&b64(b"wrong"), future());
        assert_eq!(verifier.verify(&wrong).await, Err(ExtJwtError::Invalid));
        assert_eq!(verifier.verify("not.a.token").await, Err(ExtJwtError::Invalid));
    }

    #[tokio::test]
    async fn unconfigured_secret_is_explicit_error() {
        let verifier = TwitchExtVerifier::new(SecretRef::none(), SecretRef::none());
        assert_eq!(
            verifier.verify("whatever").await,
            Err(ExtJwtError::NotConfigured)
        );
    }
}
