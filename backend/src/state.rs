//! Paylaşılan uygulama state'i — router ve handler'lara enjekte edilir.

use std::sync::Arc;

use axum::extract::FromRef;

use crate::auth::bootstrap::BootstrapAdmin;
use crate::auth::jwt::Jwt;
use crate::auth::oauth_login::LoginProviders;
use crate::auth::session::SessionStore;
use crate::billing::Billing;
use crate::bindings::ChannelBindingStore;
use crate::config::ServiceConfig;
use crate::ext_twitch::ExtTwitch;
use crate::founding::FoundingStore;
use crate::gateway::Gateway;
use crate::integrations::Integrations;
use crate::logging::Logger;
use crate::overlay::{ActiveStreams, OverlayHub};
use crate::pairing::{ClaimLimiter, PairingStore};
use crate::plans::PlanRegistry;
use crate::provider_check::ProviderTestStore;
use crate::rate_limit::AuthLimiter;
use crate::relay::RelayTargetStore;
use crate::sdk_link::{SdkCommandQueue, SttSettingsStore, TtsSettingsStore};
use crate::sdk_session::SdkSessionStore;
use crate::secrets::ManagedSecrets;
use crate::stream_settings::StreamSettingsStore;
use crate::stt_servers::provision::Provisioner;
use crate::stt_servers::SttServerStore;
use crate::usage::UsageTracker;
use crate::users::{TwoFactorStore, UserStore};
use crate::voice_models::{VoiceModelStore, VoiceSampleStore};
use crate::waitlist::WaitlistLimiter;

/// Tüm handler'ların paylaştığı state. Ucuz klonlanır (içte `Arc`).
#[derive(Clone)]
pub struct AppState {
    pub jwt: Arc<Jwt>,
    pub gateway: Arc<Gateway>,
    pub service_config: Arc<ServiceConfig>,
    pub integrations: Arc<Integrations>,
    /// Google/Twitch ile login (auth::oauth_login) — bağlantı *bağlama*
    /// (`integrations`, üstte) ile karıştırılmaz, ayrı bir registry.
    pub oauth_login: Arc<LoginProviders>,
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
    /// `/api/pairing/claim` brute-force koruması (per-IP, bellek içi).
    pub pairing_claim_limiter: Arc<ClaimLimiter>,
    /// Eşleşmiş cihazların canlı presence'ı (imzalı heartbeat + panel
    /// cihaz listesi) — `SDK_SESSIONS_FILE` ile şifreli kalıcı.
    pub sdk_sessions: Arc<SdkSessionStore>,
    /// SDK relay hedefleri (stream key'ler şifreli kalıcı).
    pub relay: Arc<RelayTargetStore>,
    /// Sağlayıcı canlı testlerinin en son sonucu (bellek içi — "last tested at").
    pub provider_tests: Arc<ProviderTestStore>,
    /// Platform kanal eşleştirmeleri (Twitch Extension EBS çözümlemesi).
    pub bindings: Arc<ChannelBindingStore>,
    /// Kullanıcı → aktif yayın overlay token'ı (EBS sunucu içi çözümleme).
    pub active_streams: Arc<ActiveStreams>,
    /// Twitch Extension EBS durumu (JWT doğrulayıcı, link kodları, limitler).
    pub ext_twitch: Arc<ExtTwitch>,
    /// Faz 2/3 katalog waitlist'i için per-IP rate limiter (bellek içi).
    pub waitlist: Arc<WaitlistLimiter>,
    /// Yayıncı hesapları (1.4) — `USERS_FILE` ile şifreli kalıcı.
    pub users: Arc<UserStore>,
    /// Login 2FA + şifre sıfırlama kodları (bellek içi, kısa ömürlü).
    pub twofactor: Arc<TwoFactorStore>,
    /// `POST /api/auth/login` ve `/signup` brute-force koruması (per-IP, bellek içi).
    pub auth_limiter: Arc<AuthLimiter>,
    /// HttpOnly browser cookie refresh sessions (hashed, rotating, revocable).
    pub browser_sessions: Arc<SessionStore>,
    /// Panel → SDK komut kuyruğu (2.6b; bellek içi, tek seferlik teslim).
    pub sdk_commands: Arc<SdkCommandQueue>,
    /// Kullanıcı başına STT modu + benchmark sonucu (`STT_SETTINGS_FILE`).
    pub stt_settings: Arc<SttSettingsStore>,
    /// Kullanıcı başına hedef dil seçimi (Faz 2.0; `STREAM_SETTINGS_FILE`).
    pub stream_settings: Arc<StreamSettingsStore>,
    /// Ses klonlama model kayıtları + admin onay durumu (Faz 2.1; `VOICE_MODELS_FILE`).
    pub voice_models: Arc<VoiceModelStore>,
    /// "Founding 5" kapalı beta başvuruları + admin onay durumu (`FOUNDING_APPS_FILE`).
    pub founding: Arc<FoundingStore>,
    /// Kullanıcı başına TTS modu + benchmark sonucu (Faz 2.2b; `TTS_SETTINGS_FILE`).
    pub tts_settings: Arc<TtsSettingsStore>,
    /// Ham ses örneği, AES-256 şifreli (Faz 2.2; `VOICE_SAMPLES_FILE`).
    pub voice_samples: Arc<VoiceSampleStore>,
    /// Self-hosted GPU STT sunucu kayıtları (roadmap 2.6; `STT_SERVERS_FILE`).
    pub stt_servers: Arc<SttServerStore>,
    /// Bir sunucuyu SSH ile kurup hazır hale getiren implementasyon —
    /// `SshProvisioner` roadmap 2.6 adım 6'da eklenene kadar `FakeProvisioner`.
    pub stt_provisioner: Arc<dyn Provisioner>,
}

