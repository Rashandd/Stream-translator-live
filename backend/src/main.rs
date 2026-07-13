//! StreamDubber backend giriş noktası.

use std::net::SocketAddr;
use std::process::ExitCode;
use std::sync::Arc;

use axum::Router;
use streamdubber_backend::{
    app,
    auth::bootstrap::BootstrapAdmin,
    auth::challenge::ChallengeStore,
    auth::jwt::Jwt,
    auth::oauth_login::LoginProviders,
    auth::session::SessionStore,
    billing::Billing,
    bindings::{self, ChannelBindingStore},
    ext_twitch::ExtTwitch,
    founding::FoundingStore,
    config::{Config, ServiceConfig},
    crypto, db,
    gateway::{
        assemblyai::AssemblyAiStt,
        deepgram::DeepgramStt,
        deepl::DeeplTranslate,
        elevenlabs::ElevenLabsTts,
        google_translate::GoogleTranslate,
        selfhosted_stt::SelfHostedStt,
        stt::{MockFallbackStt, SttProvider},
        translate::{MockFallbackTranslate, TranslateProvider},
        tts::{MockFallbackTts, TtsProvider},
        Gateway,
    },
    integrations::{token_store::TokenVault, Integrations},
    logging::Logger,
    overlay::{ActiveStreams, OverlayHub},
    pairing::{ClaimLimiter, PairingStore},
    plans::PlanRegistry,
    provider_check::ProviderTestStore,
    rate_limit::AuthLimiter,
    relay::RelayTargetStore,
    sdk_link,
    sdk_session::SdkSessionStore,
    secrets::{names, EnvSecretStore, ManagedSecrets, SecretRef, SecretStore, VaultSecretStore},
    state::AppState,
    stream_settings,
    stt_servers::provision::{FakeProvisioner, ProvisionError, Provisioner, SshProvisioner},
    stt_servers::SttServerStore,
    usage::UsageTracker,
    users::{TwoFactorStore, UserStore},
    tls::{self, TlsSettings},
    voice_models,
    waitlist::WaitlistLimiter,
};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!(error = %err, "fatal startup error");
            ExitCode::FAILURE
        }
    }
}

/// Konfigürasyonu yükler, state'i kurar, dinleyiciyi bağlar ve sunucuyu çalıştırır.
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Container healthcheck modu: /health'e bağlan, çık. (distroless'ta curl yok.)
    if std::env::args().any(|a| a == "--health-check") {
        return health_check();
    }

    let config = Config::from_env()?;

    // Taban secret kaynağı — VAULT_ADDR+VAULT_TOKEN varsa Vault, yoksa env.
    let (base_store, base_label): (Box<dyn SecretStore>, &'static str) =
        match VaultSecretStore::from_env() {
            Some(vault) => {
                tracing::info!("using Vault secret store");
                (Box::new(vault), "vault")
            }
            None => {
                tracing::info!("using env secret store (VAULT_ADDR/VAULT_TOKEN not set)");
                (Box::new(EnvSecretStore), "env")
            }
        };

    // Admin panel katmanı: panelden girilen secret'lar tabanın önüne geçer.
    // SECRETS_FILE verilirse AES-256 şifreli kalıcıdır (restart'ta korunur).
    let secrets = Arc::new(build_managed_secrets(base_store, base_label).await?);

    // DATABASE_URL verilmişse bağlan + migration uygula; pool state'e girer
    // (Stripe webhook persist, usage kalıcılığı).
    let db_pool = match &config.database_url {
        Some(url) => {
            let pool = db::connect(url).await?;
            db::run_migrations(&pool).await?;
            tracing::info!("database migrations applied");
            Some(pool)
        }
        None => {
            tracing::warn!("DATABASE_URL not set — running without database");
            None
        }
    };

    let state = build_state(secrets, db_pool).await?;
    let router = app::router(state);

    match TlsSettings::from_env() {
        Some(settings) => serve_tls(config.bind_addr, router, settings).await,
        None => {
            tracing::warn!(
                "TLS not configured — serving plain HTTP (expect TLS-terminating proxy)"
            );
            serve_plain(config.bind_addr, router).await
        }
    }
}

