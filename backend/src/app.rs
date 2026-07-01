//! Axum router kurulumu.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, put},
    Json, Router,
};
use serde::Deserialize;
use tower_http::trace::TraceLayer;

use crate::auth::authz::AdminUser;
use crate::config::ServiceConfig;
use crate::integrations;
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

/// Çalışma zamanı config'ini okur (public).
///
/// Yayıncı paneli sağlayıcı seçimini ve plan erişimini buradan okur —
/// hiçbir değer frontend'e gömülü değildir. Secret içermez.
async fn get_config(State(cfg): State<Arc<ServiceConfig>>) -> Json<BTreeMap<String, String>> {
    Json(cfg.snapshot())
}

/// Admin config güncelleme gövdesi.
#[derive(Deserialize)]
struct ConfigUpdate {
    key: String,
    value: String,
}

/// Bir config anahtarını günceller (sadece admin).
///
/// Yalnızca seed'de tanımlı anahtarlar yazılabilir; bilinmeyen anahtar 404.
/// Değişiklik restart gerektirmez, çalışan tüm okuyuculara anında yansır.
async fn put_config(
    _admin: AdminUser,
    State(cfg): State<Arc<ServiceConfig>>,
    Json(update): Json<ConfigUpdate>,
) -> Result<Json<BTreeMap<String, String>>, StatusCode> {
    if cfg.set_existing(&update.key, update.value) {
        Ok(Json(cfg.snapshot()))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// Uygulama router'ını kurar.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ws/stream", get(ws_handler))
        .route("/admin/ping", get(admin_ping))
        .route("/config", get(get_config))
        .route("/admin/config", put(put_config))
        // Multistream OAuth (1.6): yayıncı listeler/başlatır, sağlayıcı callback'ler.
        .route("/api/integrations", get(integrations::list))
        .route(
            "/api/integrations/{provider}/authorize",
            get(integrations::authorize),
        )
        .route(
            "/api/integrations/{provider}/callback",
            get(integrations::callback),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
