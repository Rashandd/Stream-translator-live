//! Axum router kurulumu.

use axum::{routing::get, Router};
use tower_http::trace::TraceLayer;

use crate::ws::ws_handler;

/// Health check handler — container health probe'ları için.
async fn health() -> &'static str {
    "ok"
}

/// Uygulama router'ını kurar (`/health`, `/ws/stream`).
pub fn router() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ws/stream", get(ws_handler))
        .layer(TraceLayer::new_for_http())
}
