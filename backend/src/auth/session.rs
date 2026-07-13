//! Browser session lifecycle.
//!
//! Access JWTs live only in `HttpOnly` cookies. Refresh tokens are random
//! 256-bit values; only their SHA-256 hashes are retained. Every refresh
//! rotates the token. Replaying an already-rotated token revokes its whole
//! family so a copied refresh token cannot race the legitimate browser.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use base64::Engine;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::Row;

use crate::auth::authz::AuthUser;
use crate::auth::jwt::Claims;
use crate::config::ServiceConfig;
use crate::state::AppState;

const REFRESH_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const ACCESS_TTL_SECS: i64 = 15 * 60;
pub(crate) const ACCESS_COOKIE: &str = "sd_access";
const REFRESH_COOKIE: &str = "sd_refresh";
const CSRF_COOKIE: &str = "sd_csrf";
const CSRF_HEADER: &str = "x-csrf-token";

/// The identity copied into a short-lived access JWT.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionPrincipal {
    pub sub: String,
    pub role: String,
    pub plan: String,
}

/// A newly issued or rotated refresh session. The raw token is returned only
/// to the cookie writer and is never persisted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedRefresh {
    pub principal: SessionPrincipal,
    pub refresh_token: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefreshStatus {
    Active,
    Rotated,
    Revoked,
}

impl RefreshStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Rotated => "rotated",
            Self::Revoked => "revoked",
        }
    }

    fn parse(value: &str) -> Result<Self, SessionError> {
        match value {
            "active" => Ok(Self::Active),
            "rotated" => Ok(Self::Rotated),
            "revoked" => Ok(Self::Revoked),
            _ => Err(SessionError::Storage("unknown refresh status".into())),
        }
    }
}

#[derive(Debug, Clone)]
struct RefreshRecord {
    family_id: String,
    principal: SessionPrincipal,
    expires_at: i64,
    status: RefreshStatus,
}

/// Refresh lifecycle errors intentionally do not expose whether a token ever
/// existed. Handlers map all authentication variants to HTTP 401.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SessionError {
    #[error("invalid refresh session")]
    Invalid,
    #[error("refresh session expired")]
    Expired,
    #[error("refresh session revoked")]
    Revoked,
    #[error("refresh token replay detected")]
    Replay,
    #[error("session storage failed: {0}")]
    Storage(String),
    #[error("secure random generation failed")]
    Random,
}

/// Refresh store. PostgreSQL is authoritative when configured; the in-memory
/// implementation keeps tests and local development self-contained.
pub struct SessionStore {
    entries: Mutex<HashMap<String, RefreshRecord>>,
    db: Option<sqlx::PgPool>,
    ttl: Duration,
}

