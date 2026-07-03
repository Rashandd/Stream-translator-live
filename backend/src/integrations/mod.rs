//! Platform/multistream OAuth entegrasyonları — opsiyonel Streamlabs
//! bağlantısı (Faz 1.6) ve YouTube Native CC (Faz 1.10).
//!
//! SDK'nın kendi gömülü RTMP relay'i (Mod A) asıl dağıtım yöntemidir. Yayıncı
//! isterse ayrıca kendi Streamlabs hesabını OAuth ile bağlayabilir (Mod A').
//! YouTube bağlantısı ise native closed-caption gönderimi içindir (bkz.
//! [`youtube`] modülü). `client_id`/`client_secret` `SecretStore`'dan
//! (env/Vault) gelir — koda gömülü değil. Hangi planda sağlayıcının açık
//! olduğu `ServiceConfig`'ten (`integration.<provider>.plans`) okunur;
//! hard-coded değildir.
//!
//! Bu modül OAuth **akışını** (authorize → callback → token değişimi) sağlar.
//! Alınan token'ın AES ile şifrelenip DB'ye (`platform_accounts`) yazılması ve
//! bağlantı testinin gerçek sağlayıcı API'siyle yapılması follow-up'tır.
//!
//! İstemci kimlikleri (`client_id`/`client_secret`) açılışta değil **çağrı
//! anında** [`SecretStore`]'dan çözülür — admin panelden girilen kimlik
//! restart gerektirmeden devreye girer.

pub mod oauth;
pub mod token_store;
pub mod youtube;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::auth::authz::{AdminUser, AuthUser};
use crate::config::ServiceConfig;
use crate::secrets::{names, SecretRef, SecretStore};
use oauth::{exchange_code, OAuthError, OAuthProvider, StateStore};
use token_store::TokenVault;

/// Bir sağlayıcının uç noktaları + çağrı anında çözülen istemci kimlikleri.
struct ProviderCreds {
    provider: OAuthProvider,
    client_id: SecretRef,
    client_secret: SecretRef,
}

impl ProviderCreds {
    async fn is_configured(&self) -> bool {
        self.client_id.resolve().await.is_some() && self.client_secret.resolve().await.is_some()
    }
}

/// Kayıtlı OAuth sağlayıcıları + CSRF state deposu + HTTP istemcisi +
/// şifreli token deposu.
pub struct Integrations {
    providers: HashMap<String, ProviderCreds>,
    states: StateStore,
    http: reqwest::Client,
    tokens: Arc<TokenVault>,
}

/// Bağlantı sonucu (frontend'e döner). **Token asla dönmez** — sadece durum.
#[derive(Debug, Serialize)]
pub struct Connection {
    pub provider: String,
    pub user_id: String,
    pub connected: bool,
    pub expires_in: Option<u64>,
    pub has_refresh: bool,
}

/// Sağlayıcı uç nokta varsayılanları (secret değil).
///
/// NOT: Streamlabs/Google URL'leri sağlayıcı dokümanına göredir. İleride
/// bunlar da config/DB'den okunabilir. Restream/Castr desteği kaldırıldı —
/// SDK'nın kendi gömülü RTMP relay'i (Mod A) asıl dağıtım yöntemidir.
fn default_providers() -> Vec<(&'static str, OAuthProvider, &'static str, &'static str)> {
    vec![
        (
            "streamlabs",
            OAuthProvider {
                name: "streamlabs".into(),
                authorize_url: "https://streamlabs.com/api/v2.0/authorize".into(),
                token_url: "https://streamlabs.com/api/v2.0/token".into(),
                scopes: vec!["socket.token".into()],
                extra_authorize_params: Vec::new(),
            },
            names::STREAMLABS_CLIENT_ID,
            names::STREAMLABS_CLIENT_SECRET,
        ),
        (
            "youtube",
            OAuthProvider {
                name: "youtube".into(),
                authorize_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
                token_url: "https://oauth2.googleapis.com/token".into(),
                scopes: vec!["https://www.googleapis.com/auth/youtube.force-ssl".into()],
                // Google refresh token'ı sadece offline+consent ile verir
                // (YouTube Native CC'nin uzun ömürlü erişimi — 1.10 notu).
                extra_authorize_params: vec![
                    ("access_type".into(), "offline".into()),
                    ("prompt".into(), "consent".into()),
                ],
            },
            names::YOUTUBE_CLIENT_ID,
            names::YOUTUBE_CLIENT_SECRET,
        ),
    ]
}