/// `SECRETS_FILE`/`SECRETS_ENC_KEY` env'ine göre [`ManagedSecrets`] kurar.
///
/// - `SECRETS_FILE` boş → kalıcılık yok (panel girişleri restart'ta kaybolur).
/// - `SECRETS_ENC_KEY` (base64 veya hex, 32 byte) → dosya şifreleme anahtarı.
/// - `SECRETS_ENC_KEY` yoksa anahtar `JWT_SECRET`'tan türetilir (SHA-256).
async fn build_managed_secrets(
    base: Box<dyn SecretStore>,
    base_label: &'static str,
) -> Result<ManagedSecrets, Box<dyn std::error::Error>> {
    let path = std::env::var("SECRETS_FILE").ok().filter(|p| !p.is_empty());
    let Some(path) = path else {
        tracing::warn!("SECRETS_FILE not set — admin-entered secrets are lost on restart");
        return Ok(ManagedSecrets::new(base, base_label));
    };

    let key = resolve_enc_key(base.as_ref(), "streamdubber.secrets.v1").await?;

    tracing::info!(%path, "secret store: encrypted persistent file");
    Ok(ManagedSecrets::with_file(base, base_label, path, &key)?)
}

/// Şifreli dosyalar (secrets/tokens) için anahtar: `SECRETS_ENC_KEY` verilmişse
/// o, yoksa `JWT_SECRET`'tan bağlama özel türetme.
async fn resolve_enc_key(
    store: &dyn SecretStore,
    context: &str,
) -> Result<[u8; crypto::KEY_LEN], Box<dyn std::error::Error>> {
    match std::env::var("SECRETS_ENC_KEY").ok().filter(|k| !k.is_empty()) {
        Some(encoded) => parse_enc_key(&encoded),
        None => {
            tracing::info!(%context, "SECRETS_ENC_KEY not set — deriving key from JWT_SECRET");
            let jwt_secret = store.get(names::JWT_SECRET).await?;
            Ok(crypto::derive_key(jwt_secret.as_bytes(), context))
        }
    }
}

/// `SECRETS_ENC_KEY` değerini çözer: 64 karakter hex veya base64, 32 byte.
fn parse_enc_key(encoded: &str) -> Result<[u8; crypto::KEY_LEN], Box<dyn std::error::Error>> {
    use base64::Engine;
    let bytes = if encoded.len() == crypto::KEY_LEN * 2 {
        hex::decode(encoded).map_err(|e| format!("SECRETS_ENC_KEY invalid hex: {e}"))?
    } else {
        base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| format!("SECRETS_ENC_KEY invalid base64: {e}"))?
    };
    let len = bytes.len();
    bytes
        .try_into()
        .map_err(|_| format!("SECRETS_ENC_KEY must be {} bytes, got {len}", crypto::KEY_LEN).into())
}

