//! Paylaşılan uygulama state'i — router ve handler'lara enjekte edilir.

use std::sync::Arc;

use axum::extract::FromRef;

use crate::auth::bootstrap::BootstrapAdmin;
use crate::auth::jwt::Jwt;
use crate::billing::Billing;
use crate::config::ServiceConfig;
use crate::gateway::Gateway;
use crate::integrations::Integrations;
use crate::logging::Logger;
use crate::overlay::OverlayHub;
use crate::pairing::PairingStore;
use crate::plans::PlanRegistry;
use crate::provider_check::ProviderTestStore;
use crate::relay::RelayTargetStore;
use crate::secrets::ManagedSecrets;
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
    pub logger: Arc<Logger>,
    pub secrets: Arc<ManagedSecrets>,
    /// Env-bootstrap admin (`ADMIN_EMAIL`+şifre). `None` → login 503.
    pub bootstrap_admin: Arc<Option<BootstrapAdmin>>,
    /// PostgreSQL havuzu (`DATABASE_URL` verilmişse). `None` → persist
    /// isteyen yollar (Stripe webhook, usage) sadece loglar.
    pub db: Option<sqlx::PgPool>,
    /// SDK ↔ hesap eşleştirme kodları (bellek içi, kısa ömürlü).
    pub pairing: Arc<PairingStore>,
    /// SDK relay hedefleri (stream key'ler şifreli kalıcı).
    pub relay: Arc<RelayTargetStore>,
    /// Sağlayıcı canlı testlerinin en son sonucu (bellek içi — "last tested at").
    pub provider_tests: Arc<ProviderTestStore>,
}

impl AppState {
    /// Test ve iskelet kurulumları için: verilen jwt + config + gateway
    /// dışındaki tüm alanlar boş/varsayılan. Alanlar `pub` — çağıran,
    /// ihtiyacına göre tek tek üzerine yazar (`state.logger = ...`).
    pub fn skeleton(jwt: Jwt, service_config: Arc<ServiceConfig>, gateway: Gateway) -> Self {
        AppState {
            jwt: Arc::new(jwt),
            gateway: Arc::new(gateway),
            logger: Arc::new(Logger::in_memory(Arc::clone(&service_config))),
            service_config,
            integrations: Arc::new(Integrations::empty()),
            billing: Arc::new(Billing::empty()),
            overlay: Arc::new(OverlayHub::new()),
            plans: Arc::new(PlanRegistry::with_defaults()),
            usage: Arc::new(UsageTracker::new()),
            secrets: Arc::new(ManagedSecrets::env()),
            bootstrap_admin: Arc::new(None),
            db: None,
            pairing: Arc::new(PairingStore::new()),
            relay: Arc::new(RelayTargetStore::in_memory()),
            provider_tests: Arc::new(ProviderTestStore::new()),
        }
    }
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

impl FromRef<AppState> for Arc<Logger> {
    fn from_ref(state: &AppState) -> Self {
        state.logger.clone()
    }
}

impl FromRef<AppState> for Arc<ManagedSecrets> {
    fn from_ref(state: &AppState) -> Self {
        state.secrets.clone()
    }
}

impl FromRef<AppState> for Arc<Option<BootstrapAdmin>> {
    fn from_ref(state: &AppState) -> Self {
        state.bootstrap_admin.clone()
    }
}

impl FromRef<AppState> for Option<sqlx::PgPool> {
    fn from_ref(state: &AppState) -> Self {
        state.db.clone()
    }
}

impl FromRef<AppState> for Arc<PairingStore> {
    fn from_ref(state: &AppState) -> Self {
        state.pairing.clone()
    }
}

impl FromRef<AppState> for Arc<RelayTargetStore> {
    fn from_ref(state: &AppState) -> Self {
        state.relay.clone()
    }
}

impl FromRef<AppState> for Arc<ProviderTestStore> {
    fn from_ref(state: &AppState) -> Self {
        state.provider_tests.clone()
    }
}