impl Integrations {
    /// Sağlayıcı kimliklerini çağrı anında `SecretStore`'dan çözecek şekilde
    /// registry kurar. Kimliği eksik sağlayıcı "yapılandırılmamış" görünür;
    /// kimlik admin panelden girildiği anda restart'sız aktifleşir.
    pub fn from_secrets(secrets: Arc<dyn SecretStore>, tokens: Arc<TokenVault>) -> Self {
        let mut providers = HashMap::new();
        for (name, provider, id_key, secret_key) in default_providers() {
            providers.insert(
                name.to_string(),
                ProviderCreds {
                    provider,
                    client_id: SecretRef::store(Arc::clone(&secrets), id_key),
                    client_secret: SecretRef::store(Arc::clone(&secrets), secret_key),
                },
            );
        }
        Integrations {
            providers,
            states: StateStore::new(Duration::from_secs(600)),
            http: reqwest::Client::new(),
            tokens,
        }
    }

    /// Sağlayıcısı olmayan boş registry — entegrasyon yapılandırılmamış
    /// ortamlar (ve testler) için. Her sağlayıcı "unknown" döner.
    pub fn empty() -> Self {
        Integrations {
            providers: HashMap::new(),
            states: StateStore::new(Duration::from_secs(600)),
            http: reqwest::Client::new(),
            tokens: Arc::new(TokenVault::in_memory()),
        }
    }

    /// Şifreli token deposu (bağlantı durumu + API çağrıları için).
    pub fn tokens(&self) -> &TokenVault {
        &self.tokens
    }

    /// Test/manuel kurulum için doğrudan registry.
    #[cfg(test)]
    fn from_parts(providers: HashMap<String, ProviderCreds>) -> Self {
        Integrations {
            providers,
            states: StateStore::new(Duration::from_secs(600)),
            http: reqwest::Client::new(),
            tokens: Arc::new(TokenVault::in_memory()),
        }
    }

    /// Sıralı sağlayıcı adları.
    pub fn provider_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.providers.keys().cloned().collect();
        names.sort();
        names
    }

    /// Sağlayıcının istemci kimliği yapılandırılmış mı?
    pub async fn is_configured(&self, name: &str) -> bool {
        match self.providers.get(name) {
            Some(creds) => creds.is_configured().await,
            None => false,
        }
    }

    /// authorize adımını başlatır: plan/erişim doğrular, `state` üretir ve
    /// yayıncının yönlendirileceği authorize URL'sini döndürür.
    pub async fn begin(
        &self,
        user_id: &str,
        plan: &str,
        provider_name: &str,
        cfg: &ServiceConfig,
    ) -> Result<String, OAuthError> {
        self.begin_inner(user_id, provider_name, cfg, Some(plan)).await
    }

    /// Admin doğrulama bağlantısı: plan erişim kontrolünü atlar. Faz 1'de
    /// broadcaster kullanıcı sistemi henüz yok — admin, kendi bootstrap
    /// hesabıyla (plan="streamer") bir sağlayıcının OAuth akışının uçtan
    /// uca çalıştığını doğrulamak için bağlanabilir. `Production Setup`
    /// sayfasındaki "Connect" aksiyonu bunu kullanır.
    pub async fn begin_admin(
        &self,
        user_id: &str,
        provider_name: &str,
        cfg: &ServiceConfig,
    ) -> Result<String, OAuthError> {
        self.begin_inner(user_id, provider_name, cfg, None).await
    }

    async fn begin_inner(
        &self,
        user_id: &str,
        provider_name: &str,
        cfg: &ServiceConfig,
        plan_check: Option<&str>,
    ) -> Result<String, OAuthError> {
        let creds = self
            .providers
            .get(provider_name)
            .ok_or_else(|| OAuthError::UnknownProvider(provider_name.to_string()))?;

        let client_id = creds
            .client_id
            .resolve()
            .await
            .ok_or_else(|| OAuthError::NotConfigured(provider_name.to_string()))?;

        if let Some(plan) = plan_check {
            let plans = cfg
                .get(&format!("integration.{provider_name}.plans"))
                .unwrap_or_default();
            if !oauth::plan_allowed(&plans, plan) {
                return Err(OAuthError::PlanNotAllowed {
                    provider: provider_name.to_string(),
                    plan: plan.to_string(),
                });
            }
        }

        let base = cfg
            .get("integration.redirect_base")
            .unwrap_or_else(|| "http://localhost:8080/api/integrations".to_string());
        let redirect_uri = format!("{base}/{provider_name}/callback");

        let state = self.states.issue(user_id, provider_name, &redirect_uri);
        Ok(creds
            .provider
            .authorize_redirect(&client_id, &redirect_uri, &state))
    }

    /// callback adımı: `state`'i doğrular, `code`'u token'a çevirir.
    /// Token'ı döndürmez; şifreli persist follow-up'tır.
    pub async fn complete(
        &self,
        provider_name: &str,
        code: &str,
        state: &str,
    ) -> Result<Connection, OAuthError> {
        let (user_id, stashed_provider, redirect_uri) =
            self.states.consume(state).ok_or(OAuthError::InvalidState)?;

        // Path'teki sağlayıcı, state'te saklananla eşleşmeli (karışıklık/CSRF).
        if stashed_provider != provider_name {
            return Err(OAuthError::InvalidState);
        }

        let creds = self
            .providers
            .get(provider_name)
            .ok_or_else(|| OAuthError::UnknownProvider(provider_name.to_string()))?;
        let (Some(client_id), Some(client_secret)) = (
            creds.client_id.resolve().await,
            creds.client_secret.resolve().await,
        ) else {
            return Err(OAuthError::NotConfigured(provider_name.to_string()));
        };

        let token = exchange_code(
            &self.http,
            &creds.provider,
            &client_id,
            &client_secret,
            code,
            &redirect_uri,
        )
        .await?;

        // Token AES-256 şifreli depoya yazılır (TOKENS_FILE ile kalıcı).
        // Persist hatası bağlantıyı düşürmez — token bu oturumda çalışır;
        // hata loglanır, restart sonrası yeniden bağlanmak gerekebilir.
        if let Err(err) = self.tokens.store(&user_id, provider_name, &token) {
            tracing::warn!(provider = %provider_name, error = %err, "oauth token persist failed");
        }

        Ok(Connection {
            provider: provider_name.to_string(),
            user_id,
            connected: true,
            expires_in: token.expires_in,
            has_refresh: token.refresh_token.is_some(),
        })
    }
}

