//! Axum router kurulumu.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{header, Method, StatusCode},
    routing::{get, post, put},
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
        // Dashboard CORS yalnız yukarıdaki route'ları sarar (bkz. fn doc).
        .layer(cors)
        // Twitch Extension EBS yüzeyi (kendi Any-origin CORS'u, Twitch JWT doğrulamalı).
        .merge(ext_twitch_routes())
        // Faz 2/3 katalog waitlist'i (kendi CORS'u, public — auth yok).
        .merge(waitlist_routes())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
