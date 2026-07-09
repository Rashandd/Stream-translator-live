//! Axum router kurulumu.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{header, Method, StatusCode},
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::Deserialize;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::auth::authz::AdminUser;
use crate::billing;
use crate::config::ServiceConfig;
use crate::ext_twitch;
use crate::integrations;
use crate::logging::{LogEntry, LogLevel, LogQuery, Logger};
use crate::overlay;
use crate::plans::{PlanDefinition, PlanRegistry};
use crate::secrets::{ManagedSecrets, SecretError, SecretStatus};
use crate::state::AppState;
use crate::waitlist;
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
/// `DATABASE_URL` yapılandırılmışsa aynı anda `service_config` tablosuna da
/// yazılır (restart-safe); DB yazımı başarısız olursa bellek eski değere
/// geri alınır ve 500 döner — bellek ve DB asla sessizce ayrışmaz.
async fn put_config(
    _admin: AdminUser,
    State(cfg): State<Arc<ServiceConfig>>,
    State(db): State<Option<sqlx::PgPool>>,
    Json(update): Json<ConfigUpdate>,
) -> Result<Json<BTreeMap<String, String>>, StatusCode> {
    let previous = cfg.get(&update.key);
    if !cfg.set_existing(&update.key, update.value.clone()) {
        return Err(StatusCode::NOT_FOUND);
    }

    if let Some(pool) = db {
        if let Err(err) = crate::config::persist_to_db(&pool, &update.key, &update.value).await {
            tracing::warn!(%err, key = %update.key, "service_config: DB yazımı başarısız, bellek geri alınıyor");
            if let Some(prev) = previous {
                cfg.set_existing(&update.key, prev);
            }
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    Ok(Json(cfg.snapshot()))
}

/// Log kayıtlarını arar/filtreler (sadece admin) — Faz 1.12.
///
/// Filtreler: `service`, `level`, `from_ms`/`to_ms` (epoch ms), `user_id`,
/// `session_id`, `limit`. Son N hata için `level=error&limit=N`.
/// Geçersiz `level` değeri 400 döner (Query deserialize hatası).
async fn get_logs(
    _admin: AdminUser,
    State(logger): State<Arc<Logger>>,
    Query(query): Query<LogQuery>,
) -> Json<Vec<LogEntry>> {
    Json(logger.query(&query))
}

/// Yönetilen secret'ların durumunu listeler (sadece admin) — değerler asla
/// dönmez, sadece baş+son maskeli önizleme (`sk_l…3456`).
async fn get_secrets(
    _admin: AdminUser,
    State(secrets): State<Arc<ManagedSecrets>>,
) -> Json<Vec<SecretStatus>> {
    Json(secrets.status().await)
}

/// Secret güncelleme gövdesi.
#[derive(Deserialize)]
struct SecretUpdate {
    value: String,
}

/// `SecretError` → HTTP durum kodu.
fn secret_status_code(err: &SecretError) -> StatusCode {
    match err {
        SecretError::NotFound(_) => StatusCode::NOT_FOUND,
        SecretError::Backend(_) => StatusCode::BAD_REQUEST,
    }
}

/// Bir secret'ı admin panelden ayarlar (sadece admin).
///
/// Yalnızca yönetilen adlar (`secrets::names::MANAGED`) yazılabilir; değer
/// derhal devreye girer (restart yok) ve `SECRETS_FILE` yapılandırıldıysa
/// AES-256 şifreli kalıcıdır. Yanıt maskeli durum listesidir — değer dönmez.
async fn put_secret(
    admin: AdminUser,
    Path(name): Path<String>,
    State(secrets): State<Arc<ManagedSecrets>>,
    State(logger): State<Arc<Logger>>,
    Json(update): Json<SecretUpdate>,
) -> Result<Json<Vec<SecretStatus>>, (StatusCode, String)> {
    secrets
        .set(&name, &update.value)
        .map_err(|err| (secret_status_code(&err), err.to_string()))?;
    // Audit: kim, hangi secret'ı, ne zaman — değer asla loglanmaz.
    logger.log(
        LogLevel::Info,
        "secrets",
        &format!("secret updated: {name}"),
        Some(&admin.claims.sub),
        None,
    );
    Ok(Json(secrets.status().await))
}

/// Bir secret'ın panel override'ını kaldırır (env/Vault değerine geri düşer).
async fn delete_secret(
    admin: AdminUser,
    Path(name): Path<String>,
    State(secrets): State<Arc<ManagedSecrets>>,
    State(logger): State<Arc<Logger>>,
) -> Result<Json<Vec<SecretStatus>>, (StatusCode, String)> {
    secrets
        .clear(&name)
        .map_err(|err| (secret_status_code(&err), err.to_string()))?;
    logger.log(
        LogLevel::Info,
        "secrets",
        &format!("secret override cleared: {name}"),
        Some(&admin.claims.sub),
        None,
    );
    Ok(Json(secrets.status().await))
}

/// Plan listesini okur (public) — fiyatlandırma sayfası ve yayıncı paneli
/// buradan okur, koda gömülü fiyat/limit olmaz.
async fn get_plans(State(plans): State<Arc<PlanRegistry>>) -> Json<Vec<PlanDefinition>> {
    Json(plans.snapshot())
}

/// Bir planı ekler/günceller (sadece admin). Restart gerekmez.
async fn put_plan(
    _admin: AdminUser,
    State(plans): State<Arc<PlanRegistry>>,
    Json(plan): Json<PlanDefinition>,
) -> Json<Vec<PlanDefinition>> {
    plans.set(plan);
    Json(plans.snapshot())
}

/// Twitch Extension EBS route'ları (Faz 2.4) — extension frontend'i Twitch
/// CDN origin'inden (`https://<client-id>.ext-twitch.tv`) çağırır, bu yüzden
/// yalnız bu aile CORS'a açılır. Kimlik doğrulama CORS'a değil, her istekte
/// doğrulanan Twitch extension JWT'sine dayanır.
fn ext_twitch_routes() -> Router<AppState> {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);
    Router::new()
        .route("/ext/twitch/bind", post(ext_twitch::bind_channel))
        .route("/ext/twitch/state/{channel_id}", get(ext_twitch::get_state))
        .route(
            "/ext/twitch/captions/{channel_id}",
            get(ext_twitch::captions_ws),
        )
        .layer(cors)
}

/// Waitlist route'u — dashboard'un geri kalanının aksine tarayıcıdan doğrudan
/// `api.strovak.com`'a atılan gerçek bir public POST, bu yüzden kendi CORS'unu
/// taşır (diğer route ailelerinde henüz global bir CORS katmanı yok).
fn waitlist_routes() -> Router<AppState> {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::POST])
        .allow_headers([header::CONTENT_TYPE]);
    Router::new()
        .route("/api/waitlist", post(waitlist::submit))
        .layer(cors)
}