/// `OAuthError` → HTTP durum kodu.
fn oauth_status(err: &OAuthError) -> StatusCode {
    match err {
        OAuthError::UnknownProvider(_) => StatusCode::NOT_FOUND,
        OAuthError::NotConfigured(_) => StatusCode::SERVICE_UNAVAILABLE,
        OAuthError::InvalidState => StatusCode::BAD_REQUEST,
        OAuthError::PlanNotAllowed { .. } => StatusCode::FORBIDDEN,
        OAuthError::Exchange(_) => StatusCode::BAD_GATEWAY,
    }
}

/// Bir sağlayıcının yayıncı paneline dönen durumu.
#[derive(Debug, Serialize)]
pub struct ProviderStatus {
    pub name: String,
    pub configured: bool,
    pub plans: String,
    pub enabled_for_plan: bool,
    /// Bu yayıncının kayıtlı (persist edilmiş) token'ı var mı?
    pub connected: bool,
}

/// `GET /api/integrations` — sağlayıcı listesi + plan erişimi (yayıncı).
pub async fn list(
    user: AuthUser,
    State(integ): State<Arc<Integrations>>,
    State(cfg): State<Arc<ServiceConfig>>,
) -> Json<Vec<ProviderStatus>> {
    let plan = &user.claims.plan;
    let mut items = Vec::new();
    for name in integ.provider_names() {
        let plans = cfg
            .get(&format!("integration.{name}.plans"))
            .unwrap_or_default();
        items.push(ProviderStatus {
            configured: integ.is_configured(&name).await,
            enabled_for_plan: oauth::plan_allowed(&plans, plan),
            connected: integ.tokens.connected(&user.claims.sub, &name),
            plans,
            name,
        });
    }
    Json(items)
}

/// authorize yanıtı — yayıncı bu URL'ye yönlendirilir.
#[derive(Debug, Serialize)]
pub struct AuthorizeResponse {
    pub authorize_url: String,
}

/// `GET /api/integrations/{provider}/authorize` (yayıncı).
pub async fn authorize(
    user: AuthUser,
    Path(provider): Path<String>,
    State(integ): State<Arc<Integrations>>,
    State(cfg): State<Arc<ServiceConfig>>,
) -> Result<Json<AuthorizeResponse>, (StatusCode, String)> {
    match integ
        .begin(&user.claims.sub, &user.claims.plan, &provider, &cfg)
        .await
    {
        Ok(authorize_url) => Ok(Json(AuthorizeResponse { authorize_url })),
        Err(err) => Err((oauth_status(&err), err.to_string())),
    }
}

