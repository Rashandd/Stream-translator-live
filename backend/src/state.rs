//! Paylaşılan uygulama state'i — router ve handler'lara enjekte edilir.

use std::sync::Arc;

use axum::extract::FromRef;

use crate::auth::jwt::Jwt;
use crate::config::ServiceConfig;
use crate::gateway::Gateway;

/// Tüm handler'ların paylaştığı state. Ucuz klonlanır (içte `Arc`).
#[derive(Clone)]
pub struct AppState {
    pub jwt: Arc<Jwt>,
    pub gateway: Arc<Gateway>,
    pub service_config: Arc<ServiceConfig>,
}

impl FromRef<AppState> for Arc<Jwt> {
    fn from_ref(state: &AppState) -> Self {
        state.jwt.clone()
    }
}

impl FromRef<AppState> for Arc<Gateway> {
    fn from_ref(state: &AppState) -> Self {
        state.gateway.clone()
    }
}

impl FromRef<AppState> for Arc<ServiceConfig> {
    fn from_ref(state: &AppState) -> Self {
        state.service_config.clone()
    }
}