impl AppState {
    /// Test ve iskelet kurulumları için: verilen jwt + config + gateway
    /// dışındaki tüm alanlar boş/varsayılan. Alanlar `pub` — çağıran,
    /// ihtiyacına göre tek tek üzerine yazar (`state.logger = ...`).
    pub fn skeleton(jwt: Jwt, service_config: Arc<ServiceConfig>, gateway: Gateway) -> Self {
        let secrets = Arc::new(ManagedSecrets::env());
        let ext_twitch = Arc::new(ExtTwitch::from_secrets(&secrets));
        AppState {
            jwt: Arc::new(jwt),
            gateway: Arc::new(gateway),
            logger: Arc::new(Logger::in_memory(Arc::clone(&service_config))),
            overlay: Arc::new(OverlayHub::new(Arc::clone(&service_config))),
            service_config,
            integrations: Arc::new(Integrations::empty()),
            oauth_login: Arc::new(LoginProviders::empty()),
            billing: Arc::new(Billing::empty()),
            plans: Arc::new(PlanRegistry::with_defaults()),
            usage: Arc::new(UsageTracker::new()),
            secrets,
            bootstrap_admin: Arc::new(None),
            db: None,
            pairing: Arc::new(PairingStore::new()),
            pairing_claim_limiter: Arc::new(ClaimLimiter::new()),
            sdk_sessions: Arc::new(SdkSessionStore::in_memory()),
            relay: Arc::new(RelayTargetStore::in_memory()),
            provider_tests: Arc::new(ProviderTestStore::new()),
            bindings: Arc::new(ChannelBindingStore::new()),
            active_streams: Arc::new(ActiveStreams::new()),
            ext_twitch,
            waitlist: Arc::new(WaitlistLimiter::new()),
            users: Arc::new(UserStore::in_memory()),
            twofactor: Arc::new(TwoFactorStore::new()),
            auth_limiter: Arc::new(AuthLimiter::new()),
            browser_sessions: Arc::new(SessionStore::in_memory()),
            sdk_commands: Arc::new(SdkCommandQueue::new()),
            stt_settings: Arc::new(SttSettingsStore::in_memory()),
            stream_settings: Arc::new(StreamSettingsStore::in_memory()),
            voice_models: Arc::new(VoiceModelStore::in_memory()),
            founding: Arc::new(FoundingStore::in_memory()),
            tts_settings: Arc::new(TtsSettingsStore::in_memory()),
            voice_samples: Arc::new(VoiceSampleStore::in_memory()),
            stt_servers: Arc::new(SttServerStore::in_memory()),
            stt_provisioner: Arc::new(crate::stt_servers::provision::FakeProvisioner::always(Err(
                crate::stt_servers::provision::ProvisionError::Bootstrap(
                    "no provisioner configured".into(),
                ),
            ))),
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

impl FromRef<AppState> for Arc<ClaimLimiter> {
    fn from_ref(state: &AppState) -> Self {
        state.pairing_claim_limiter.clone()
    }
}

impl FromRef<AppState> for Arc<SdkSessionStore> {
    fn from_ref(state: &AppState) -> Self {
        state.sdk_sessions.clone()
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

impl FromRef<AppState> for Arc<ChannelBindingStore> {
    fn from_ref(state: &AppState) -> Self {
        state.bindings.clone()
    }
}

impl FromRef<AppState> for Arc<ActiveStreams> {
    fn from_ref(state: &AppState) -> Self {
        state.active_streams.clone()
    }
}

impl FromRef<AppState> for Arc<ExtTwitch> {
    fn from_ref(state: &AppState) -> Self {
        state.ext_twitch.clone()
    }
}

impl FromRef<AppState> for Arc<WaitlistLimiter> {
    fn from_ref(state: &AppState) -> Self {
        state.waitlist.clone()
    }
}

impl FromRef<AppState> for Arc<UserStore> {
    fn from_ref(state: &AppState) -> Self {
        state.users.clone()
    }
}

impl FromRef<AppState> for Arc<TwoFactorStore> {
    fn from_ref(state: &AppState) -> Self {
        state.twofactor.clone()
    }
}

impl FromRef<AppState> for Arc<SessionStore> {
    fn from_ref(state: &AppState) -> Self {
        state.browser_sessions.clone()
    }
}

impl FromRef<AppState> for Arc<SdkCommandQueue> {
    fn from_ref(state: &AppState) -> Self {
        state.sdk_commands.clone()
    }
}

impl FromRef<AppState> for Arc<SttSettingsStore> {
    fn from_ref(state: &AppState) -> Self {
        state.stt_settings.clone()
    }
}

impl FromRef<AppState> for Arc<StreamSettingsStore> {
    fn from_ref(state: &AppState) -> Self {
        state.stream_settings.clone()
    }
}

impl FromRef<AppState> for Arc<VoiceModelStore> {
    fn from_ref(state: &AppState) -> Self {
        state.voice_models.clone()
    }
}

impl FromRef<AppState> for Arc<FoundingStore> {
    fn from_ref(state: &AppState) -> Self {
        state.founding.clone()
    }
}

impl FromRef<AppState> for Arc<TtsSettingsStore> {
    fn from_ref(state: &AppState) -> Self {
        state.tts_settings.clone()
    }
}

impl FromRef<AppState> for Arc<VoiceSampleStore> {
    fn from_ref(state: &AppState) -> Self {
        state.voice_samples.clone()
    }
}

impl FromRef<AppState> for Arc<SttServerStore> {
    fn from_ref(state: &AppState) -> Self {
        state.stt_servers.clone()
    }
}

impl FromRef<AppState> for Arc<dyn Provisioner> {
    fn from_ref(state: &AppState) -> Self {
        state.stt_provisioner.clone()
    }
}