/// Paylaşılan state'i kurar ve sağlayıcıları kaydeder.
///
/// JWT secret ve sağlayıcı key'leri [`SecretStore`]'dan çekilir — koda gömülmez.
/// Sağlayıcı key'leri **çağrı anında** çözülür: admin panelden girilen key
/// restart gerektirmeden devreye girer. Key yokken `providers.mock_fallback`
/// config'ine göre mock cevap (dev/CI) veya açık hata (üretim) üretilir.
async fn build_state(
    secrets: Arc<ManagedSecrets>,
    db_pool: Option<sqlx::PgPool>,
) -> Result<AppState, Box<dyn std::error::Error>> {
    let jwt_secret = secrets.get(names::JWT_SECRET).await?;

    let service_config = Arc::new(ServiceConfig::with_defaults());

    // CORS origin'leri env'den tohumla (panel de CORS'a tabi — ilk kurulum
    // için env şart; sonrası admin panelden restart'sız değiştirilir).
    if let Some(origins) = std::env::var("CORS_ALLOWED_ORIGINS")
        .ok()
        .filter(|v| !v.trim().is_empty())
    {
        service_config.set("cors.allowed_origins", origins.trim());
    }

    // DB varsa önceki oturumlarda admin panelden kaydedilmiş override'ları
    // (legal.*.html dahil) belleğe yükle — restart'ta kaybolmasınlar. Bu env
    // seed'inden SONRA çalışır: DB'de kayıtlı bir değer varsa (örn. admin
    // cors.allowed_origins'i panelden değiştirmişse) env yerine onu kullanır.
    if let Some(pool) = &db_pool {
        match streamdubber_backend::config::load_from_db(pool).await {
            Ok(rows) => service_config.apply_db_rows(rows),
            Err(err) => {
                tracing::warn!(%err, "service_config: DB'den yüklenemedi, varsayılanlar kullanılıyor")
            }
        }
    }

    // Plan kataloğu da aynı desende: kod-içi varsayılanlarla başlar, DB
    // varsa önceki admin düzenlemeleri üzerine yazılır (restart-safe).
    let plans = Arc::new(PlanRegistry::with_defaults());
    if let Some(pool) = &db_pool {
        match streamdubber_backend::plans::load_from_db(pool).await {
            Ok(rows) => plans.apply_db_rows(rows),
            Err(err) => {
                tracing::warn!(%err, "plans: DB'den yüklenemedi, varsayılanlar kullanılıyor")
            }
        }
    }

    let store: Arc<dyn SecretStore> = Arc::clone(&secrets) as Arc<dyn SecretStore>;

    let mut gateway = Gateway::new(Arc::clone(&service_config));

    // Key'i store'dan çözen sağlayıcı + mock fallback sarmalayıcısı.
    let stt = |name: &'static str, key: &'static str, real: Arc<dyn SttProvider>| {
        MockFallbackStt::new(
            name,
            SecretRef::store(Arc::clone(&store), key),
            real,
            Arc::clone(&service_config),
        )
    };
    let translate = |name: &'static str, key: &'static str, real: Arc<dyn TranslateProvider>| {
        MockFallbackTranslate::new(
            name,
            SecretRef::store(Arc::clone(&store), key),
            real,
            Arc::clone(&service_config),
        )
    };
    let tts = |name: &'static str, key: &'static str, real: Arc<dyn TtsProvider>| {
        MockFallbackTts::new(
            name,
            SecretRef::store(Arc::clone(&store), key),
            real,
            Arc::clone(&service_config),
        )
    };

    gateway.register_stt(Arc::new(stt(
        "deepgram",
        names::DEEPGRAM_KEY,
        Arc::new(DeepgramStt::managed(Arc::clone(&store))),
    )));
    gateway.register_stt(Arc::new(stt(
        "assemblyai",
        names::ASSEMBLYAI_KEY,
        Arc::new(AssemblyAiStt::managed(Arc::clone(&store))),
    )));
    gateway.register_translate(Arc::new(translate(
        "deepl",
        names::DEEPL_KEY,
        Arc::new(DeeplTranslate::managed(Arc::clone(&store))),
    )));
    gateway.register_translate(Arc::new(translate(
        "google",
        names::GOOGLE_TRANSLATE_KEY,
        Arc::new(GoogleTranslate::managed(Arc::clone(&store))),
    )));
    gateway.register_tts(Arc::new(tts(
        "elevenlabs",
        names::ELEVENLABS_KEY,
        Arc::new(ElevenLabsTts::managed(Arc::clone(&store))),
    )));
    tracing::info!("providers registered (keys resolved per-call from secret store)");

    // Multistream OAuth (1.6): client id/secret çağrı anında çözülür.
    // Token'lar TOKENS_FILE verilirse AES-256 şifreli kalıcıdır.
    let tokens_file = std::env::var("TOKENS_FILE").ok().filter(|p| !p.is_empty());
    let token_vault = match tokens_file {
        Some(path) => {
            let key = resolve_enc_key(secrets.as_ref(), "streamdubber.tokens.v1").await?;
            tracing::info!(%path, "oauth token store: encrypted persistent file");
            Arc::new(TokenVault::with_file(path, &key)?)
        }
        None => {
            tracing::warn!("TOKENS_FILE not set — oauth connections are lost on restart");
            Arc::new(TokenVault::in_memory())
        }
    };
    let integrations = Arc::new(Integrations::from_secrets(Arc::clone(&store), token_vault));

    // Google/Twitch ile login (auth::oauth_login) — bağlantı bağlama
    // (yukarıdaki Integrations) ile ayrı, kendi client id/secret'ları var.
    let oauth_login = Arc::new(LoginProviders::from_secrets(Arc::clone(&store)));

    // Stripe (1.11): secret'lar çağrı anında çözülür; yoksa checkout/webhook
    // "yapılandırılmamış" (503) döner.
    let billing = Arc::new(Billing::from_secrets(Arc::clone(&store)));

    // SDK relay hedefleri: RELAY_FILE verilirse stream key'ler AES-256
    // şifreli kalıcı; yoksa bellek içi (restart'ta yeniden girilir).
    let relay_file = std::env::var("RELAY_FILE").ok().filter(|p| !p.is_empty());
    let relay = match relay_file {
        Some(path) => {
            let key = resolve_enc_key(secrets.as_ref(), "streamdubber.relay.v1").await?;
            tracing::info!(%path, "relay target store: encrypted persistent file");
            Arc::new(RelayTargetStore::with_file(path, &key)?)
        }
        None => {
            tracing::warn!("RELAY_FILE not set — relay targets are lost on restart");
            Arc::new(RelayTargetStore::in_memory())
        }
    };

    // Eşleşmiş cihazların canlı presence'ı (heartbeat secret + cihaz adı):
    // SDK_SESSIONS_FILE verilirse AES-256 şifreli kalıcı; yoksa restart'ta
    // cihazlar yeniden pairing yapmadan da çalışmaya devam eder (device JWT
    // hâlâ geçerli) ama panel listesi/heartbeat state'i sıfırlanır.
    let sdk_sessions_file = std::env::var("SDK_SESSIONS_FILE").ok().filter(|p| !p.is_empty());
    let sdk_sessions = match sdk_sessions_file {
        Some(path) => {
            let key = resolve_enc_key(secrets.as_ref(), "streamdubber.sdk_sessions.v1").await?;
            tracing::info!(%path, "sdk session store: encrypted persistent file");
            Arc::new(SdkSessionStore::with_file(path, &key)?)
        }
        None => {
            tracing::warn!("SDK_SESSIONS_FILE not set — sdk device list is lost on restart");
            Arc::new(SdkSessionStore::in_memory())
        }
    };

    // Yayıncı hesapları (1.4): USERS_FILE verilirse AES-256 şifreli kalıcı;
    // DB pool'u varsa users tablosuna best-effort yansıtılır.
    let users_file = std::env::var("USERS_FILE").ok().filter(|p| !p.is_empty());
    let users = match users_file {
        Some(path) => {
            let key = resolve_enc_key(secrets.as_ref(), "streamdubber.users.v1").await?;
            tracing::info!(%path, "user store: encrypted persistent file");
            UserStore::with_file(path, &key)?
        }
        None => {
            tracing::warn!("USERS_FILE not set — user accounts are lost on restart");
            UserStore::in_memory()
        }
    }
    .with_db(db_pool.clone());

    // Kullanım sayaçları: USAGE_FILE verilirse kalıcı JSONL (restart'a
    // dayanır); DB pool'u varsa dakika sınırlarında usage_records upsert'i.
    let usage_file = std::env::var("USAGE_FILE").ok().filter(|p| !p.is_empty());
    let usage = match usage_file {
        Some(path) => {
            tracing::info!(%path, "usage store: persistent JSONL file");
            UsageTracker::with_file(&path)?
        }
        None => {
            tracing::warn!("USAGE_FILE not set — usage counters reset on restart");
            UsageTracker::new()
        }
    }
    .with_db(db_pool.clone());

    // Merkezi log deposu (1.12): LOG_FILE verilirse kalıcı JSONL, yoksa bellek
    // içi (restart'ta kaybolur — üretimde LOG_FILE bekleniyor).
    let log_file = std::env::var("LOG_FILE").ok().filter(|p| !p.is_empty());
    let logger = match log_file {
        Some(path) => {
            tracing::info!(%path, "log store: persistent JSONL file");
            Arc::new(Logger::with_file(Arc::clone(&service_config), &path)?)
        }
        None => {
            tracing::warn!("LOG_FILE not set — log store is in-memory (lost on restart)");
            Arc::new(Logger::in_memory(Arc::clone(&service_config)))
        }
    };

    // Kanal eşleştirmeleri: bellek içi depo; DB varsa açılışta tablodan yüklenir.
    let channel_bindings = Arc::new(ChannelBindingStore::new());
    if let Some(pool) = &db_pool {
        channel_bindings.load(bindings::load_bindings(pool, &logger).await);
    }

    // Env-bootstrap admin: yoksa admin panel girişi 503 döner (uyar).
    let bootstrap_admin = BootstrapAdmin::from_env();
    if bootstrap_admin.is_none() {
        tracing::warn!(
            "ADMIN_EMAIL/ADMIN_PASSWORD_HASH not set — admin panel login is disabled (503)"
        );
    }

    // STT mod tercihi + benchmark sonucu (2.6b): STT_SETTINGS_FILE verilirse
    // düz JSON kalıcı (secret içermez); yoksa bellek içi.
    let stt_settings_file = std::env::var("STT_SETTINGS_FILE")
        .ok()
        .filter(|p| !p.is_empty());
    let stt_settings = match stt_settings_file {
        Some(path) => {
            tracing::info!(%path, "stt settings store: persistent JSON file");
            Arc::new(sdk_link::SttSettingsStore::with_file(path)?)
        }
        None => {
            tracing::warn!("STT_SETTINGS_FILE not set — stt mode/benchmark reset on restart");
            Arc::new(sdk_link::SttSettingsStore::in_memory())
        }
    };

    // Yayıncının hedef dil seçimi (Faz 2.0): STREAM_SETTINGS_FILE verilirse
    // düz JSON kalıcı (dil kodları secret değil); yoksa bellek içi.
    let stream_settings_file = std::env::var("STREAM_SETTINGS_FILE")
        .ok()
        .filter(|p| !p.is_empty());
    let stream_settings = match stream_settings_file {
        Some(path) => {
            tracing::info!(%path, "stream language settings store: persistent JSON file");
            Arc::new(stream_settings::StreamSettingsStore::with_file(path)?)
        }
        None => {
            tracing::warn!(
                "STREAM_SETTINGS_FILE not set — target language selection resets on restart"
            );
            Arc::new(stream_settings::StreamSettingsStore::in_memory())
        }
    };

    // Ses klonlama model kayıtları (Faz 2.1): VOICE_MODELS_FILE verilirse
    // düz JSON kalıcı (metadata secret değil — ham ses örneği hiç
    // saklanmaz); yoksa bellek içi.
    let voice_models_file = std::env::var("VOICE_MODELS_FILE")
        .ok()
        .filter(|p| !p.is_empty());
    let voice_models = match voice_models_file {
        Some(path) => {
            tracing::info!(%path, "voice model store: persistent JSON file");
            Arc::new(voice_models::VoiceModelStore::with_file(path)?)
        }
        None => {
            tracing::warn!("VOICE_MODELS_FILE not set — voice model records reset on restart");
            Arc::new(voice_models::VoiceModelStore::in_memory())
        }
    };

    // "Founding 5" kapalı beta başvuruları: FOUNDING_APPS_FILE verilirse
    // düz JSON kalıcı (kanal linki secret değil); yoksa bellek içi.
    let founding_apps_file = std::env::var("FOUNDING_APPS_FILE")
        .ok()
        .filter(|p| !p.is_empty());
    let founding = match founding_apps_file {
        Some(path) => {
            tracing::info!(%path, "founding application store: persistent JSON file");
            Arc::new(FoundingStore::with_file(path)?)
        }
        None => {
            tracing::warn!("FOUNDING_APPS_FILE not set — founding applications reset on restart");
            Arc::new(FoundingStore::in_memory())
        }
    };

    // Yerel TTS modu + benchmark sonucu (Faz 2.2b): TTS_SETTINGS_FILE
    // verilirse düz JSON kalıcı (secret içermez); yoksa bellek içi.
    let tts_settings_file = std::env::var("TTS_SETTINGS_FILE")
        .ok()
        .filter(|p| !p.is_empty());
    let tts_settings = match tts_settings_file {
        Some(path) => {
            tracing::info!(%path, "tts settings store: persistent JSON file");
            Arc::new(sdk_link::TtsSettingsStore::with_file(path)?)
        }
        None => {
            tracing::warn!("TTS_SETTINGS_FILE not set — tts mode/benchmark reset on restart");
            Arc::new(sdk_link::TtsSettingsStore::in_memory())
        }
    };

    // Ham ses örneği (2026-07-06 kararı — "Öz Kaynaklı Ses Klonlama"):
    // VOICE_SAMPLES_FILE verilirse AES-256 şifreli kalıcı; yoksa bellek içi
    // (restart'ta kaybolur — yeniden eğitim/motor değişimi mümkün olmaz,
    // sadece dev/test için kabul edilir).
    let voice_samples_file = std::env::var("VOICE_SAMPLES_FILE")
        .ok()
        .filter(|p| !p.is_empty());
    let voice_samples = match voice_samples_file {
        Some(path) => {
            let key = resolve_enc_key(secrets.as_ref(), "streamdubber.voice_samples.v1").await?;
            tracing::info!(%path, "voice sample store: encrypted persistent file");
            Arc::new(voice_models::VoiceSampleStore::with_file(path, &key)?)
        }
        None => {
            tracing::warn!("VOICE_SAMPLES_FILE not set — raw voice samples are lost on restart");
            Arc::new(voice_models::VoiceSampleStore::in_memory())
        }
    };

    // Self-hosted GPU STT sunucuları (roadmap 2.6): STT_SERVERS_FILE
    // verilirse SSH private key/API token AES-256 şifreli kalıcı.
    let stt_servers_file = std::env::var("STT_SERVERS_FILE")
        .ok()
        .filter(|p| !p.is_empty());
    let stt_servers = match stt_servers_file {
        Some(path) => {
            let key = resolve_enc_key(secrets.as_ref(), "streamdubber.stt_servers.v1").await?;
            tracing::info!(%path, "stt server store: encrypted persistent file");
            Arc::new(SttServerStore::with_file(path, &key)?)
        }
        None => {
            tracing::warn!("STT_SERVERS_FILE not set — stt server records are lost on restart");
            Arc::new(SttServerStore::in_memory())
        }
    };
    // Sabit bir API key'i yok (host'u SttServerStore çözer) — MockFallbackStt
    // sarmalayıcısına ihtiyaç duymaz, `stt.provider=selfhosted` seçilince
    // doğrudan devreye girer.
    gateway.register_stt(Arc::new(SelfHostedStt::new(Arc::clone(&stt_servers))));

    // STT_BOOTSTRAP_SCRIPT: infra/stt-server/bootstrap.sh'in yolu.
    // STT_APP_DIR: stt-server/main.py + requirements.txt'in bulunduğu dizin
    // (bare-mode kurulum — Docker kullanılamayan ortamlar, ör. Vast.ai
    // container instance — için SshProvisioner bunları uzak makineye yazar).
    // Hiçbiri okunamazsa (yerel dev'de kopyalanmamış, image'a gömülmemiş vb.)
    // backend çökmez — provisioning açık bir hatayla başarısız olur, admin
    // panel "sunucu hazır" gibi yanıltıcı bir şey göstermez.
    let bootstrap_script_path = std::env::var("STT_BOOTSTRAP_SCRIPT")
        .ok()
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| "infra/stt-server/bootstrap.sh".to_string());
    let app_dir = std::env::var("STT_APP_DIR")
        .ok()
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| "stt-server".to_string());
    let main_py_path = format!("{app_dir}/main.py");
    let requirements_path = format!("{app_dir}/requirements.txt");
    let stt_provisioner: Arc<dyn Provisioner> =
        match SshProvisioner::from_paths(&bootstrap_script_path, &main_py_path, &requirements_path) {
            Ok(provisioner) => {
                tracing::info!(
                    bootstrap = %bootstrap_script_path,
                    app_dir = %app_dir,
                    "stt provisioner: real SshProvisioner"
                );
                Arc::new(provisioner)
            }
            Err(err) => {
                tracing::warn!(
                    bootstrap = %bootstrap_script_path,
                    app_dir = %app_dir,
                    %err,
                    "STT provisioner files not readable — self-hosted STT provisioning will fail until fixed"
                );
                Arc::new(FakeProvisioner::always(Err(ProvisionError::Bootstrap(format!(
                    "provisioner files not readable (bootstrap={bootstrap_script_path}, app_dir={app_dir}): {err}"
                )))))
            }
        };

    let browser_sessions = Arc::new(match &db_pool {
        Some(pool) => SessionStore::with_db(pool.clone()),
        None => SessionStore::in_memory(),
    });

    Ok(AppState {
        jwt: Arc::new(Jwt::new(jwt_secret.as_bytes())),
        gateway: Arc::new(gateway),
        overlay: Arc::new(OverlayHub::new(Arc::clone(&service_config))),
        ext_twitch: Arc::new(ExtTwitch::from_secrets(&secrets)),
        service_config,
        integrations,
        oauth_login,
        billing,
        plans,
        usage: Arc::new(usage),
        logger,
        secrets,
        bootstrap_admin: Arc::new(bootstrap_admin),
        db: db_pool,
        pairing: Arc::new(PairingStore::new()),
        pairing_claim_limiter: Arc::new(ClaimLimiter::new()),
        sdk_sessions,
        relay,
        provider_tests: Arc::new(ProviderTestStore::new()),
        bindings: channel_bindings,
        active_streams: Arc::new(ActiveStreams::new()),
        waitlist: Arc::new(WaitlistLimiter::new()),
        users: Arc::new(users),
        twofactor: Arc::new(TwoFactorStore::new()),
        challenges: Arc::new(ChallengeStore::default()),
        auth_limiter: Arc::new(AuthLimiter::new()),
        browser_sessions,
        sdk_commands: Arc::new(sdk_link::SdkCommandQueue::new()),
        stt_settings,
        stream_settings,
        stt_servers,
        stt_provisioner,
        voice_models,
        founding,
        tts_settings,
        voice_samples,
    })
}