/// `GET /admin/integrations/{provider}/authorize` (sadece admin).
///
/// Yayıncı akışındaki plan/erişim kontrolünü atlar (bkz. [`Integrations::begin_admin`]).
/// Production Setup sayfasındaki "Connect" aksiyonu operatörün kendi
/// bootstrap admin hesabıyla bir OAuth sağlayıcısının uçtan uca çalıştığını
/// doğrulaması için bunu çağırır.
pub async fn authorize_admin(
    admin: AdminUser,
    Path(provider): Path<String>,
    State(integ): State<Arc<Integrations>>,
    State(cfg): State<Arc<ServiceConfig>>,
) -> Result<Json<AuthorizeResponse>, (StatusCode, String)> {
    match integ.begin_admin(&admin.claims.sub, &provider, &cfg).await {
        Ok(authorize_url) => Ok(Json(AuthorizeResponse { authorize_url })),
        Err(err) => Err((oauth_status(&err), err.to_string())),
    }
}

/// callback query parametreleri.
#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

/// `GET /api/integrations/{provider}/callback` (sağlayıcıdan yönlendirme).
///
/// Kimlik `state`'ten çözülür (issue anında kullanıcıya bağlandı); ayrı auth
/// başlığı beklenmez. `integration.frontend_base` config'i doluysa (admin
/// panelden ayarlanır) sonucu JSON yerine frontend'e 302 redirect ile
/// bildirir (`?provider=...&connected=1` veya `&error=...`) — Production
/// Setup sayfası "Connect" akışını böyle tamamlar. Boşsa (varsayılan) geriye
/// dönük uyumlu JSON gövdesi döner.
pub async fn callback(
    Path(provider): Path<String>,
    Query(q): Query<CallbackQuery>,
    State(integ): State<Arc<Integrations>>,
    State(cfg): State<Arc<ServiceConfig>>,
    State(logger): State<Arc<crate::logging::Logger>>,
) -> axum::response::Response {
    use axum::response::{IntoResponse, Redirect};

    let frontend_base = cfg.get("integration.frontend_base").unwrap_or_default();
    let respond_error = |status: StatusCode, message: String| -> axum::response::Response {
        if frontend_base.is_empty() {
            (status, message).into_response()
        } else {
            let url = format!(
                "{frontend_base}/admin/production-setup?provider={provider}&error={}",
                urlencoding_light(&message)
            );
            Redirect::to(&url).into_response()
        }
    };

    if let Some(err) = q.error {
        return respond_error(StatusCode::BAD_REQUEST, format!("provider error: {err}"));
    }
    let (Some(code), Some(state)) = (q.code, q.state) else {
        return respond_error(StatusCode::BAD_REQUEST, "missing code or state".to_string());
    };

    match integ.complete(&provider, &code, &state).await {
        Ok(conn) => {
            logger.log(
                crate::logging::LogLevel::Info,
                "integrations",
                &format!("oauth connected: {provider}"),
                Some(&conn.user_id),
                None,
            );
            if frontend_base.is_empty() {
                Json(conn).into_response()
            } else {
                let url = format!("{frontend_base}/admin/production-setup?provider={provider}&connected=1");
                Redirect::to(&url).into_response()
            }
        }
        Err(err) => {
            logger.log(
                crate::logging::LogLevel::Warn,
                "integrations",
                &format!("oauth callback failed for {provider}: {err}"),
                None,
                None,
            );
            respond_error(oauth_status(&err), err.to_string())
        }
    }
}