/// Dashboard/panel route ailesi için config-driven CORS.
///
/// Frontend (Vercel, `strovak.com`) tarayıcıdan **doğrudan**
/// `api.strovak.com`'a istek atar (CLAUDE.md: API proxy YASAK) — bu
/// cross-origin'dir ve preflight'ın geçmesi gerekir. İzinli origin listesi
/// `cors.allowed_origins` config anahtarından (virgülle ayrılmış) her
/// istekte okunur: admin panelden değiştirilebilir, restart gerektirmez.
/// İlk deploy'da `CORS_ALLOWED_ORIGINS` env'i bu anahtarı tohumlar
/// (panel de CORS'a tabi olduğu için yumurta-tavuk olmaz).
fn dashboard_cors(cfg: Arc<ServiceConfig>) -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(move |origin, _| {
            let Ok(origin) = origin.to_str() else {
                return false;
            };
            cfg.get("cors.allowed_origins")
                .map(|list| {
                    list.split(',')
                        .map(str::trim)
                        .filter(|allowed| !allowed.is_empty())
                        .any(|allowed| allowed == "*" || allowed.eq_ignore_ascii_case(origin))
                })
                .unwrap_or(false)
        }))
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
}

/// Uygulama router'ını kurar.
///
/// Dashboard CORS katmanı yalnız bu fonksiyondaki ana route'lara uygulanır;
/// `ext_twitch_routes` ve `waitlist_routes` sonradan merge edilir ve kendi
/// (daha geniş, Any-origin) CORS katmanlarını taşır — çift header oluşmaz.
pub fn router(state: AppState) -> Router {
    let cors = dashboard_cors(Arc::clone(&state.service_config));
    Router::new()
        .route("/health", get(health))
        .route("/ws/stream", get(ws_handler))
        // Tek giriş ucu (1.4): önce env-bootstrap admin, sonra yayıncı deposu.
        .route("/api/auth/login", post(crate::users::login))
        .route("/api/auth/signup", post(crate::users::signup))
        .route("/api/auth/2fa", post(crate::users::twofa_verify))
        .route("/api/auth/reset/request", post(crate::users::reset_request))
        .route("/api/auth/reset/confirm", post(crate::users::reset_confirm))
        // Google/Twitch ile login (auth yok — kapasite kod/state'te).
        .route(
            "/api/auth/oauth/{provider}/start",
            get(crate::auth::oauth_login::start),
        )
        .route(
            "/api/auth/oauth/{provider}/callback",
            get(crate::auth::oauth_login::callback),
        )
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
            "/api/integrations/{provider}/test",
            post(integrations::test_connection),
        )
        .route(
            "/api/integrations/{provider}/callback",
            get(integrations::callback),
        )
        // Admin doğrulama bağlantısı (V11): plan-gate atlanır, operatör kendi
        // bootstrap hesabıyla sağlayıcının OAuth akışını uçtan uca test eder.
        .route(
            "/admin/integrations/{provider}/authorize",
            get(integrations::authorize_admin),
        )
        // Stripe (1.11): yayıncı checkout başlatır, Stripe webhook'la olay bildirir.
        .route("/api/billing/checkout", post(billing::checkout))
        .route("/webhooks/stripe", post(billing::webhook))
        // SDK eşleştirme (1.4): panel kod üretir, SDK kodla token alır.
        .route("/api/pairing/code", post(crate::pairing::create_code))
        .route("/api/pairing/claim", post(crate::pairing::claim))
        // Twitch Extension (2.4) dashboard ucu: kanal bağlama kodu + durum.
        .route("/api/ext/twitch/code", post(ext_twitch::create_link_code))
        .route(
            "/api/ext/twitch/binding",
            get(ext_twitch::get_binding).delete(ext_twitch::delete_binding),
        )
        // SDK relay hedefleri (1.6): panel yazar, eşleşmiş SDK okur.
        .route(
            "/api/relay/targets",
            put(crate::relay::put_targets).get(crate::relay::get_targets),
        )
        // Yerel STT politikası (2.6b): SDK/panel, plan izni + kota çarpanı +
        // donanım/watchdog eşiklerini + kullanıcının mod/benchmark durumunu
        // tek çağrıda okur (config-driven).
        .route("/api/stt/policy", get(crate::stt_policy::get_policy))
        // STT mod tercihi (2.6b): panel yazar (local için benchmark şartı),
        // SDK policy yanıtından okur.
        .route("/api/stt/mode", put(crate::sdk_link::set_mode))
        // SDK komut kanalı (2.6b): panel komut bırakır, arka plandaki SDK
        // yoklar (tek seferlik teslim) ve benchmark sonucunu raporlar.
        .route(
            "/api/sdk/commands",
            post(crate::sdk_link::push_command).get(crate::sdk_link::poll_commands),
        )
        .route("/api/sdk/benchmark", post(crate::sdk_link::report_benchmark))
        // Canlı cihaz presence'ı: SDK imzalı heartbeat gönderir, panel
        // eşleşmiş cihazları listeler/kaldırır (bkz. `sdk_session` modülü).
        .route("/api/sdk/heartbeat", post(crate::sdk_session::heartbeat))
        .route("/api/sdk/devices", get(crate::sdk_session::list_devices))
        .route(
            "/api/sdk/devices/{id}",
            delete(crate::sdk_session::remove_device),
        )
        // Yerel TTS politikası (2.2b) — stt/policy ile aynı desen, XTTS-v2
        // için: plan izni + kota çarpanı + donanım/watchdog eşikleri +
        // kullanıcının mod/benchmark durumu tek çağrıda.
        .route("/api/tts/policy", get(crate::tts_policy::get_policy))
        // TTS mod tercihi (2.2b): panel yazar (local için benchmark şartı),
        // SDK policy yanıtından okur.
        .route("/api/tts/mode", put(crate::sdk_link::set_tts_mode))
        // TTS donanım testi sonucu (2.2b) — komut kanalı STT ile paylaşılır
        // (`tts_benchmark` whitelist'te), sonuç ayrı depoya yazılır.
        .route("/api/tts/benchmark", post(crate::sdk_link::report_tts_benchmark))
        // DeepL hedef dil matrisi (Faz 2.0): tam katalog + admin'in açtığı
        // diller + planın izin verdiği azami sayı + yayıncının kayıtlı
        // seçimi tek çağrıda; PUT seçimi doğrulayıp kalıcılaştırır.
        .route(
            "/api/translate/languages",
            get(crate::stream_settings::get_languages).put(crate::stream_settings::put_languages),
        )
        // Katalogun tamamı (auth gerekmez, gizli veri yok) — admin panelin
        // "hangi diller açık" listesini oluşturması için.
        .route("/api/translate/catalog", get(crate::stream_settings::get_catalog))
        // Ses klonlama (Faz 2.1): yayıncı örnek yollar, model oluşur
        // (pending_review) — admin onaylamadan Faz 2.2'de kullanılamaz.
        .route("/api/voice/clone", post(crate::voice_models::clone_voice))
        .route("/api/voice/models", get(crate::voice_models::list_my_voice_models))
        .route("/admin/voice/models", get(crate::voice_models::list_all_voice_models))
        .route(
            "/admin/voice/models/{id}/approve",
            post(crate::voice_models::approve_voice_model),
        )
        .route(
            "/admin/voice/models/{id}/reject",
            post(crate::voice_models::reject_voice_model),
        )
        // Dil ataması (Faz 2.2): admin, yayıncının hedef dili için hangi
        // onaylı modelin aktif kullanılacağını seçer.
        .route(
            "/admin/voice/models/{id}/assign",
            post(crate::voice_models::assign_voice_model),
        )
        // OBS Browser Source altyazı overlay'i (1.9): auth gerektirmez, token capability'dir.
        .route("/overlay/{token}", get(overlay::overlay_ws_handler))
        // Planlar (1.11): fiyat/limit hard-coded değil, admin panelden değişir.
        .route("/api/plans", get(get_plans))
        .route("/admin/plans", put(put_plan))
        // Loglama (1.12): merkezi log deposu araması, sadece admin.
        .route("/admin/logs", get(get_logs))
        // Secret yönetimi: admin panelden girilir, sadece maskeli okunur.
        .route("/admin/secrets", get(get_secrets))
        .route(
            "/admin/secrets/{name}",
            put(put_secret).delete(delete_secret),
        )
        // Sağlayıcı canlı bağlantı testi: girilen key gerçek API'ye karşı denenir.
        .route(
            "/admin/providers/{name}/test",
            post(crate::provider_check::test_provider),
        )
        // SMTP canlı testi: panelden girilen SMTP ayarlarıyla gerçek mail atar.
        .route("/admin/mail/test", post(crate::mail::mail_test))
        // Production Setup sihirbazı (V11): deploy/secret/provider/Stripe/OAuth
        // durumunu tek yanıtta toplar.
        .route(
            "/admin/setup/status",
            get(crate::setup_status::get_setup_status),
        )
        // Self-hosted GPU STT sunucuları (roadmap 2.6): admin herhangi bir
        // sağlayıcıdan GPU'lu VPS/VDS ekler, backend SSH ile kurar.
        .route(
            "/admin/stt-servers",
            get(crate::stt_servers::list_servers).post(crate::stt_servers::create_server),
        )
        .route(
            "/admin/stt-servers/{id}",
            get(crate::stt_servers::get_server).delete(crate::stt_servers::delete_server),
        )
        .route(
            "/admin/stt-servers/{id}/provision",
            post(crate::stt_servers::provision_server),
        )
        // Dashboard CORS yalnız yukarıdaki route'ları sarar (bkz. fn doc).
        .layer(cors)
        // Twitch Extension EBS yüzeyi (kendi Any-origin CORS'u, Twitch JWT doğrulamalı).
        .merge(ext_twitch_routes())
        // Faz 2/3 katalog waitlist'i (kendi CORS'u, public — auth yok).
        .merge(waitlist_routes())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
