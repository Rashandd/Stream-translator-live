//! Platform/multistream OAuth entegrasyonları — opsiyonel Streamlabs
//! bağlantısı (Faz 1.6) ve YouTube Native CC (Faz 1.10).
//!
//! SDK'nın kendi gömülü RTMP relay'i (Mod A) asıl dağıtım yöntemidir. Yayıncı
//! isterse ayrıca kendi Streamlabs hesabını OAuth ile bağlayabilir (Mod A').
//! YouTube bağlantısı ise native closed-caption gönderimi içindir (bkz.
//! [`youtube`] modülü). `client_id`/`client_secret` `SecretStore`'dan
//! (env/Vault) gelir — koda gömülü değil. Bir OAuth sağlayıcı yapılandırılmışsa
//! yayıncı planından bağımsız bağlanabilir.
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
use crate::founding::{FoundingPlatform, FoundingStore};
use crate::secrets::{names, SecretRef, SecretStore};
use crate::users::UserStore;
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

/// Yayıncının kayıtlı OAuth bağlantısına karşı yapılan canlı test sonucu.
#[derive(Debug, Serialize)]
pub struct ConnectionCheckOutcome {
    pub provider: String,
    pub ok: bool,
    pub connected: bool,
    /// Sağlayıcının döndürdüğü HTTP durum kodu (ağ hatasında `None`).
    pub status: Option<u16>,
    pub detail: String,
}

/// Test edilemeyen OAuth bağlantı durumları.
#[derive(Debug, thiserror::Error)]
pub enum ConnectionCheckError {
    #[error("unknown provider: {0}")]
    UnknownProvider(String),
    #[error("{0} is not connected")]
    NotConnected(String),
    #[error("{0} has no live connection test yet")]
    UnsupportedProvider(String),
    /// Access token süresi geçmiş, refresh_token yok ya da refresh isteği
    /// sağlayıcı tarafından reddedildi — kullanıcının yeniden bağlanması gerekir.
    #[error("{0} access token expired and could not be refreshed")]
    TokenRefreshFailed(String),
}

struct ConnectionCheckDef {
    provider: &'static str,
    url_key: &'static str,
    default_url: &'static str,
    auth: ConnectionAuthStyle,
}

enum ConnectionAuthStyle {
    /// `Authorization: Bearer <access_token>`.
    Bearer,
    /// Query param `access_token=<access_token>` (Streamlabs v1.0).
    QueryAccessToken,
    /// `Authorization: Bearer <access_token>` + `Client-Id: <client_id>`
    /// (Twitch Helix — access token tek başına yetmiyor). `client_id` bu
    /// sağlayıcının registry kaydından çağrı anında çözülür (static def
    /// içine gömülemez, `SecretStore` erişimi gerektirir).
    BearerWithClientId,
}

