//! Opaque login-2FA challenge (A-04).
//!
//! When login or signup needs an email verification code, the browser must not
//! hold the email or subject — the pending state is a short-lived, opaque
//! `challenge_id` carried in an HttpOnly `sd_auth_challenge` cookie. The server
//! keeps subject / normalized email / role here; verify and resend read the
//! cookie, never a request-body field. The 6-digit codes themselves stay in
//! [`crate::users::TwoFactorStore`], keyed by the challenge's email.
//!
//! Deliberately in-memory and short-lived: a restart drops pending challenges
//! (the user simply signs in again). The store interface mirrors
//! [`crate::auth::session::SessionStore`] so a multi-instance Redis backend can
//! be swapped in later (plan §8) without touching call sites.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::http::{header, HeaderMap, HeaderValue};
use axum::response::Response;

use crate::auth::session::{cookie_secure, cookie_value, secure_flag};

pub(crate) const CHALLENGE_COOKIE: &str = "sd_auth_challenge";

/// Server-side pending-login state. The browser only ever holds the opaque id
/// that maps here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Challenge {
    /// User id, or `admin-bootstrap` for the env-configured admin.
    pub subject: String,
    /// Normalized (trimmed, lowercased) email the code was sent to.
    pub email: String,
    /// `admin` or `broadcaster` — drives the post-verify redirect.
    pub role: String,
    created: Instant,
}

/// Opaque `challenge_id` → pending identity, single-use and time-limited.
pub struct ChallengeStore {
    inner: Mutex<HashMap<String, Challenge>>,
    ttl: Duration,
}

impl ChallengeStore {
    pub fn new(ttl: Duration) -> Self {
        ChallengeStore {
            inner: Mutex::new(HashMap::new()),
            ttl,
        }
    }

    /// Issue a fresh opaque challenge id bound to the pending identity.
    pub fn issue(&self, subject: &str, email: &str, role: &str) -> String {
        let id = uuid::Uuid::new_v4().simple().to_string();
        if let Ok(mut map) = self.inner.lock() {
            map.insert(
                id.clone(),
                Challenge {
                    subject: subject.to_string(),
                    email: email.trim().to_lowercase(),
                    role: role.to_string(),
                    created: Instant::now(),
                },
            );
        }
        id
    }

    /// Read a live challenge without consuming it (resend). Expired → `None`.
    pub fn peek(&self, id: &str) -> Option<Challenge> {
        let map = self.inner.lock().ok()?;
        let challenge = map.get(id)?;
        if challenge.created.elapsed() > self.ttl {
            return None;
        }
        Some(challenge.clone())
    }

    /// Consume a live challenge (verify). Single-use; expired → `None`.
    pub fn consume(&self, id: &str) -> Option<Challenge> {
        let mut map = self.inner.lock().ok()?;
        let challenge = map.remove(id)?;
        if challenge.created.elapsed() > self.ttl {
            return None;
        }
        Some(challenge)
    }

    pub fn ttl_secs(&self) -> u64 {
        self.ttl.as_secs()
    }
}

impl Default for ChallengeStore {
    fn default() -> Self {
        ChallengeStore::new(Duration::from_secs(600))
    }
}

/// Read the opaque challenge id from the request cookie.
pub(crate) fn challenge_cookie(headers: &HeaderMap) -> Option<String> {
    cookie_value(headers, CHALLENGE_COOKIE)
}

/// Attach the challenge cookie (HttpOnly, scoped to `/api/auth`, `SameSite=Strict`)
/// to a response — used on the pending-login JSON reply.
pub(crate) fn set_challenge_cookie(response: &mut Response, id: &str, max_age_secs: u64) {
    let cookie = format!(
        "{CHALLENGE_COOKIE}={id}; Path=/api/auth; Max-Age={max_age_secs}; HttpOnly; SameSite=Strict{}",
        secure_flag(cookie_secure())
    );
    if let Ok(value) = HeaderValue::from_str(&cookie) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
}

/// Expire the challenge cookie once the challenge is spent (verify success or
/// a missing/expired challenge).
pub(crate) fn clear_challenge_cookie(response: &mut Response) {
    let cookie = format!(
        "{CHALLENGE_COOKIE}=; Path=/api/auth; Max-Age=0; HttpOnly; SameSite=Strict{}",
        secure_flag(cookie_secure())
    );
    if let Ok(value) = HeaderValue::from_str(&cookie) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_then_consume_is_single_use_and_normalizes_email() {
        let store = ChallengeStore::new(Duration::from_secs(60));
        let id = store.issue("user-1", "  Streamer@Example.com ", "broadcaster");
        let peeked = store.peek(&id).expect("live challenge");
        assert_eq!(peeked.email, "streamer@example.com");
        assert_eq!(peeked.role, "broadcaster");
        assert_eq!(peeked.subject, "user-1");
        // Consume removes it; a second consume returns nothing.
        assert_eq!(
            store.consume(&id).map(|c| c.subject),
            Some("user-1".to_string())
        );
        assert!(store.consume(&id).is_none());
        assert!(store.peek(&id).is_none());
    }

    #[test]
    fn expired_challenge_is_rejected() {
        let store = ChallengeStore::new(Duration::from_millis(0));
        let id = store.issue("user-1", "a@b.com", "broadcaster");
        std::thread::sleep(Duration::from_millis(5));
        assert!(store.peek(&id).is_none());
        assert!(store.consume(&id).is_none());
    }

    #[test]
    fn unknown_id_is_rejected() {
        let store = ChallengeStore::new(Duration::from_secs(60));
        assert!(store.consume("nope").is_none());
        assert!(store.peek("nope").is_none());
    }
}
