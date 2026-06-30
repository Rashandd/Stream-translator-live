//! Axum router kurulumu.

use axum::{routing::get, Router};
use tower_http::trace::TraceLayer;

use crate::auth::authz::AdminUser;
use crate::state::AppState;
use crate::ws::ws_handler;

/// Health check handler — container health probe'ları için.
async fn health() -> &'static str {
    "ok"
}

/// Admin-only ping — yetkilendirme katmanının doğrulaması.
/// `AdminUser` extractor admin olmayan/eksik token'ı 401/403 ile reddeder.
async fn admin_ping(admin: AdminUser) -> String {
    format!("admin:{}", admin.claims.sub)
}

/// Uygulama router'ını kurar.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ws/stream", get(ws_handler))
        .route("/admin/ping", get(admin_ping))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
