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

pub mod oauth;
pub mod youtube;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::auth::authz::AuthUser;
use crate::config::ServiceConfig;
use crate::secrets::{names, SecretStore};
use oauth::{exchange_code, OAuthError, OAuthProvider, StateStore};

/// Bir sağlayıcının uç noktaları + (varsa) çözümlenmiş istemci kimlikleri.
struct ProviderCreds {
    provider: OAuthProvider,
    client_id: Option<String>,
    client_secret: Option<String>,
}

impl ProviderCreds {
    fn is_configured(&self) -> bool {
        self.client_id.is_some() && self.client_secret.is_some()
    }
}

/// Kayıtlı OAuth sağlayıcıları + CSRF state deposu + HTTP istemcisi.
pub struct Integrations {
    providers: HashMap<String, ProviderCreds>,
    states: StateStore,
    http: reqwest::Client,
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
            },
            names::YOUTUBE_CLIENT_ID,
            names::YOUTUBE_CLIENT_SECRET,
        ),
    ]
}

impl Integrations {
    /// Sağlayıcı kimliklerini `SecretStore`'dan çözümleyerek registry kurar.
    /// Kimliği eksik sağlayıcı "yapılandırılmamış" olarak işaretlenir.
    pub async fn from_secrets(secrets: &dyn SecretStore) -> Self {
        let mut providers = HashMap::new();
        for (name, provider, id_key, secret_key) in default_providers() {
            let client_id = secrets.get(id_key).await.ok();
            let client_secret = secrets.get(secret_key).await.ok();
            providers.insert(
                name.to_string(),
                ProviderCreds {
                    provider,
                    client_id,
                    client_secret,
                },
            );
        }
        Integrations {
            providers,
            states: StateStore::new(Duration::from_secs(600)),
            http: reqwest::Client::new(),
        }
    }

    /// Sağlayıcısı olmayan boş registry — entegrasyon yapılandırılmamış
    /// ortamlar (ve testler) için. Her sağlayıcı "unknown" döner.
    pub fn empty() -> Self {
        Integrations {
            providers: HashMap::new(),
            states: StateStore::new(Duration::from_secs(600)),
            http: reqwest::Client::new(),
        }
    }

    /// Test/manuel kurulum için doğrudan registry.
    #[cfg(test)]
    fn from_parts(providers: HashMap<String, ProviderCreds>) -> Self {
        Integrations {
            providers,
            states: StateStore::new(Duration::from_secs(600)),
            http: reqwest::Client::new(),
        }
    }

    /// Sıralı sağlayıcı adları.
    pub fn provider_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.providers.keys().cloned().collect();
        names.sort();
        names
    }

    /// Sağlayıcının istemci kimliği yapılandırılmış mı?
    pub fn is_configured(&self, name: &str) -> bool {
        self.providers.get(name).is_some_and(ProviderCreds::is_configured)
    }

    /// authorize adımını başlatır: plan/erişim doğrular, `state` üretir ve
    /// yayıncının yönlendirileceği authorize URL'sini döndürür.
    pub fn begin(
        &self,
        user_id: &str,
        plan: &str,
        provider_name: &str,
        cfg: &ServiceConfig,
    ) -> Result<String, OAuthError> {
        let creds = self
            .providers
            .get(provider_name)
            .ok_or_else(|| OAuthError::UnknownProvider(provider_name.to_string()))?;

        let client_id = creds
            .client_id
            .as_deref()
            .ok_or_else(|| OAuthError::NotConfigured(provider_name.to_string()))?;

        let plans = cfg
            .get(&format!("integration.{provider_name}.plans"))
            .unwrap_or_default();
        if !oauth::plan_allowed(&plans, plan) {
            return Err(OAuthError::PlanNotAllowed {
                provider: provider_name.to_string(),
                plan: plan.to_string(),
            });
        }

        let base = cfg
            .get("integration.redirect_base")
            .unwrap_or_else(|| "http://localhost:8080/api/integrations".to_string());
        let redirect_uri = format!("{base}/{provider_name}/callback");

        let state = self.states.issue(user_id, provider_name, &redirect_uri);
        Ok(creds
            .provider
            .authorize_redirect(client_id, &redirect_uri, &state))
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
        let (Some(client_id), Some(client_secret)) =
            (creds.client_id.as_deref(), creds.client_secret.as_deref())
        else {
            return Err(OAuthError::NotConfigured(provider_name.to_string()));
        };

        let token = exchange_code(
            &self.http,
            &creds.provider,
            client_id,
            client_secret,
            code,
            &redirect_uri,
        )
        .await?;

        // TODO(follow-up): token'ı crypto::Cipher ile şifreleyip
        // platform_accounts'a yaz. Şu an sadece bağlantı durumu döner.
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
}