impl SessionStore {
    pub fn in_memory() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            db: None,
            ttl: REFRESH_TTL,
        }
    }

    pub fn with_db(db: sqlx::PgPool) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            db: Some(db),
            ttl: REFRESH_TTL,
        }
    }

    pub async fn issue(&self, principal: SessionPrincipal) -> Result<IssuedRefresh, SessionError> {
        let raw = random_opaque_token()?;
        let hash = opaque_token_hash(&raw);
        let family_id = uuid::Uuid::new_v4().to_string();
        let expires_at = chrono::Utc::now().timestamp()
            + i64::try_from(self.ttl.as_secs()).unwrap_or(i64::MAX);
        let record = RefreshRecord {
            family_id,
            principal: principal.clone(),
            expires_at,
            status: RefreshStatus::Active,
        };

        if let Some(pool) = &self.db {
            insert_db(pool, &hash, &record).await?;
        } else {
            self.entries
                .lock()
                .map_err(|_| SessionError::Storage("refresh lock poisoned".into()))?
                .insert(hash, record);
        }

        Ok(IssuedRefresh {
            principal,
            refresh_token: raw,
            expires_at,
        })
    }

    pub async fn rotate(&self, raw: &str) -> Result<IssuedRefresh, SessionError> {
        if let Some(pool) = &self.db {
            self.rotate_db(pool, raw).await
        } else {
            self.rotate_memory(raw)
        }
    }

    pub async fn revoke(&self, raw: &str) -> Result<(), SessionError> {
        let hash = opaque_token_hash(raw);
        if let Some(pool) = &self.db {
            let family: Option<String> = sqlx::query_scalar(
                "SELECT family_id::text FROM browser_sessions WHERE token_hash = $1",
            )
            .bind(&hash)
            .fetch_optional(pool)
            .await
            .map_err(storage)?;
            let Some(family) = family else {
                return Err(SessionError::Invalid);
            };
            sqlx::query(
                "UPDATE browser_sessions SET status = 'revoked', revoked_at = NOW()
                 WHERE family_id = $1::uuid AND status <> 'revoked'",
            )
            .bind(family)
            .execute(pool)
            .await
            .map_err(storage)?;
            return Ok(());
        }

        let mut entries = self
            .entries
            .lock()
            .map_err(|_| SessionError::Storage("refresh lock poisoned".into()))?;
        let family = entries
            .get(&hash)
            .map(|record| record.family_id.clone())
            .ok_or(SessionError::Invalid)?;
        revoke_family(&mut entries, &family);
        Ok(())
    }

    /// Test-only observability without exposing hashes or principals.
    pub fn debug_contains_raw_token(&self, raw: &str) -> bool {
        self.entries
            .lock()
            .map(|entries| entries.keys().any(|retained| retained == raw))
            .unwrap_or(false)
    }

    fn rotate_memory(&self, raw: &str) -> Result<IssuedRefresh, SessionError> {
        let old_hash = opaque_token_hash(raw);
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| SessionError::Storage("refresh lock poisoned".into()))?;
        let old = entries.get(&old_hash).cloned().ok_or(SessionError::Invalid)?;

        match old.status {
            RefreshStatus::Rotated => {
                revoke_family(&mut entries, &old.family_id);
                return Err(SessionError::Replay);
            }
            RefreshStatus::Revoked => return Err(SessionError::Revoked),
            RefreshStatus::Active => {}
        }
        if old.expires_at <= chrono::Utc::now().timestamp() {
            revoke_family(&mut entries, &old.family_id);
            return Err(SessionError::Expired);
        }

        if let Some(record) = entries.get_mut(&old_hash) {
            record.status = RefreshStatus::Rotated;
        }
        let next_raw = random_opaque_token()?;
        let next_hash = opaque_token_hash(&next_raw);
        let next = RefreshRecord {
            family_id: old.family_id,
            principal: old.principal.clone(),
            expires_at: old.expires_at,
            status: RefreshStatus::Active,
        };
        entries.insert(next_hash, next);
        Ok(IssuedRefresh {
            principal: old.principal,
            refresh_token: next_raw,
            expires_at: old.expires_at,
        })
    }

    async fn rotate_db(
        &self,
        pool: &sqlx::PgPool,
        raw: &str,
    ) -> Result<IssuedRefresh, SessionError> {
        let old_hash = opaque_token_hash(raw);
        let mut tx = pool.begin().await.map_err(storage)?;
        let row = sqlx::query(
            "SELECT family_id::text, subject, role, plan, status,
                    EXTRACT(EPOCH FROM expires_at)::bigint AS expires_at
             FROM browser_sessions WHERE token_hash = $1 FOR UPDATE",
        )
        .bind(&old_hash)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage)?
        .ok_or(SessionError::Invalid)?;

        let family: String = row.try_get("family_id").map_err(storage)?;
        let status = RefreshStatus::parse(row.try_get::<String, _>("status").map_err(storage)?.as_str())?;
        if status == RefreshStatus::Rotated {
            sqlx::query(
                "UPDATE browser_sessions SET status = 'revoked', revoked_at = NOW()
                 WHERE family_id = $1::uuid AND status <> 'revoked'",
            )
            .bind(&family)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
            tx.commit().await.map_err(storage)?;
            return Err(SessionError::Replay);
        }
        if status == RefreshStatus::Revoked {
            return Err(SessionError::Revoked);
        }

        let expires_at: i64 = row.try_get("expires_at").map_err(storage)?;
        if expires_at <= chrono::Utc::now().timestamp() {
            sqlx::query(
                "UPDATE browser_sessions SET status = 'revoked', revoked_at = NOW()
                 WHERE family_id = $1::uuid AND status <> 'revoked'",
            )
            .bind(&family)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
            tx.commit().await.map_err(storage)?;
            return Err(SessionError::Expired);
        }

        let principal = SessionPrincipal {
            sub: row.try_get("subject").map_err(storage)?,
            role: row.try_get("role").map_err(storage)?,
            plan: row.try_get("plan").map_err(storage)?,
        };
        sqlx::query(
            "UPDATE browser_sessions SET status = 'rotated', rotated_at = NOW()
             WHERE token_hash = $1",
        )
        .bind(&old_hash)
        .execute(&mut *tx)
        .await
        .map_err(storage)?;

        let next_raw = random_opaque_token()?;
        let next_hash = opaque_token_hash(&next_raw);
        sqlx::query(
            "INSERT INTO browser_sessions
                (token_hash, family_id, subject, role, plan, status, expires_at)
             VALUES ($1, $2::uuid, $3, $4, $5, 'active', to_timestamp($6))",
        )
        .bind(&next_hash)
        .bind(&family)
        .bind(&principal.sub)
        .bind(&principal.role)
        .bind(&principal.plan)
        .bind(expires_at)
        .execute(&mut *tx)
        .await
        .map_err(storage)?;
        tx.commit().await.map_err(storage)?;

        Ok(IssuedRefresh {
            principal,
            refresh_token: next_raw,
            expires_at,
        })
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::in_memory()
    }
}