fn connection_check_defs() -> &'static [ConnectionCheckDef] {
    &[
        ConnectionCheckDef {
            provider: "streamlabs",
            url_key: "providers.streamlabs.test_url",
            default_url: "https://streamlabs.com/api/v1.0/socket/token",
            auth: ConnectionAuthStyle::QueryAccessToken,
        },
        ConnectionCheckDef {
            provider: "youtube",
            url_key: "providers.youtube.test_url",
            default_url: "https://www.googleapis.com/youtube/v3/channels?part=id&mine=true",
            auth: ConnectionAuthStyle::Bearer,
        },
        ConnectionCheckDef {
            provider: "twitch",
            url_key: "providers.twitch.test_url",
            default_url: "https://api.twitch.tv/helix/users",
            auth: ConnectionAuthStyle::BearerWithClientId,
        },
    ]
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
        (
            "twitch",
            OAuthProvider {
                name: "twitch".into(),
                authorize_url: "https://id.twitch.tv/oauth2/authorize".into(),
                token_url: "https://id.twitch.tv/oauth2/token".into(),
                // Sadece kimlik: founding-5 kanal doğrulaması için "hangi
                // Twitch kanalı" bilgisi yeterli, yayın/moderasyon izni
                // istenmiyor.
                scopes: vec!["user:read:email".into()],
                extra_authorize_params: Vec::new(),
            },
            names::TWITCH_CONNECT_CLIENT_ID,
            names::TWITCH_CONNECT_CLIENT_SECRET,
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

    /// Sağlayıcının istemci kimliğini çağrı anında çözer (örn. Twitch Helix
    /// `Client-Id` header'ı için). Sağlayıcı bilinmiyor/yapılandırılmamışsa `None`.
    pub async fn client_id(&self, provider_name: &str) -> Option<String> {
        self.providers.get(provider_name)?.client_id.resolve().await
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
        _plan: &str,
        provider_name: &str,
        cfg: &ServiceConfig,
    ) -> Result<String, OAuthError> {
        self.begin_inner(user_id, provider_name, cfg).await
    }

    /// Admin doğrulama bağlantısı. Production Setup sayfasındaki "Connect"
    /// aksiyonu operatörün kendi hesabıyla OAuth sağlayıcısının uçtan uca
    /// çalıştığını doğrulaması için bunu kullanır.
    pub async fn begin_admin(
        &self,
        user_id: &str,
        provider_name: &str,
        cfg: &ServiceConfig,
    ) -> Result<String, OAuthError> {
        self.begin_inner(user_id, provider_name, cfg).await
    }

    async fn begin_inner(
        &self,
        user_id: &str,
        provider_name: &str,
        cfg: &ServiceConfig,
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

    /// Kayıtlı token'ı döndürür; süresi dolmuşsa (60sn güvenlik payı) ve
    /// `refresh_token` varsa önce sağlayıcıdan yeniler ve kalıcı depoyu
    /// günceller. `refresh_token` yoksa/refresh reddedilirse kullanıcının
    /// yeniden bağlanması gerektiğini bildiren bir hata döner.
    pub async fn get_fresh_access_token(
        &self,
        user_id: &str,
        provider_name: &str,
    ) -> Result<String, ConnectionCheckError> {
        let creds = self
            .providers
            .get(provider_name)
            .ok_or_else(|| ConnectionCheckError::UnknownProvider(provider_name.to_string()))?;
        let stored = self
            .tokens
            .get(user_id, provider_name)
            .ok_or_else(|| ConnectionCheckError::NotConnected(provider_name.to_string()))?;

        let expires_soon = stored
            .expires_at
            .is_some_and(|exp| exp - 60 <= chrono::Utc::now().timestamp());
        if !expires_soon {
            return Ok(stored.access_token);
        }

        let Some(refresh_token) = stored.refresh_token.as_deref() else {
            return Err(ConnectionCheckError::TokenRefreshFailed(
                provider_name.to_string(),
            ));
        };
        let (Some(client_id), Some(client_secret)) = (
            creds.client_id.resolve().await,
            creds.client_secret.resolve().await,
        ) else {
            return Err(ConnectionCheckError::TokenRefreshFailed(
                provider_name.to_string(),
            ));
        };

        let refreshed = oauth::refresh_access_token(
            &self.http,
            &creds.provider,
            &client_id,
            &client_secret,
            refresh_token,
        )
        .await
        .map_err(|_| ConnectionCheckError::TokenRefreshFailed(provider_name.to_string()))?;

        let access_token = refreshed.access_token.clone();
        if let Err(err) = self
            .tokens
            .store_refreshed(user_id, provider_name, &refreshed)
        {
            tracing::warn!(provider = %provider_name, error = %err, "refreshed token persist failed");
        }
        Ok(access_token)
    }

    /// Kayıtlı OAuth token'ıyla sağlayıcıya hafif, yan etkisiz canlı çağrı yapar.
    pub async fn test_connection(
        &self,
        user_id: &str,
        provider_name: &str,
        cfg: &ServiceConfig,
        http: &reqwest::Client,
    ) -> Result<ConnectionCheckOutcome, ConnectionCheckError> {
        if !self.providers.contains_key(provider_name) {
            return Err(ConnectionCheckError::UnknownProvider(
                provider_name.to_string(),
            ));
        }
        let def = connection_check_defs()
            .iter()
            .find(|d| d.provider == provider_name)
            .ok_or_else(|| ConnectionCheckError::UnsupportedProvider(provider_name.to_string()))?;
        let access_token = self.get_fresh_access_token(user_id, provider_name).await?;

        let url = cfg
            .get(def.url_key)
            .filter(|u| !u.is_empty())
            .unwrap_or_else(|| def.default_url.to_string());

        let request = match def.auth {
            ConnectionAuthStyle::Bearer => http.get(url).bearer_auth(&access_token),
            ConnectionAuthStyle::QueryAccessToken => {
                let parsed = reqwest::Url::parse(&url);
                let Ok(mut url) = parsed else {
                    return Ok(ConnectionCheckOutcome {
                        provider: def.provider.to_string(),
                        ok: false,
                        connected: true,
                        status: None,
                        detail: "connection test URL is invalid".to_string(),
                    });
                };
                url.query_pairs_mut()
                    .append_pair("access_token", &access_token);
                http.get(url)
            }
            ConnectionAuthStyle::BearerWithClientId => {
                let mut req = http.get(url).bearer_auth(&access_token);
                if let Some(client_id) = self.client_id(provider_name).await {
                    req = req.header("Client-Id", client_id);
                }
                req
            }
        };

        match request.send().await {
            Ok(resp) => {
                let status = resp.status();
                Ok(ConnectionCheckOutcome {
                    provider: def.provider.to_string(),
                    ok: status.is_success(),
                    connected: true,
                    status: Some(status.as_u16()),
                    detail: if status.is_success() {
                        format!("provider responded {status}")
                    } else {
                        format!("provider rejected the stored token ({status})")
                    },
                })
            }
            Err(err) => Ok(ConnectionCheckOutcome {
                provider: def.provider.to_string(),
                ok: false,
                connected: true,
                status: None,
                detail: if err.is_timeout() {
                    "request timed out".to_string()
                } else {
                    "request failed".to_string()
                },
            }),
        }
    }
}

/// `OAuthError` → HTTP durum kodu.
fn oauth_status(err: &OAuthError) -> StatusCode {
    match err {
        OAuthError::UnknownProvider(_) => StatusCode::NOT_FOUND,
        OAuthError::NotConfigured(_) => StatusCode::SERVICE_UNAVAILABLE,
        OAuthError::InvalidState => StatusCode::BAD_REQUEST,
        OAuthError::Exchange(_) => StatusCode::BAD_GATEWAY,
    }
}

fn connection_check_status(err: &ConnectionCheckError) -> StatusCode {
    match err {
        ConnectionCheckError::UnknownProvider(_) => StatusCode::NOT_FOUND,
        ConnectionCheckError::NotConnected(_) => StatusCode::CONFLICT,
        ConnectionCheckError::UnsupportedProvider(_) => StatusCode::NOT_IMPLEMENTED,
        ConnectionCheckError::TokenRefreshFailed(_) => StatusCode::UNAUTHORIZED,
    }
}

/// Bir sağlayıcının yayıncı paneline dönen durumu.
#[derive(Debug, Serialize)]
pub struct ProviderStatus {
    pub name: String,
    pub configured: bool,
    /// Bu yayıncının kayıtlı (persist edilmiş) token'ı var mı?
    pub connected: bool,
}

/// `GET /api/integrations` — sağlayıcı listesi + bağlantı durumu (yayıncı).
pub async fn list(
    user: AuthUser,
    State(integ): State<Arc<Integrations>>,
) -> Json<Vec<ProviderStatus>> {
    let mut items = Vec::new();
    for name in integ.provider_names() {
        items.push(ProviderStatus {
            configured: integ.is_configured(&name).await,
            connected: integ.tokens.connected(&user.claims.sub, &name),
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

/// `POST /api/integrations/{provider}/test` (yayıncı).
pub async fn test_connection(
    user: AuthUser,
    Path(provider): Path<String>,
    State(integ): State<Arc<Integrations>>,
    State(cfg): State<Arc<ServiceConfig>>,
    State(logger): State<Arc<crate::logging::Logger>>,
) -> Result<Json<ConnectionCheckOutcome>, (StatusCode, String)> {
    let http = reqwest::Client::new();
    match integ
        .test_connection(&user.claims.sub, &provider, &cfg, &http)
        .await
    {
        Ok(outcome) => {
            logger.log(
                crate::logging::LogLevel::Info,
                "integrations",
                &format!(
                    "connection check: {} → {}",
                    outcome.provider,
                    if outcome.ok { "ok" } else { "failed" }
                ),
                Some(&user.claims.sub),
                None,
            );
            Ok(Json(outcome))
        }
        Err(err) => Err((connection_check_status(&err), err.to_string())),
    }
}

/// `DELETE /api/integrations/{provider}` (yayıncı) — kayıtlı token'ı siler.
/// Bulunamasa da idempotent olarak `204` döner (bkz. `TokenVault::remove`).
pub async fn disconnect(
    user: AuthUser,
    Path(provider): Path<String>,
    State(integ): State<Arc<Integrations>>,
) -> Result<StatusCode, (StatusCode, String)> {
    integ
        .tokens()
        .remove(&user.claims.sub, &provider)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
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
    State(founding): State<Arc<FoundingStore>>,
    State(users): State<Arc<UserStore>>,
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
            // Waitlist = founding-5 akışının genişletilmesi: gerçek bir
            // kanal bağlandığında (Streamlabs hariç — o bir kanal değil,
            // multistream aggregator) hiç başvurusu olmayan kullanıcı için
            // otomatik başvuru oluştur. Best-effort: herhangi bir adım
            // başarısız olursa sessizce vazgeçilir, OAuth akışı bloklanmaz.
            auto_apply_founding(&integ, &founding, &users, &cfg, &conn).await;
            if frontend_base.is_empty() {
                Json(conn).into_response()
            } else {
                let url = format!(
                    "{frontend_base}/admin/production-setup?provider={provider}&connected=1"
                );
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

/// OAuth connect başarılı olunca, hiç founding başvurusu olmayan kullanıcı
/// için gerçek kanal kimliğini çözüp otomatik başvuru oluşturur. Streamlabs
/// bir "kanal" değil multistream aggregator olduğu için tetiklemez (çağıran
/// zaten sadece youtube/twitch için çağırır — burada da ikinci bir savunma
/// katmanı olarak platform eşlemesi yapılır). Her adım best-effort: hata
/// durumunda sadece loglanır, hiçbir şey panik/err döndürmez.
async fn auto_apply_founding(
    integ: &Integrations,
    founding: &FoundingStore,
    users: &UserStore,
    cfg: &ServiceConfig,
    conn: &Connection,
) {
    let platform = match conn.provider.as_str() {
        "youtube" => FoundingPlatform::Youtube,
        "twitch" => FoundingPlatform::Twitch,
        _ => return,
    };
    // auto_apply_if_absent zaten "var olan başvuruya dokunma" davranışını
    // garanti ediyor; burada erken çıkmak sadece gereksiz token/identity
    // fetch'ini önler.
    if founding.get(&conn.user_id).is_some() {
        return;
    }
    let Some(email) = users
        .persistent_get_by_id(&conn.user_id)
        .await
        .ok()
        .flatten()
        .map(|record| record.email)
    else {
        tracing::warn!(user_id = %conn.user_id, "auto founding: user record not found, skipping");
        return;
    };
    let access_token = match integ
        .get_fresh_access_token(&conn.user_id, &conn.provider)
        .await
    {
        Ok(token) => token,
        Err(err) => {
            tracing::warn!(provider = %conn.provider, error = %err, "auto founding: token unavailable, skipping");
            return;
        }
    };
    let client_id = if conn.provider == "twitch" {
        integ.client_id("twitch").await
    } else {
        None
    };
    let http = reqwest::Client::new();
    let Some(channel_url) = resolve_channel_url(
        &http,
        cfg,
        &conn.provider,
        &access_token,
        client_id.as_deref(),
    )
    .await
    else {
        tracing::warn!(provider = %conn.provider, "auto founding: channel identity fetch failed, skipping");
        return;
    };
    founding.auto_apply_if_absent(&conn.user_id, &email, platform, &channel_url);
}

/// Sağlayıcının kendi identity API'sinden yayıncının kanal URL'ini çözer.
/// Ağ/parse hatasında `None` — çağıran best-effort olarak vazgeçer.
async fn resolve_channel_url(
    http: &reqwest::Client,
    cfg: &ServiceConfig,
    provider_name: &str,
    access_token: &str,
    client_id: Option<&str>,
) -> Option<String> {
    match provider_name {
        "youtube" => {
            let url = cfg
                .get("integration.youtube.identity_url")
                .filter(|u| !u.is_empty())
                .unwrap_or_else(|| {
                    "https://www.googleapis.com/youtube/v3/channels?part=snippet&mine=true"
                        .to_string()
                });
            let resp = http.get(url).bearer_auth(access_token).send().await.ok()?;
            if !resp.status().is_success() {
                return None;
            }
            let body: serde_json::Value = resp.json().await.ok()?;
            let item = body.get("items")?.as_array()?.first()?;
            let id = item.get("id")?.as_str()?;
            let custom_url = item
                .get("snippet")
                .and_then(|s| s.get("customUrl"))
                .and_then(|v| v.as_str());
            Some(match custom_url {
                Some(handle) => format!("https://youtube.com/{handle}"),
                None => format!("https://youtube.com/channel/{id}"),
            })
        }
        "twitch" => {
            let client_id = client_id?;
            let url = cfg
                .get("integration.twitch.identity_url")
                .filter(|u| !u.is_empty())
                .unwrap_or_else(|| "https://api.twitch.tv/helix/users".to_string());
            let resp = http
                .get(url)
                .bearer_auth(access_token)
                .header("Client-Id", client_id)
                .send()
                .await
                .ok()?;
            if !resp.status().is_success() {
                return None;
            }
            let body: serde_json::Value = resp.json().await.ok()?;
            let item = body.get("data")?.as_array()?.first()?;
            let login = item.get("login")?.as_str()?;
            Some(format!("https://twitch.tv/{login}"))
        }
        _ => None,
    }
}

/// Redirect query değeri için minimal URL-encode (bağımlılık eklemeden) —
/// sadece boşluk/`&`/`=`/`#` gibi URL'i bozan karakterleri kaçırır. Hata
/// mesajları operatöre gösterilecek kısa İngilizce metinlerdir, kullanıcı
/// girdisi taşımaz (secret asla bu yola girmez — bkz. `OAuthError` varyantları).
/// `pub(crate)` — `auth::oauth_login`'in kendi callback redirect'i de reuse ediyor.
pub(crate) fn urlencoding_light(input: &str) -> String {
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
    use axum::routing::{get, post};
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

    async fn spawn_streamlabs_check_mock() -> String {
        async fn socket_token(
            Query(q): Query<Map<String, String>>,
        ) -> (StatusCode, Json<serde_json::Value>) {
            if q.get("access_token").map(String::as_str) == Some("at-good") {
                (
                    StatusCode::OK,
                    Json(serde_json::json!({ "socket_token": "socket-ok" })),
                )
            } else {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({ "error": "bad token" })),
                )
            }
        }
        let app = Router::new().route("/socket/token", get(socket_token));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}/socket/token")
    }

    #[tokio::test]
    async fn full_flow_begin_then_complete() {
        let token_url = spawn_token_mock().await;
        let integ = integrations_with(&token_url, true);
        let cfg = cfg();

        // begin: URL üretilmeli ve içinde bir state olmalı.
        let url = integ
            .begin("user-1", "pro", "streamlabs", &cfg)
            .await
            .unwrap();
        let state = reqwest::Url::parse(&url)
            .unwrap()
            .query_pairs()
            .find(|(k, _)| k == "state")
            .map(|(_, v)| v.to_string())
            .expect("state in url");

        // complete: aynı state ile bağlantı kurulmalı.
        let conn = integ
            .complete("streamlabs", "code-1", &state)
            .await
            .unwrap();
        assert!(conn.connected);
        assert_eq!(conn.user_id, "user-1");
        assert_eq!(conn.expires_in, Some(3600));
        assert!(conn.has_refresh);

        // Token persist edildi: bağlantı durumu artık kalıcı depodan okunur.
        assert!(integ.tokens().connected("user-1", "streamlabs"));
        let stored = integ
            .tokens()
            .get("user-1", "streamlabs")
            .expect("stored token");
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
    async fn begin_allows_any_plan_when_provider_is_configured() {
        let integ = integrations_with("http://unused", true);
        let cfg = cfg();
        let url = integ
            .begin("u", "starter", "streamlabs", &cfg)
            .await
            .expect("configured OAuth is available to every plan");
        assert!(url.contains("client_id=cid"));
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
        let err = integ
            .complete("streamlabs", "code", "bogus-state")
            .await
            .unwrap_err();
        assert!(matches!(err, OAuthError::InvalidState));
    }

    #[tokio::test]
    async fn begin_admin_uses_same_configured_oauth_rule() {
        let integ = integrations_with("http://unused", true);
        let cfg = cfg();
        let url = integ
            .begin_admin("admin-bootstrap", "streamlabs", &cfg)
            .await
            .expect("admin can verify configured OAuth");
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

    #[tokio::test]
    async fn streamlabs_connection_check_uses_persisted_token() {
        let test_url = spawn_streamlabs_check_mock().await;
        let integ = integrations_with("http://unused", true);
        integ
            .tokens()
            .store(
                "user-1",
                "streamlabs",
                &oauth::TokenResponse {
                    access_token: "at-good".into(),
                    refresh_token: Some("rt".into()),
                    expires_in: Some(3600),
                    token_type: Some("Bearer".into()),
                },
            )
            .unwrap();
        let cfg = cfg();
        cfg.set("providers.streamlabs.test_url", test_url);
        let http = reqwest::Client::new();

        let outcome = integ
            .test_connection("user-1", "streamlabs", &cfg, &http)
            .await
            .unwrap();

        assert!(outcome.ok);
        assert_eq!(outcome.status, Some(200));
        assert!(outcome.connected);
    }

    #[tokio::test]
    async fn twitch_is_registered_as_connect_provider() {
        use crate::secrets::MemorySecretStore;

        let store = Arc::new(MemorySecretStore::with([
            (names::TWITCH_CONNECT_CLIENT_ID, "tw-cid"),
            (names::TWITCH_CONNECT_CLIENT_SECRET, "tw-sec"),
        ]));
        let integ = Integrations::from_secrets(
            store as Arc<dyn SecretStore>,
            Arc::new(TokenVault::in_memory()),
        );
        let cfg = cfg();
        let url = integ.begin("u", "pro", "twitch", &cfg).await.unwrap();
        assert!(
            url.starts_with("https://id.twitch.tv/oauth2/authorize?"),
            "url: {url}"
        );
        assert!(url.contains("client_id=tw-cid"), "url: {url}");
        assert_eq!(integ.client_id("twitch").await.as_deref(), Some("tw-cid"));
    }

    async fn spawn_twitch_helix_users_mock() -> String {
        async fn users(headers: axum::http::HeaderMap) -> (StatusCode, Json<serde_json::Value>) {
            let bearer_ok = headers.get("authorization").and_then(|v| v.to_str().ok())
                == Some("Bearer at-good");
            let client_id_ok =
                headers.get("client-id").and_then(|v| v.to_str().ok()) == Some("tw-cid");
            if bearer_ok && client_id_ok {
                (
                    StatusCode::OK,
                    Json(serde_json::json!({ "data": [{ "id": "1", "login": "streamer1" }] })),
                )
            } else {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({ "error": "unauthorized" })),
                )
            }
        }
        let app = Router::new().route("/helix/users", get(users));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}/helix/users")
    }

    #[tokio::test]
    async fn twitch_connection_check_sends_bearer_and_client_id() {
        use crate::secrets::MemorySecretStore;

        let test_url = spawn_twitch_helix_users_mock().await;
        let store = Arc::new(MemorySecretStore::with([
            (names::TWITCH_CONNECT_CLIENT_ID, "tw-cid"),
            (names::TWITCH_CONNECT_CLIENT_SECRET, "tw-sec"),
        ]));
        let integ = Integrations::from_secrets(
            store as Arc<dyn SecretStore>,
            Arc::new(TokenVault::in_memory()),
        );
        integ
            .tokens()
            .store(
                "user-1",
                "twitch",
                &oauth::TokenResponse {
                    access_token: "at-good".into(),
                    refresh_token: Some("rt".into()),
                    expires_in: Some(3600),
                    token_type: Some("Bearer".into()),
                },
            )
            .unwrap();
        let cfg = cfg();
        cfg.set("providers.twitch.test_url", test_url);
        let http = reqwest::Client::new();

        let outcome = integ
            .test_connection("user-1", "twitch", &cfg, &http)
            .await
            .unwrap();

        assert!(outcome.ok, "detail: {}", outcome.detail);
        assert_eq!(outcome.status, Some(200));
    }

    #[tokio::test]
    async fn get_fresh_access_token_returns_stored_token_when_not_expired() {
        let integ = integrations_with("http://unused", true);
        integ
            .tokens()
            .store(
                "user-1",
                "streamlabs",
                &oauth::TokenResponse {
                    access_token: "at-current".into(),
                    refresh_token: Some("rt".into()),
                    expires_in: Some(3600),
                    token_type: Some("Bearer".into()),
                },
            )
            .unwrap();

        let token = integ
            .get_fresh_access_token("user-1", "streamlabs")
            .await
            .unwrap();
        assert_eq!(token, "at-current");
    }

    #[tokio::test]
    async fn get_fresh_access_token_refreshes_when_expired() {
        let token_url = spawn_token_mock().await;
        let integ = integrations_with(&token_url, true);
        // expires_in ihmal edilirse expires_at=None kalır (asla "expired soon"
        // sayılmaz) — refresh tetiklemek için doğrudan store() sonrası
        // negatif bir expires_at'e sahip bir kayıt kurmamız gerekiyor; en
        // kolay yol store() sonrası TokenVault'un iç durumunu taklit etmek
        // yerine expires_in=0 vermek (şimdi + 0 - 60 <= şimdi → refresh gerekir).
        integ
            .tokens()
            .store(
                "user-1",
                "streamlabs",
                &oauth::TokenResponse {
                    access_token: "at-old".into(),
                    refresh_token: Some("rt-old".into()),
                    expires_in: Some(0),
                    token_type: Some("Bearer".into()),
                },
            )
            .unwrap();

        let token = integ
            .get_fresh_access_token("user-1", "streamlabs")
            .await
            .unwrap();
        // spawn_token_mock her zaman access_token="at" döner (grant_type kontrol etmiyor).
        assert_eq!(token, "at");
        // Yenilenen token kalıcı depoya yazıldı.
        let stored = integ.tokens().get("user-1", "streamlabs").unwrap();
        assert_eq!(stored.access_token, "at");
    }

    #[tokio::test]
    async fn get_fresh_access_token_fails_without_refresh_token() {
        let integ = integrations_with("http://unused", true);
        integ
            .tokens()
            .store(
                "user-1",
                "streamlabs",
                &oauth::TokenResponse {
                    access_token: "at-old".into(),
                    refresh_token: None,
                    expires_in: Some(0),
                    token_type: Some("Bearer".into()),
                },
            )
            .unwrap();

        let err = integ
            .get_fresh_access_token("user-1", "streamlabs")
            .await
            .unwrap_err();
        assert!(matches!(err, ConnectionCheckError::TokenRefreshFailed(_)));
    }

    #[tokio::test]
    async fn connection_check_requires_connected_token() {
        let integ = integrations_with("http://unused", true);
        let cfg = cfg();
        let http = reqwest::Client::new();

        let err = integ
            .test_connection("user-1", "streamlabs", &cfg, &http)
            .await
            .unwrap_err();

        assert!(matches!(err, ConnectionCheckError::NotConnected(_)));
    }

    async fn body_text(resp: axum::response::Response) -> String {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        String::from_utf8_lossy(&bytes).to_string()
    }

    fn youtube_provider_at(token_url: &str) -> ProviderCreds {
        ProviderCreds {
            provider: OAuthProvider {
                name: "youtube".into(),
                authorize_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
                token_url: token_url.into(),
                scopes: vec!["https://www.googleapis.com/auth/youtube.force-ssl".into()],
                extra_authorize_params: vec![
                    ("access_type".into(), "offline".into()),
                    ("prompt".into(), "consent".into()),
                ],
            },
            client_id: SecretRef::fixed("yt-cid"),
            client_secret: SecretRef::fixed("yt-sec"),
        }
    }

    /// YouTube `channels?part=snippet&mine=true` uç noktasını taklit eder.
    async fn spawn_youtube_identity_mock() -> String {
        async fn channels() -> Json<serde_json::Value> {
            Json(serde_json::json!({
                "items": [{ "id": "UC123", "snippet": { "customUrl": "@teststreamer" } }]
            }))
        }
        let app = Router::new().route("/channels", get(channels));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}/channels")
    }

    #[tokio::test]
    async fn oauth_connect_creates_founding_application_for_new_user() {
        let token_url = spawn_token_mock().await;
        let identity_url = spawn_youtube_identity_mock().await;

        let mut providers = HashMap::new();
        providers.insert("youtube".to_string(), youtube_provider_at(&token_url));
        let integ = Arc::new(Integrations::from_parts(providers));

        let cfg = Arc::new(cfg());
        cfg.set("integration.youtube.identity_url", identity_url);
        let logger = Arc::new(crate::logging::Logger::in_memory(Arc::clone(&cfg)));
        let founding = Arc::new(crate::founding::FoundingStore::in_memory());
        let users = Arc::new(UserStore::in_memory());
        let user = users
            .create_oauth_user("youtube", "ext-1", "streamer@example.com", "starter")
            .unwrap();

        // begin_admin plan-gate'i atlar (streamlabs/youtube.plans burada
        // yapılandırılmadı); state'i doğru user_id ile issue eder.
        let authorize_url = integ.begin_admin(&user.id, "youtube", &cfg).await.unwrap();
        let state = reqwest::Url::parse(&authorize_url)
            .unwrap()
            .query_pairs()
            .find(|(k, _)| k == "state")
            .map(|(_, v)| v.to_string())
            .expect("state in url");

        let resp = callback(
            Path("youtube".to_string()),
            Query(CallbackQuery {
                code: Some("code-1".to_string()),
                state: Some(state),
                error: None,
            }),
            State(Arc::clone(&integ)),
            State(Arc::clone(&cfg)),
            State(logger),
            State(Arc::clone(&founding)),
            State(Arc::clone(&users)),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);

        let application = founding.get(&user.id).expect("auto-created application");
        assert!(application.auto_created_via_oauth);
        assert_eq!(
            application.platform,
            crate::founding::FoundingPlatform::Youtube
        );
        assert_eq!(application.channel_url, "https://youtube.com/@teststreamer");
        assert_eq!(application.email, "streamer@example.com");
        assert_eq!(application.status, crate::founding::FoundingStatus::Pending);
    }

    #[tokio::test]
    async fn oauth_connect_does_not_override_existing_manual_application() {
        let token_url = spawn_token_mock().await;
        let identity_url = spawn_youtube_identity_mock().await;

        let mut providers = HashMap::new();
        providers.insert("youtube".to_string(), youtube_provider_at(&token_url));
        let integ = Arc::new(Integrations::from_parts(providers));

        let cfg = Arc::new(cfg());
        cfg.set("integration.youtube.identity_url", identity_url);
        let logger = Arc::new(crate::logging::Logger::in_memory(Arc::clone(&cfg)));
        let founding = Arc::new(crate::founding::FoundingStore::in_memory());
        let users = Arc::new(UserStore::in_memory());
        let user = users
            .create_oauth_user("youtube", "ext-2", "manual@example.com", "starter")
            .unwrap();
        founding.upsert_application(
            &user.id,
            "manual@example.com",
            crate::founding::FoundingPlatform::Kick,
            "https://kick.com/manual-channel",
            "manual application",
        );

        let authorize_url = integ.begin_admin(&user.id, "youtube", &cfg).await.unwrap();
        let state = reqwest::Url::parse(&authorize_url)
            .unwrap()
            .query_pairs()
            .find(|(k, _)| k == "state")
            .map(|(_, v)| v.to_string())
            .expect("state in url");

        let resp = callback(
            Path("youtube".to_string()),
            Query(CallbackQuery {
                code: Some("code-1".to_string()),
                state: Some(state),
                error: None,
            }),
            State(integ),
            State(cfg),
            State(logger),
            State(Arc::clone(&founding)),
            State(users),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);

        // Manuel başvuru ezilmedi.
        let application = founding.get(&user.id).unwrap();
        assert!(!application.auto_created_via_oauth);
        assert_eq!(application.channel_url, "https://kick.com/manual-channel");
    }

    #[tokio::test]
    async fn callback_returns_json_by_default_on_missing_code() {
        let integ = Arc::new(integrations_with("http://unused", true));
        let cfg = Arc::new(ServiceConfig::with_defaults()); // integration.frontend_base = "" (varsayılan)
        let logger = Arc::new(crate::logging::Logger::in_memory(Arc::clone(&cfg)));
        let founding = Arc::new(crate::founding::FoundingStore::in_memory());
        let users = Arc::new(UserStore::in_memory());

        let resp = callback(
            Path("streamlabs".to_string()),
            Query(CallbackQuery {
                code: None,
                state: None,
                error: None,
            }),
            State(integ),
            State(cfg),
            State(logger),
            State(founding),
            State(users),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert!(
            resp.headers().get("location").is_none(),
            "should not redirect by default"
        );
        assert!(body_text(resp).await.contains("missing code or state"));
    }

    #[tokio::test]
    async fn callback_redirects_to_frontend_when_configured() {
        let integ = Arc::new(integrations_with("http://unused", true));
        let cfg = Arc::new(ServiceConfig::with_defaults());
        cfg.set("integration.frontend_base", "https://panel.example.com");
        let logger = Arc::new(crate::logging::Logger::in_memory(Arc::clone(&cfg)));
        let founding = Arc::new(crate::founding::FoundingStore::in_memory());
        let users = Arc::new(UserStore::in_memory());

        let resp = callback(
            Path("streamlabs".to_string()),
            Query(CallbackQuery {
                code: None,
                state: None,
                error: Some("access_denied".to_string()),
            }),
            State(integ),
            State(cfg),
            State(logger),
            State(founding),
            State(users),
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