/// `GET /api/integrations` — sağlayıcı listesi + plan erişimi (yayıncı).
pub async fn list(
    user: AuthUser,
    State(integ): State<Arc<Integrations>>,
    State(cfg): State<Arc<ServiceConfig>>,
) -> Json<Vec<ProviderStatus>> {
    let plan = &user.claims.plan;
    let items = integ
        .provider_names()
        .into_iter()
        .map(|name| {
            let plans = cfg
                .get(&format!("integration.{name}.plans"))
                .unwrap_or_default();
            ProviderStatus {
                configured: integ.is_configured(&name),
                enabled_for_plan: oauth::plan_allowed(&plans, plan),
                plans,
                name,
            }
        })
        .collect();
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
    match integ.begin(&user.claims.sub, &user.claims.plan, &provider, &cfg) {
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
/// başlığı beklenmez.
pub async fn callback(
    Path(provider): Path<String>,
    Query(q): Query<CallbackQuery>,
    State(integ): State<Arc<Integrations>>,
    State(logger): State<Arc<crate::logging::Logger>>,
) -> Result<Json<Connection>, (StatusCode, String)> {
    if let Some(err) = q.error {
        return Err((StatusCode::BAD_REQUEST, format!("provider error: {err}")));
    }
    let (Some(code), Some(state)) = (q.code, q.state) else {
        return Err((StatusCode::BAD_REQUEST, "missing code or state".to_string()));
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
            Ok(Json(conn))
        }
        Err(err) => {
            logger.log(
                crate::logging::LogLevel::Warn,
                "integrations",
                &format!("oauth callback failed for {provider}: {err}"),
                None,
                None,
            );
            Err((oauth_status(&err), err.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::Form;
    use axum::routing::post;
    use axum::Router;
    use std::collections::HashMap as Map;

    fn provider_at(token_url: &str, configured: bool) -> ProviderCreds {
        ProviderCreds {
            provider: OAuthProvider {
                name: "streamlabs".into(),
                authorize_url: "https://streamlabs.com/api/v2.0/authorize".into(),
                token_url: token_url.into(),
                scopes: vec!["socket.token".into()],
            },
            client_id: configured.then(|| "cid".to_string()),
            client_secret: configured.then(|| "sec-1".to_string()),
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
        let url = integ.begin("user-1", "pro", "streamlabs", &cfg).unwrap();
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
    }

    #[test]
    fn begin_rejects_plan_without_access() {
        let integ = integrations_with("http://unused", true);
        let cfg = cfg();
        // integration.streamlabs.plans = "pro" → "starter" reddedilir.
        let err = integ.begin("u", "starter", "streamlabs", &cfg).unwrap_err();
        assert!(matches!(err, OAuthError::PlanNotAllowed { .. }));
    }

    #[test]
    fn begin_rejects_unknown_and_unconfigured() {
        let cfg = cfg();
        let integ = integrations_with("http://unused", false);
        assert!(matches!(
            integ.begin("u", "pro", "ghost", &cfg),
            Err(OAuthError::UnknownProvider(_))
        ));
        assert!(matches!(
            integ.begin("u", "pro", "streamlabs", &cfg),
            Err(OAuthError::NotConfigured(_))
        ));
    }

    #[tokio::test]
    async fn complete_rejects_unknown_state() {
        let integ = integrations_with("http://unused", true);
        let err = integ.complete("streamlabs", "code", "bogus-state").await.unwrap_err();
        assert!(matches!(err, OAuthError::InvalidState));
    }

    #[tokio::test]
    async fn complete_rejects_provider_mismatch() {
        let token_url = spawn_token_mock().await;
        let integ = integrations_with(&token_url, true);
        let cfg = cfg();
        let url = integ.begin("u", "pro", "streamlabs", &cfg).unwrap();
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
}