/// Safe, non-secret session data returned to the frontend. `csrf_token` is a
/// double-submit value, not an authentication credential; the browser still
/// needs the HttpOnly access/refresh cookies for every authenticated request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BrowserSession {
    pub authenticated: bool,
    pub user_id: String,
    pub role: String,
    pub plan: String,
    pub expires_at: i64,
    pub csrf_token: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RequestSecurityError {
    #[error("request origin is not allowed")]
    Origin,
    #[error("csrf validation failed")]
    Csrf,
}

/// Set a brand-new browser session after password, 2FA or OAuth validation.
pub async fn create_browser_session(
    app: &AppState,
    principal: SessionPrincipal,
) -> Result<Response, (StatusCode, String)> {
    let issued = app
        .browser_sessions
        .issue(principal)
        .await
        .map_err(internal_http)?;
    session_response(app, issued)
}

/// `GET /api/auth/session` — validates the short-lived access cookie through
/// `AuthUser` and gives AuthGuard a live role/expiry decision.
pub async fn current_session(user: AuthUser, headers: HeaderMap) -> Json<BrowserSession> {
    Json(BrowserSession {
        authenticated: true,
        user_id: user.claims.sub,
        role: user.claims.role,
        plan: user.claims.plan,
        expires_at: user.claims.exp,
        csrf_token: cookie_value(&headers, CSRF_COOKIE).unwrap_or_default(),
    })
}

/// `POST /api/auth/refresh` — one-time rotation. Reuse of the old token
/// revokes the entire family in `SessionStore`.
pub async fn refresh(State(app): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(error) = require_cookie_request(&headers, &app.service_config) {
        return (StatusCode::FORBIDDEN, error.to_string()).into_response();
    }
    let Some(raw) = cookie_value(&headers, REFRESH_COOKIE) else {
        return clear_session_response(StatusCode::UNAUTHORIZED);
    };
    match app.browser_sessions.rotate(&raw).await {
        Ok(issued) => match session_response(&app, issued) {
            Ok(response) => response,
            Err(error) => error.into_response(),
        },
        Err(_) => clear_session_response(StatusCode::UNAUTHORIZED),
    }
}

