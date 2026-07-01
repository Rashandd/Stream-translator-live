//! Paylaşılan uygulama state'i — router ve handler'lara enjekte edilir.

use std::sync::Arc;

use axum::extract::FromRef;

use crate::auth::jwt::Jwt;
use crate::billing::Billing;
use crate::config::ServiceConfig;
use crate::gateway::Gateway;
use crate::integrations::Integrations;
use crate::overlay::OverlayHub;
use crate::plans::PlanRegistry;
use crate::usage::UsageTracker;

/// Tüm handler'ların paylaştığı state. Ucuz klonlanır (içte `Arc`).
#[derive(Clone)]
pub struct AppState {
    pub jwt: Arc<Jwt>,
    pub gateway: Arc<Gateway>,
    pub service_config: Arc<ServiceConfig>,
    pub integrations: Arc<Integrations>,
    pub billing: Arc<Billing>,
    pub overlay: Arc<OverlayHub>,
    pub plans: Arc<PlanRegistry>,
    pub usage: Arc<UsageTracker>,
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

impl FromRef<AppState> for Arc<Integrations> {
    fn from_ref(state: &AppState) -> Self {
        state.integrations.clone()
    }
}

impl FromRef<AppState> for Arc<Billing> {
    fn from_ref(state: &AppState) -> Self {
        state.billing.clone()
    }
}

impl FromRef<AppState> for Arc<OverlayHub> {
    fn from_ref(state: &AppState) -> Self {
        state.overlay.clone()
    }
}

impl FromRef<AppState> for Arc<PlanRegistry> {
    fn from_ref(state: &AppState) -> Self {
        state.plans.clone()
    }
}

impl FromRef<AppState> for Arc<UsageTracker> {
    fn from_ref(state: &AppState) -> Self {
        state.usage.clone()
    }
}