/// Redirect query değeri için minimal URL-encode (bağımlılık eklemeden) —
/// sadece boşluk/`&`/`=`/`#` gibi URL'i bozan karakterleri kaçırır. Hata
/// mesajları operatöre gösterilecek kısa İngilizce metinlerdir, kullanıcı
/// girdisi taşımaz (secret asla bu yola girmez — bkz. `OAuthError` varyantları).
fn urlencoding_light(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            ' ' => "%20".to_string(),
            '&' => "%26".to_string(),
            '=' => "%3D".to_string(),
            '#' => "%23".to_string(),
            '%' => "%25".to_string(),
            other => other.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::Form;
    use axum::routing::post;
    use axum::Router;
    use std::collections::HashMap as Map;

    fn provider_at(token_url: &str, configured: bool) -> ProviderCreds {
        let secret_ref = |v: &str| {
            if configured {
                SecretRef::fixed(v)
            } else {
                SecretRef::none()
            }
        };
        ProviderCreds {
            provider: OAuthProvider {
                name: "streamlabs".into(),
                authorize_url: "https://streamlabs.com/api/v2.0/authorize".into(),
                token_url: token_url.into(),
                scopes: vec!["socket.token".into()],
                extra_authorize_params: Vec::new(),
            },
            client_id: secret_ref("cid"),
            client_secret: secret_ref("sec-1"),
        }
    }

    fn integrations_with(token_url: &str, configured: bool) -> Integrations {
        let mut map = HashMap::new();
        map.insert("streamlabs".to_string(), provider_at(token_url, configured));
        Integrations::from_parts(map)
    }

    fn cfg() -> ServiceConfig {
        ServiceConfig::with_defaults()
    }

    async fn spawn_token_mock() -> String {
        async fn token(Form(form): Form<Map<String, String>>) -> Json<serde_json::Value> {
            let _ = form;
            Json(serde_json::json!({
                "access_token": "at", "refresh_token": "rt", "expires_in": 3600
            }))
        }
        let app = Router::new().route("/oauth/token", post(token));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}/oauth/token")
    }

    #[tokio::test]
    async fn full_flow_begin_then_complete() {
        let token_url = spawn_token_mock().await;
        let integ = integrations_with(&token_url, true);
        let cfg = cfg();

        // begin: URL üretilmeli ve içinde bir state olmalı.
        let url = integ.begin("user-1", "pro", "streamlabs", &cfg).await.unwrap();
        let state = reqwest::Url::parse(&url)
            .unwrap()
            .query_pairs()
            .find(|(k, _)| k == "state")
            .map(|(_, v)| v.to_string())
            .expect("state in url");

        // complete: aynı state ile bağlantı kurulmalı.
        let conn = integ.complete("streamlabs", "code-1", &state).await.unwrap();
        assert!(conn.connected);
        assert_eq!(conn.user_id, "user-1");
        assert_eq!(conn.expires_in, Some(3600));
        assert!(conn.has_refresh);

        // Token persist edildi: bağlantı durumu artık kalıcı depodan okunur.
        assert!(integ.tokens().connected("user-1", "streamlabs"));
        let stored = integ.tokens().get("user-1", "streamlabs").expect("stored token");
        assert_eq!(stored.access_token, "at");
        assert_eq!(stored.refresh_token.as_deref(), Some("rt"));
        assert!(!integ.tokens().connected("user-2", "streamlabs"));
    }

    #[tokio::test]
    async fn youtube_authorize_url_requests_offline_refresh_token() {
        use crate::secrets::MemorySecretStore;

        let store = Arc::new(MemorySecretStore::with([
            (names::YOUTUBE_CLIENT_ID, "yt-cid"),
            (names::YOUTUBE_CLIENT_SECRET, "yt-sec"),
        ]));
        let integ = Integrations::from_secrets(
            store as Arc<dyn SecretStore>,
            Arc::new(TokenVault::in_memory()),
        );
        let cfg = cfg();
        let url = integ.begin("u", "pro", "youtube", &cfg).await.unwrap();
        // Google refresh token'ı ancak bu iki parametreyle verir (1.10).
        assert!(url.contains("access_type=offline"), "url: {url}");
        assert!(url.contains("prompt=consent"), "url: {url}");
    }

    #[tokio::test]
    async fn begin_rejects_plan_without_access() {
        let integ = integrations_with("http://unused", true);
        let cfg = cfg();
        // integration.streamlabs.plans = "pro" → "starter" reddedilir.
        let err = integ
            .begin("u", "starter", "streamlabs", &cfg)
            .await
            .unwrap_err();
        assert!(matches!(err, OAuthError::PlanNotAllowed { .. }));
    }

    #[tokio::test]
    async fn begin_rejects_unknown_and_unconfigured() {
        let cfg = cfg();
        let integ = integrations_with("http://unused", false);
        assert!(matches!(
            integ.begin("u", "pro", "ghost", &cfg).await,
            Err(OAuthError::UnknownProvider(_))
        ));
        assert!(matches!(
            integ.begin("u", "pro", "streamlabs", &cfg).await,
            Err(OAuthError::NotConfigured(_))
        ));
    }

    #[tokio::test]
    async fn creds_set_at_runtime_activate_without_restart() {
        use crate::secrets::MemorySecretStore;

        // Boş store ile kur → yapılandırılmamış.
        let store = Arc::new(MemorySecretStore::new());
        let integ = Integrations::from_secrets(
            Arc::clone(&store) as Arc<dyn SecretStore>,
            Arc::new(TokenVault::in_memory()),
        );
        let cfg = cfg();
        assert!(!integ.is_configured("streamlabs").await);
        assert!(matches!(
            integ.begin("u", "pro", "streamlabs", &cfg).await,
            Err(OAuthError::NotConfigured(_))
        ));

        // Admin panel kimliği girdi (restart yok) → authorize URL üretilir.
        store.insert(names::STREAMLABS_CLIENT_ID, "cid-live");
        store.insert(names::STREAMLABS_CLIENT_SECRET, "sec-live");
        assert!(integ.is_configured("streamlabs").await);
        let url = integ.begin("u", "pro", "streamlabs", &cfg).await.unwrap();
        assert!(url.contains("client_id=cid-live"));
    }

    #[tokio::test]
    async fn complete_rejects_unknown_state() {
        let integ = integrations_with("http://unused", true);
        let err = integ.complete("streamlabs", "code", "bogus-state").await.unwrap_err();
        assert!(matches!(err, OAuthError::InvalidState));
    }

    #[tokio::test]
    async fn begin_admin_skips_plan_check() {
        let integ = integrations_with("http://unused", true);
        let cfg = cfg();
        // integration.streamlabs.plans = "pro" — broadcaster "starter" ile
        // reddedilirdi (bkz. begin_rejects_plan_without_access); admin bunu
        // atlar çünkü sadece bağlantıyı doğrulamak istiyor.
        let url = integ
            .begin_admin("admin-bootstrap", "streamlabs", &cfg)
            .await
            .expect("admin bypasses plan gate");
        assert!(url.contains("client_id=cid"));
    }

    #[tokio::test]
    async fn complete_rejects_provider_mismatch() {
        let token_url = spawn_token_mock().await;
        let integ = integrations_with(&token_url, true);
        let cfg = cfg();
        let url = integ.begin("u", "pro", "streamlabs", &cfg).await.unwrap();
        let state = reqwest::Url::parse(&url)
            .unwrap()
            .query_pairs()
            .find(|(k, _)| k == "state")
            .map(|(_, v)| v.to_string())
            .unwrap();
        // state streamlabs için üretildi; başka sağlayıcı adıyla tüketmeye çalış → reddedilir.
        let err = integ.complete("other", "code", &state).await.unwrap_err();
        assert!(matches!(err, OAuthError::InvalidState));
    }

    async fn body_text(resp: axum::response::Response) -> String {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        String::from_utf8_lossy(&bytes).to_string()
    }

    #[tokio::test]
    async fn callback_returns_json_by_default_on_missing_code() {
        let integ = Arc::new(integrations_with("http://unused", true));
        let cfg = Arc::new(ServiceConfig::with_defaults()); // integration.frontend_base = "" (varsayılan)
        let logger = Arc::new(crate::logging::Logger::in_memory(Arc::clone(&cfg)));

        let resp = callback(
            Path("streamlabs".to_string()),
            Query(CallbackQuery { code: None, state: None, error: None }),
            State(integ),
            State(cfg),
            State(logger),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert!(resp.headers().get("location").is_none(), "should not redirect by default");
        assert!(body_text(resp).await.contains("missing code or state"));
    }

    #[tokio::test]
    async fn callback_redirects_to_frontend_when_configured() {
        let integ = Arc::new(integrations_with("http://unused", true));
        let cfg = Arc::new(ServiceConfig::with_defaults());
        cfg.set("integration.frontend_base", "https://panel.example.com");
        let logger = Arc::new(crate::logging::Logger::in_memory(Arc::clone(&cfg)));

        let resp = callback(
            Path("streamlabs".to_string()),
            Query(CallbackQuery { code: None, state: None, error: Some("access_denied".to_string()) }),
            State(integ),
            State(cfg),
            State(logger),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        let location = resp
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        assert!(location.starts_with("https://panel.example.com/admin/production-setup?"));
        assert!(location.contains("provider=streamlabs"));
        assert!(location.contains("error="));
    }
}
