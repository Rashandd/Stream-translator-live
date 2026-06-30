//! Rol tabanlı yetkilendirme (RBAC) ve HTTP extractor'ları.

use std::sync::Arc;

use axum::{
    extract::{FromRef, FromRequestParts},
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
};

use super::jwt::{Claims, Jwt, JwtError};

/// Kullanıcı rolü.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Broadcaster,
    Admin,
}

impl Role {
    /// String'den rol çözümler. Bilinmeyen değer `None`.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "broadcaster" => Some(Role::Broadcaster),
            "admin" => Some(Role::Admin),
            _ => None,
        }
    }
}

/// Yetkilendirme reddi.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("missing or malformed Authorization header")]
    MissingBearer,
    #[error("invalid token: {0}")]
    InvalidToken(#[from] JwtError),
    #[error("insufficient role")]
    Forbidden,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let status = match self {
            AuthError::MissingBearer | AuthError::InvalidToken(_) => StatusCode::UNAUTHORIZED,
            AuthError::Forbidden => StatusCode::FORBIDDEN,
        };
        (status, self.to_string()).into_response()
    }
}

/// `Authorization: Bearer <token>` başlığından token'ı çıkarır.
fn bearer_token(parts: &Parts) -> Result<&str, AuthError> {
    parts
        .headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .ok_or(AuthError::MissingBearer)
}

/// Doğrulanmış kullanıcı — herhangi bir rol kabul edilir.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub claims: Claims,
    pub role: Role,
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    Arc<Jwt>: axum::extract::FromRef<S>,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let jwt = Arc::<Jwt>::from_ref(state);
        let token = bearer_token(parts)?;
        let claims = jwt.verify(token)?;
        // Bilinmeyen rol = broadcaster'a düşürme; reddet (güvenli varsayılan).
        let role = Role::parse(&claims.role).ok_or(AuthError::Forbidden)?;
        Ok(AuthUser { claims, role })
    }
}

/// Sadece admin rolünün geçebildiği extractor.
#[derive(Debug, Clone)]
pub struct AdminUser {
    pub claims: Claims,
}

impl<S> FromRequestParts<S> for AdminUser
where
    S: Send + Sync,
    Arc<Jwt>: axum::extract::FromRef<S>,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let user = AuthUser::from_request_parts(parts, state).await?;
        match user.role {
            Role::Admin => Ok(AdminUser { claims: user.claims }),
            Role::Broadcaster => Err(AuthError::Forbidden),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_roles() {
        assert_eq!(Role::parse("broadcaster"), Some(Role::Broadcaster));
        assert_eq!(Role::parse("admin"), Some(Role::Admin));
        assert_eq!(Role::parse("root"), None);
    }

    fn parts_with(header: Option<&str>) -> Parts {
        let mut builder = axum::http::Request::builder().uri("/");
        if let Some(h) = header {
            builder = builder.header(axum::http::header::AUTHORIZATION, h);
        }
        let req = builder.body(()).expect("build request");
        req.into_parts().0
    }

    #[test]
    fn extracts_bearer_token() {
        let parts = parts_with(Some("Bearer abc.def"));
        assert_eq!(bearer_token(&parts).unwrap(), "abc.def");
    }

    #[test]
    fn rejects_missing_header() {
        let parts = parts_with(None);
        assert!(matches!(bearer_token(&parts), Err(AuthError::MissingBearer)));
    }

    #[test]
    fn rejects_non_bearer_scheme() {
        let parts = parts_with(Some("Basic abc"));
        assert!(matches!(bearer_token(&parts), Err(AuthError::MissingBearer)));
    }

    #[test]
    fn rejects_empty_bearer() {
        let parts = parts_with(Some("Bearer "));
        assert!(matches!(bearer_token(&parts), Err(AuthError::MissingBearer)));
    }
}