/// `POST /api/auth/logout` — revoke the refresh family and expire all browser
/// cookies. The response is intentionally successful for an already-gone
/// session so logout is idempotent.
pub async fn logout(State(app): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(error) = require_cookie_request(&headers, &app.service_config) {
        return (StatusCode::FORBIDDEN, error.to_string()).into_response();
    }
    if let Some(raw) = cookie_value(&headers, REFRESH_COOKIE) {
        if let Err(error) = app.browser_sessions.revoke(&raw).await {
            tracing::debug!(%error, "logout refresh session was already unavailable");
        }
    }
    clear_session_response(StatusCode::NO_CONTENT)
}

/// Extract the access JWT from an HttpOnly cookie. Bearer support remains in
/// `authz` for SDK and server-to-server clients.
pub(crate) fn access_cookie(headers: &HeaderMap) -> Option<String> {
    cookie_value(headers, ACCESS_COOKIE)
}

/// Cookie-authenticated mutations must come from an explicitly configured
/// frontend origin and carry a matching double-submit CSRF value.
pub(crate) fn require_cookie_request(
    headers: &HeaderMap,
    config: &ServiceConfig,
) -> Result<(), RequestSecurityError> {
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .ok_or(RequestSecurityError::Origin)?;
    let allowed = config
        .get("cors.allowed_origins")
        .map(|origins| {
            origins
                .split(',')
                .map(str::trim)
                .filter(|candidate| !candidate.is_empty() && *candidate != "*")
                .any(|candidate| candidate.eq_ignore_ascii_case(origin))
        })
        .unwrap_or(false);
    if !allowed {
        return Err(RequestSecurityError::Origin);
    }

    let header_token = headers
        .get(CSRF_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or(RequestSecurityError::Csrf)?;
    let cookie_token = cookie_value(headers, CSRF_COOKIE).ok_or(RequestSecurityError::Csrf)?;
    if !constant_time_eq(header_token.as_bytes(), cookie_token.as_bytes()) {
        return Err(RequestSecurityError::Csrf);
    }
    Ok(())
}

/// Builds the access/refresh/csrf `Set-Cookie` header strings for a freshly
/// issued or rotated session, plus the safe body describing it. Shared by the
/// JSON session response and the OAuth callback redirect so both paths set an
/// identical cookie triple.
fn session_cookies(
    app: &AppState,
    issued: IssuedRefresh,
) -> Result<(BrowserSession, Vec<String>), (StatusCode, String)> {
    let expires_at = chrono::Utc::now().timestamp() + ACCESS_TTL_SECS;
    let claims = Claims {
        sub: issued.principal.sub.clone(),
        role: issued.principal.role.clone(),
        plan: issued.principal.plan.clone(),
        exp: expires_at,
    };
    let access = app
        .jwt
        .issue(&claims)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    let csrf = random_opaque_token().map_err(internal_http)?;
    let secure = cookie_secure();
    let refresh_max_age = issued
        .expires_at
        .saturating_sub(chrono::Utc::now().timestamp());
    let cookies = vec![
        format!(
            "{ACCESS_COOKIE}={access}; Path=/; Max-Age={ACCESS_TTL_SECS}; HttpOnly; SameSite=Lax{}",
            secure_flag(secure)
        ),
        format!(
            "{REFRESH_COOKIE}={}; Path=/api/auth; Max-Age={refresh_max_age}; HttpOnly; SameSite=Strict{}",
            issued.refresh_token,
            secure_flag(secure)
        ),
        format!(
            "{CSRF_COOKIE}={csrf}; Path=/; Max-Age={refresh_max_age}; SameSite=Lax{}",
            secure_flag(secure)
        ),
    ];
    let body = BrowserSession {
        authenticated: true,
        user_id: issued.principal.sub,
        role: issued.principal.role,
        plan: issued.principal.plan,
        expires_at,
        csrf_token: csrf,
    };
    Ok((body, cookies))
}

fn session_response(
    app: &AppState,
    issued: IssuedRefresh,
) -> Result<Response, (StatusCode, String)> {
    let (body, cookies) = session_cookies(app, issued)?;
    let mut response = Json(body).into_response();
    for cookie in cookies {
        append_set_cookie(&mut response, cookie)?;
    }
    Ok(response)
}

/// Issue a fresh browser session and attach its `Set-Cookie` headers to an
/// arbitrary response — e.g. the OAuth callback's 303 redirect — so a browser
/// login never has to carry an access token in the URL fragment or
/// `localStorage`. The passed response keeps its status/location; only cookies
/// are added.
pub async fn attach_new_browser_session(
    app: &AppState,
    principal: SessionPrincipal,
    mut response: Response,
) -> Result<Response, (StatusCode, String)> {
    let issued = app
        .browser_sessions
        .issue(principal)
        .await
        .map_err(internal_http)?;
    let (_body, cookies) = session_cookies(app, issued)?;
    for cookie in cookies {
        append_set_cookie(&mut response, cookie)?;
    }
    Ok(response)
}

fn clear_session_response(status: StatusCode) -> Response {
    let secure = cookie_secure();
    let mut response = status.into_response();
    for (name, path, http_only) in [
        (ACCESS_COOKIE, "/", true),
        (REFRESH_COOKIE, "/api/auth", true),
        (CSRF_COOKIE, "/", false),
    ] {
        let cookie = format!(
            "{name}=; Path={path}; Max-Age=0; SameSite=Lax{}{}",
            if http_only { "; HttpOnly" } else { "" },
            secure_flag(secure)
        );
        if let Ok(value) = HeaderValue::from_str(&cookie) {
            response.headers_mut().append(header::SET_COOKIE, value);
        }
    }
    response
}

fn append_set_cookie(
    response: &mut Response,
    cookie: String,
) -> Result<(), (StatusCode, String)> {
    let value = HeaderValue::from_str(&cookie).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not create session cookie".to_string(),
        )
    })?;
    response.headers_mut().append(header::SET_COOKIE, value);
    Ok(())
}