/// Düz HTTP servis (TLS terminasyonu dışarıda).
async fn serve_plain(addr: SocketAddr, router: Router) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "backend listening (plain)");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// TLS 1.3 (+ opsiyonel mTLS) servis.
async fn serve_tls(
    addr: SocketAddr,
    router: Router,
    settings: TlsSettings,
) -> Result<(), Box<dyn std::error::Error>> {
    let server_config = tls::build_server_config(&settings)?;
    let rustls_config = axum_server::tls_rustls::RustlsConfig::from_config(server_config);

    let handle = axum_server::Handle::new();
    let shutdown_handle = handle.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        shutdown_handle.graceful_shutdown(Some(std::time::Duration::from_secs(5)));
    });

    tracing::info!(%addr, mtls = settings.mutual(), "backend listening (TLS 1.3)");
    axum_server::bind_rustls(addr, rustls_config)
        .handle(handle)
        .serve(router.into_make_service())
        .await?;
    Ok(())
}

/// Container healthcheck — `/health`'e basit bir HTTP GET atıp `200`'ü doğrular.
/// std-only (ek bağımlılık yok), distroless runtime'da çalışır.
fn health_check() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    let addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let port = addr.rsplit(':').next().unwrap_or("8080");
    let target = format!("127.0.0.1:{port}");

    let mut stream = TcpStream::connect(&target)?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    stream.write_all(b"GET /health HTTP/1.0\r\nHost: localhost\r\n\r\n")?;

    let mut response = String::new();
    stream.read_to_string(&mut response)?;

    if response
        .lines()
        .next()
        .is_some_and(|line| line.contains("200"))
    {
        Ok(())
    } else {
        Err(format!("health check failed: {:?}", response.lines().next()).into())
    }
}

/// Ctrl-C ile düzgün kapanma.
async fn shutdown_signal() {
    if let Err(err) = tokio::signal::ctrl_c().await {
        tracing::error!(error = %err, "failed to listen for shutdown signal");
    }
}
