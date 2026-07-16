use std::sync::Arc;

use axum::body::to_bytes;
use axum::http::header;
use streamdubber_backend::auth::jwt::Jwt;
use streamdubber_backend::auth::session::{
    create_browser_session, SessionError, SessionPrincipal, SessionStore,
};
use streamdubber_backend::config::ServiceConfig;
use streamdubber_backend::gateway::Gateway;
use streamdubber_backend::state::AppState;

fn principal() -> SessionPrincipal {
    SessionPrincipal {
        sub: "user-1".into(),
        role: "broadcaster".into(),
        plan: "free".into(),
    }
}

#[tokio::test]
async fn refresh_tokens_rotate_and_replay_revokes_the_family() {
    let sessions = SessionStore::in_memory();
    let first = sessions.issue(principal()).await.expect("initial session");
    let second = sessions.rotate(&first.refresh_token).await.expect("first rotation");

    assert_ne!(first.refresh_token, second.refresh_token);
    assert_eq!(second.principal, principal());
    assert_eq!(sessions.rotate(&first.refresh_token).await, Err(SessionError::Replay));
    assert_eq!(sessions.rotate(&second.refresh_token).await, Err(SessionError::Revoked));
}

#[tokio::test]
async fn logout_revokes_the_complete_refresh_family() {
    let sessions = SessionStore::in_memory();
    let first = sessions.issue(principal()).await.expect("initial session");
    let second = sessions.rotate(&first.refresh_token).await.expect("rotation");

    sessions.revoke(&second.refresh_token).await.expect("revoke");
    assert_eq!(sessions.rotate(&second.refresh_token).await, Err(SessionError::Revoked));
}

#[tokio::test]
async fn only_hashes_are_retained_in_the_refresh_store() {
    let sessions = SessionStore::in_memory();
    let issued = sessions.issue(principal()).await.expect("initial session");

    assert!(!sessions.debug_contains_raw_token(&issued.refresh_token));
}

#[tokio::test]
async fn browser_response_uses_secure_httponly_cookies_and_exposes_no_token() {
    let config = Arc::new(ServiceConfig::with_defaults());
    let gateway = Gateway::new(Arc::clone(&config));
    let app = AppState::skeleton(Jwt::new(b"cookie-test-secret"), config, gateway);

    let response = create_browser_session(&app, principal())
        .await
        .expect("browser session");
    let cookies: Vec<_> = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|value| value.to_str().expect("cookie header").to_string())
        .collect();
    assert_eq!(cookies.len(), 3);
    assert!(
        cookies.iter().all(|cookie| !cookie.contains("Domain=")),
        "browser cookies must stay host-only: {cookies:?}"
    );
    assert!(cookies.iter().any(|cookie| {
        cookie.starts_with("sd_access=")
            && cookie.contains("HttpOnly")
            && cookie.contains("Secure")
            && cookie.contains("SameSite=Lax")
    }));
    assert!(cookies.iter().any(|cookie| {
        cookie.starts_with("sd_refresh=")
            && cookie.contains("HttpOnly")
            && cookie.contains("Secure")
            && cookie.contains("Path=/api/auth")
    }));

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json body");
    assert!(json.get("token").is_none());
    assert_eq!(json["role"], "broadcaster");
    assert!(json["csrf_token"].as_str().is_some_and(|value| !value.is_empty()));
}