pub(crate) fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|part| part.trim().split_once('='))
        .find_map(|(candidate, value)| (candidate == name).then(|| value.to_string()))
}

pub(crate) fn cookie_secure() -> bool {
    !matches!(
        std::env::var("AUTH_COOKIE_SECURE").as_deref(),
        Ok("0") | Ok("false") | Ok("FALSE")
    )
}

pub(crate) fn secure_flag(secure: bool) -> &'static str {
    if secure { "; Secure" } else { "" }
}

fn internal_http(error: SessionError) -> (StatusCode, String) {
    tracing::error!(%error, "browser session operation failed");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "session operation failed".to_string(),
    )
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0_u8, |acc, (left, right)| acc | (left ^ right)) == 0
}

fn revoke_family(entries: &mut HashMap<String, RefreshRecord>, family: &str) {
    for record in entries.values_mut() {
        if record.family_id == family {
            record.status = RefreshStatus::Revoked;
        }
    }
}

async fn insert_db(
    pool: &sqlx::PgPool,
    hash: &str,
    record: &RefreshRecord,
) -> Result<(), SessionError> {
    sqlx::query(
        "INSERT INTO browser_sessions
            (token_hash, family_id, subject, role, plan, status, expires_at)
         VALUES ($1, $2::uuid, $3, $4, $5, $6, to_timestamp($7))",
    )
    .bind(hash)
    .bind(&record.family_id)
    .bind(&record.principal.sub)
    .bind(&record.principal.role)
    .bind(&record.principal.plan)
    .bind(record.status.as_str())
    .bind(record.expires_at)
    .execute(pool)
    .await
    .map_err(storage)?;
    Ok(())
}

pub(crate) fn random_opaque_token() -> Result<String, SessionError> {
    let mut bytes = [0_u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|_| SessionError::Random)?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

pub(crate) fn opaque_token_hash(raw: &str) -> String {
    hex::encode(Sha256::digest(raw.as_bytes()))
}

fn storage(error: impl std::fmt::Display) -> SessionError {
    SessionError::Storage(error.to_string())
}
