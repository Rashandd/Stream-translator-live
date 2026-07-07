//! Login/signup via Google (YouTube hesabı) veya Twitch.
//!
//! Var olan `integrations::oauth` OAuth2 çekirdeğini (authorize URL kurma,
//! CSRF `state`, token değişimi) reuse eder ama **ayrı bir registry**'dir:
//! `integrations::Integrations` zaten var olan bir kullanıcının bir yayın
//! *hedefi* bağlaması içindir (broadcast-yazma scope'ları, örn. YouTube'un
//! `youtube.force-ssl`'i); bu modül *kimlik* içindir (kullanıcı henüz yok,
//! sadece e-posta/profil okuyan minimal scope yeter). İkisini karıştırmak
//! hem daha kötü bir consent ekranına ("giriş yapmak için kanalını yönetme
//! izni ver") hem de login JWT'sinin geniş bir broadcast token'la
//! karışmasına yol açardı.
//!
//! Akış: `GET /api/auth/oauth/{provider}/start` (auth yok) → sağlayıcıya
//! 302 → `GET /api/auth/oauth/{provider}/callback` → code exchange →
//! sağlayıcının kendi "ben kimim" uç noktasından kimlik çekilir →
//! [`crate::users::UserStore::find_by_oauth`] bulursa login; yoksa aynı
//! e-posta zaten (şifreli veya başka bir sağlayıcıyla) kayıtlıysa
//! **reddedilir** — otomatik bağlama yok, hesap ele geçirme riskini sıfırlar
//! (sağlayıcı e-postayı doğrulanmış saysa bile: Twitch/Kick bunu garanti
//! etmez). Hiçbiri değilse yeni OAuth-only hesap açılır. Sonuç, e-posta/
//! şifre login'iyle birebir aynı JWT ([`crate::users::issue_user_token`]
//! reuse edilir), frontend'e `#token=...` URL fragment'iyle iletilir (2FA
//! e-postasıyla aynı konvansiyon — token loglara/erişim kayıtlarına düşmez).
//!
//! Kick bilinçli olarak burada YOK: OAuth 2.1 + PKCE gerektiriyor (mevcut
//! [`exchange_code`] sadece client_secret akışını biliyor) ve endpoint'leri
//! (id.kick.com/api.kick.com) canlı doğrulanmadı — ayrı bir roadmap task'ı.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;

use crate::config::ServiceConfig;
use crate::integrations::oauth::{exchange_code, OAuthProvider, StateStore};
use crate::integrations::{urlencoding_light, CallbackQuery};
use crate::logging::LogLevel;
use crate::secrets::{names, SecretRef, SecretStore};
use crate::state::AppState;
use crate::users::{issue_user_token, LoginResponse, UserError, UserRecord};
#[cfg(test)]
use crate::users::UserStore;

/// `StateStore::issue` bir `user_id` bekliyor (yayın-hedefi bağlama akışında
/// zaten girişli bir kullanıcı olur); login'de henüz kullanıcı yok, bu
/// yüzden hiç okunmayan sabit bir placeholder geçilir.
const NO_USER_YET: &str = "-";

/// Login-amaçlı bir sağlayıcının uç noktaları + çağrı anında çözülen kimlikler.
///
/// `userinfo_url` `OAuthProvider`'ın parçası değil (o struct `integrations`
/// ile paylaşılıyor ve bağlama akışının "ben kimim" ihtiyacı yok) — burada
/// ayrı tutulması aynı zamanda testte gerçek Google/Twitch'e gitmeden mock
/// sunucuya işaret edilebilmesini sağlıyor.
struct LoginProviderCreds {
    provider: OAuthProvider,
    client_id: SecretRef,
    client_secret: SecretRef,
    userinfo_url: String,
}

/// Kayıtlı login sağlayıcıları + CSRF state deposu + HTTP istemcisi.
/// `integrations::Integrations`'ın aksine token'ları kalıcı saklamaz —
/// access token'ı yalnızca callback sırasında kimlik çekmek için kullanılır,
/// sonra atılır.
pub struct LoginProviders {
    providers: HashMap<String, LoginProviderCreds>,
    states: StateStore,
    http: reqwest::Client,
}

fn default_providers() -> Vec<(&'static str, OAuthProvider, &'static str, &'static str, &'static str)> {
    vec![
        (
            "google",
            OAuthProvider {
                name: "google".into(),
                authorize_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
                token_url: "https://oauth2.googleapis.com/token".into(),
                scopes: vec!["openid".into(), "email".into(), "profile".into()],
                extra_authorize_params: Vec::new(),
            },
            names::GOOGLE_LOGIN_CLIENT_ID,
            names::GOOGLE_LOGIN_CLIENT_SECRET,
            "https://www.googleapis.com/oauth2/v3/userinfo",
        ),
        (
            "twitch",
            OAuthProvider {
                name: "twitch".into(),
                authorize_url: "https://id.twitch.tv/oauth2/authorize".into(),
                token_url: "https://id.twitch.tv/oauth2/token".into(),
                scopes: vec!["user:read:email".into()],
                extra_authorize_params: Vec::new(),
            },
            names::TWITCH_CLIENT_ID,
            names::TWITCH_CLIENT_SECRET,
            "https://api.twitch.tv/helix/users",
        ),
    ]
}

impl LoginProviders {
    /// Sağlayıcı kimliklerini çağrı anında `SecretStore`'dan çözecek
    /// registry kurar — admin panelden girilen kimlik restart'sız aktifleşir
    /// (bkz. `Integrations::from_secrets`, aynı desen).
    pub fn from_secrets(secrets: Arc<dyn SecretStore>) -> Self {
        let mut providers = HashMap::new();
        for (name, provider, id_key, secret_key, userinfo_url) in default_providers() {
            providers.insert(
                name.to_string(),
                LoginProviderCreds {
                    provider,
                    client_id: SecretRef::store(Arc::clone(&secrets), id_key),
                    client_secret: SecretRef::store(Arc::clone(&secrets), secret_key),
                    userinfo_url: userinfo_url.to_string(),
                },
            );
        }
        LoginProviders {
            providers,
            states: StateStore::new(Duration::from_secs(600)),
            http: reqwest::Client::new(),
        }
    }

    /// Sağlayıcısı olmayan boş registry — testler ve entegrasyonu
    /// yapılandırılmamış ortamlar için (bkz. `Integrations::empty`, aynı desen).
    pub fn empty() -> Self {
        LoginProviders {
            providers: HashMap::new(),
            states: StateStore::new(Duration::from_secs(600)),
            http: reqwest::Client::new(),
        }
    }

    #[cfg(test)]
    fn from_parts(providers: HashMap<String, LoginProviderCreds>) -> Self {
        LoginProviders {
            providers,
            states: StateStore::new(Duration::from_secs(600)),
            http: reqwest::Client::new(),
        }
    }

    /// authorize adımı: yayıncının yönlendirileceği sağlayıcı URL'sini kurar.
    async fn begin(&self, provider_name: &str, cfg: &ServiceConfig) -> Result<String, OAuthLoginError> {
        let creds = self
            .providers
            .get(provider_name)
            .ok_or_else(|| OAuthLoginError::UnknownProvider(provider_name.to_string()))?;
        let client_id = creds
            .client_id
            .resolve()
            .await
            .ok_or_else(|| OAuthLoginError::NotConfigured(provider_name.to_string()))?;

        let base = cfg
            .get("auth.oauth_redirect_base")
            .unwrap_or_else(|| "http://localhost:8080/api/auth/oauth".to_string());
        let redirect_uri = format!("{base}/{provider_name}/callback");
        let state = self.states.issue(NO_USER_YET, provider_name, &redirect_uri);
        Ok(creds.provider.authorize_redirect(&client_id, &redirect_uri, &state))
    }

    /// callback adımı: `code`'u token'a çevirir, sağlayıcıdan kimlik çeker,
    /// var olan hesabı bulur ya da (e-posta çakışmıyorsa) yenisini açar, ve
    /// e-posta/şifre login'iyle aynı şekilde bir JWT döner.
    async fn complete(
        &self,
        app: &AppState,
        provider_name: &str,
        code: &str,
        state: &str,
    ) -> Result<String, OAuthLoginError> {
        let (_, stashed_provider, redirect_uri) =
            self.states.consume(state).ok_or(OAuthLoginError::InvalidState)?;
        if stashed_provider != provider_name {
            return Err(OAuthLoginError::InvalidState);
        }

        let creds = self
            .providers
            .get(provider_name)
            .ok_or_else(|| OAuthLoginError::UnknownProvider(provider_name.to_string()))?;
        let (Some(client_id), Some(client_secret)) =
            (creds.client_id.resolve().await, creds.client_secret.resolve().await)
        else {
            return Err(OAuthLoginError::NotConfigured(provider_name.to_string()));
        };

        let token = exchange_code(&self.http, &creds.provider, &client_id, &client_secret, code, &redirect_uri)
            .await
            .map_err(|e| OAuthLoginError::Exchange(e.to_string()))?;

        let identity = fetch_identity(
            &self.http,
            provider_name,
            &creds.userinfo_url,
            &client_id,
            &token.access_token,
        )
        .await?;

        if let Some(existing) = app.users.find_by_oauth(provider_name, &identity.external_id) {
            return issue_token_string(app, &existing);
        }
        if app.users.get(&identity.email).is_some() {
            return Err(OAuthLoginError::EmailExists);
        }

        let plan = app
            .service_config
            .get("auth.default_plan")
            .unwrap_or_else(|| "free".to_string());
        let user = app
            .users
            .create_oauth_user(provider_name, &identity.external_id, &identity.email, &plan)?;
        app.logger.log(
            LogLevel::Info,
            "auth",
            &format!("oauth signup via {provider_name}"),
            Some(&user.id),
            None,
        );
        issue_token_string(app, &user)
    }
}

fn issue_token_string(app: &AppState, user: &UserRecord) -> Result<String, OAuthLoginError> {
    match issue_user_token(app, user) {
        Ok(LoginResponse::Token { token, .. }) => Ok(token),
        Ok(LoginResponse::TwoFactorPending { .. }) => {
            Err(OAuthLoginError::Internal("unexpected 2fa challenge for oauth login".to_string()))
        }
        Err((_, message)) => Err(OAuthLoginError::Internal(format!("token issue failed: {message}"))),
    }
}

/// Sağlayıcıdan çekilen, hesap eşleme için yeterli minimal kimlik.
#[derive(Debug, Clone, PartialEq, Eq)]
struct OAuthIdentity {
    external_id: String,
    email: String,
}

/// Sağlayıcının "ben kimim" uç noktasından kimlik çeker. Sağlayıcı başına
/// yanıt şekli farklı olduğu için (Google düz obje, Twitch `{data:[...]}`,
/// Twitch ayrıca `Client-Id` header'ı da istiyor) tek bir generic soyutlama
/// yerine küçük bir match — sadece 2 sağlayıcı için trait/plugin sistemi
/// gereksiz olurdu.
async fn fetch_identity(
    http: &reqwest::Client,
    provider_name: &str,
    userinfo_url: &str,
    client_id: &str,
    access_token: &str,
) -> Result<OAuthIdentity, OAuthLoginError> {
    match provider_name {
        "google" => {
            #[derive(Debug, Deserialize)]
            struct GoogleUserInfo {
                sub: String,
                #[serde(default)]
                email: Option<String>,
            }
            let info: GoogleUserInfo = http
                .get(userinfo_url)
                .bearer_auth(access_token)
                .send()
                .await
                .map_err(|e| OAuthLoginError::Identity(e.to_string()))?
                .json()
                .await
                .map_err(|e| OAuthLoginError::Identity(e.to_string()))?;
            let email = info
                .email
                .ok_or_else(|| OAuthLoginError::Identity("google account has no email".to_string()))?;
            Ok(OAuthIdentity { external_id: info.sub, email })
        }
        "twitch" => {
            #[derive(Debug, Deserialize)]
            struct TwitchUser {
                id: String,
                #[serde(default)]
                email: Option<String>,
            }
            #[derive(Debug, Deserialize)]
            struct TwitchUsersResponse {
                data: Vec<TwitchUser>,
            }
            let resp: TwitchUsersResponse = http
                .get(userinfo_url)
                .bearer_auth(access_token)
                .header("Client-Id", client_id)
                .send()
                .await
                .map_err(|e| OAuthLoginError::Identity(e.to_string()))?
                .json()
                .await
                .map_err(|e| OAuthLoginError::Identity(e.to_string()))?;
            let user = resp
                .data
                .into_iter()
                .next()
                .ok_or_else(|| OAuthLoginError::Identity("twitch returned no user".to_string()))?;
            let email = user.email.ok_or_else(|| {
                OAuthLoginError::Identity("twitch account has no email (missing user:read:email scope?)".to_string())
            })?;
            Ok(OAuthIdentity { external_id: user.id, email })
        }
        other => Err(OAuthLoginError::UnknownProvider(other.to_string())),
    }
}

/// OAuth login akışı hataları.
#[derive(Debug, thiserror::Error)]
enum OAuthLoginError {
    #[error("unknown provider: {0}")]
    UnknownProvider(String),
    #[error("provider not configured: {0}")]
    NotConfigured(String),
    #[error("invalid or expired state")]
    InvalidState,
    #[error("token exchange failed: {0}")]
    Exchange(String),
    #[error("could not read account identity from provider: {0}")]
    Identity(String),
    #[error("an account with this email already exists — sign in with your password")]
    EmailExists,
    #[error("account error: {0}")]
    Account(#[from] UserError),
    #[error("internal error: {0}")]
    Internal(String),
}

fn oauth_login_status(err: &OAuthLoginError) -> StatusCode {
    match err {
        OAuthLoginError::UnknownProvider(_) => StatusCode::NOT_FOUND,
        OAuthLoginError::NotConfigured(_) => StatusCode::SERVICE_UNAVAILABLE,
        OAuthLoginError::InvalidState => StatusCode::BAD_REQUEST,
        OAuthLoginError::Exchange(_) | OAuthLoginError::Identity(_) => StatusCode::BAD_GATEWAY,
        OAuthLoginError::EmailExists => StatusCode::CONFLICT,
        OAuthLoginError::Account(_) | OAuthLoginError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// `GET /api/auth/oauth/{provider}/start` (auth yok) — sağlayıcıya 302.
pub async fn start(Path(provider): Path<String>, State(app): State<AppState>) -> Result<Response, (StatusCode, String)> {
    let url = app
        .oauth_login
        .begin(&provider, &app.service_config)
        .await
        .map_err(|e| (oauth_login_status(&e), e.to_string()))?;
    Ok(Redirect::to(&url).into_response())
}

/// `GET /api/auth/oauth/{provider}/callback` (sağlayıcıdan yönlendirme).
///
/// Başarı/hata, `integration.frontend_base` config'i doluysa (admin
/// panelden ayarlanır — `integrations::callback` ile aynı config) frontend'e
/// 302 redirect ile bildirilir; boşsa JSON döner. Token URL **fragment**'ında
/// gider (`#token=...`), query'de değil — sunucu erişim loglarına düşmesin
/// diye (2FA e-postasının URL'e yazılmaması ile aynı gerekçe).
pub async fn callback(
    Path(provider): Path<String>,
    Query(q): Query<CallbackQuery>,
    State(app): State<AppState>,
) -> Response {
    let frontend_base = app.service_config.get("integration.frontend_base").unwrap_or_default();
    let respond_error = |message: String| -> Response {
        if frontend_base.is_empty() {
            (StatusCode::BAD_REQUEST, message).into_response()
        } else {
            let url = format!(
                "{frontend_base}/login/oauth-complete?provider={provider}&error={}",
                urlencoding_light(&message)
            );
            Redirect::to(&url).into_response()
        }
    };

    if let Some(err) = q.error {
        return respond_error(format!("provider error: {err}"));
    }
    let (Some(code), Some(state)) = (q.code, q.state) else {
        return respond_error("missing code or state".to_string());
    };

    match app.oauth_login.complete(&app, &provider, &code, &state).await {
        Ok(token) => {
            app.logger.log(LogLevel::Info, "auth", &format!("oauth login completed via {provider}"), None, None);
            if frontend_base.is_empty() {
                axum::Json(serde_json::json!({ "token": token })).into_response()
            } else {
                let url = format!("{frontend_base}/login/oauth-complete#token={token}");
                Redirect::to(&url).into_response()
            }
        }
        Err(err) => {
            app.logger.log(
                LogLevel::Warn,
                "auth",
                &format!("oauth login failed for {provider}: {err}"),
                None,
                None,
            );
            respond_error(err.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::{get, post};
    use axum::Router;
    use std::collections::HashMap as Map;

    fn provider_at(base: &str, configured: bool) -> LoginProviderCreds {
        let secret_ref = |v: &str| {
            if configured {
                SecretRef::fixed(v)
            } else {
                SecretRef::none()
            }
        };
        LoginProviderCreds {
            provider: OAuthProvider {
                name: "google".into(),
                authorize_url: format!("{base}/authorize"),
                token_url: format!("{base}/token"),
                scopes: vec!["openid".into(), "email".into()],
                extra_authorize_params: Vec::new(),
            },
            client_id: secret_ref("cid"),
            client_secret: secret_ref("sec-1"),
            userinfo_url: format!("{base}/userinfo"),
        }
    }

    fn providers_with(base: &str, configured: bool) -> LoginProviders {
        let mut map = Map::new();
        map.insert("google".to_string(), provider_at(base, configured));
        LoginProviders::from_parts(map)
    }

    fn cfg() -> ServiceConfig {
        ServiceConfig::with_defaults()
    }

    fn skeleton_state(users: UserStore, oauth_login: LoginProviders) -> AppState {
        use crate::auth::jwt::Jwt;
        use crate::config::ServiceConfig;
        use crate::gateway::Gateway;

        let config = Arc::new(ServiceConfig::with_defaults());
        let gateway = Gateway::new(Arc::clone(&config));
        let mut state = AppState::skeleton(Jwt::new(b"test-secret"), config, gateway);
        state.users = Arc::new(users);
        state.oauth_login = Arc::new(oauth_login);
        state
    }

    /// Sahte Google token+userinfo uç noktaları — sabit `g-123` sub'ı, verilen e-posta.
    async fn spawn_google_mock(email: &str) -> String {
        async fn token(axum::extract::Form(_): axum::extract::Form<Map<String, String>>) -> axum::Json<serde_json::Value> {
            axum::Json(serde_json::json!({ "access_token": "at-xyz", "expires_in": 3600 }))
        }
        async fn userinfo(
            axum::extract::State(email): axum::extract::State<String>,
        ) -> axum::Json<serde_json::Value> {
            axum::Json(serde_json::json!({ "sub": "g-123", "email": email }))
        }
        let app = Router::new()
            .route("/token", post(token))
            .route("/userinfo", get(userinfo))
            .with_state(email.to_string());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    /// Sadece bu testler için: `begin()` üzerinden gerçek bir `state` üretip
    /// callback'in bekleyeceği kod/state ikilisini simüle eder.
    async fn begin_and_capture_state(providers: &LoginProviders) -> String {
        let url = providers.begin("google", &cfg()).await.unwrap();
        reqwest::Url::parse(&url)
            .unwrap()
            .query_pairs()
            .find(|(k, _)| k == "state")
            .map(|(_, v)| v.to_string())
            .expect("state in authorize url")
    }

    #[tokio::test]
    async fn complete_creates_a_new_account_on_first_login() {
        let base = spawn_google_mock("streamer@example.com").await;
        let providers = providers_with(&base, true);
        let state_param = begin_and_capture_state(&providers).await;

        let app = skeleton_state(UserStore::in_memory(), providers);
        let token = app
            .oauth_login
            .complete(&app, "google", "code-1", &state_param)
            .await
            .expect("first login creates an account");

        assert!(!token.is_empty());
        let user = app.users.find_by_oauth("google", "g-123").expect("account created");
        assert_eq!(user.email, "streamer@example.com");
        assert!(user.password_hash.is_none());
        assert!(user.email_verified);
    }

    #[tokio::test]
    async fn complete_logs_in_an_existing_linked_account_without_creating_a_duplicate() {
        let base = spawn_google_mock("streamer@example.com").await;
        let providers = providers_with(&base, true);

        let users = UserStore::in_memory();
        let existing = users
            .create_oauth_user("google", "g-123", "streamer@example.com", "free")
            .unwrap();
        let app = skeleton_state(users, providers);
        let state_param = begin_and_capture_state(&app.oauth_login).await;

        let token = app
            .oauth_login
            .complete(&app, "google", "code-1", &state_param)
            .await
            .expect("existing identity logs in");
        assert!(!token.is_empty());

        // Still exactly one account for this identity — no duplicate created.
        let by_email = app.users.get("streamer@example.com").expect("account exists");
        assert_eq!(by_email.id, existing.id);
    }

    #[tokio::test]
    async fn complete_rejects_when_email_already_belongs_to_a_password_account() {
        let base = spawn_google_mock("streamer@example.com").await;
        let providers = providers_with(&base, true);

        let users = UserStore::in_memory();
        users.signup("streamer@example.com", "hunter2-strong", "free").unwrap();
        let app = skeleton_state(users, providers);
        let state_param = begin_and_capture_state(&app.oauth_login).await;

        let err = app
            .oauth_login
            .complete(&app, "google", "code-1", &state_param)
            .await
            .unwrap_err();
        assert!(matches!(err, OAuthLoginError::EmailExists));

        // No new (OAuth-only) account was created for this identity.
        assert!(app.users.find_by_oauth("google", "g-123").is_none());
    }

    #[test]
    fn create_oauth_user_then_find_by_oauth_matches_across_a_fresh_lookup() {
        let users = UserStore::in_memory();
        let created = users
            .create_oauth_user("twitch", "t-1", "new@example.com", "free")
            .unwrap();
        let found = users.find_by_oauth("twitch", "t-1").expect("found");
        assert_eq!(found.id, created.id);
        assert!(found.password_hash.is_none());
    }

    #[test]
    fn oauth_login_status_maps_email_exists_to_conflict() {
        assert_eq!(oauth_login_status(&OAuthLoginError::EmailExists), StatusCode::CONFLICT);
        assert_eq!(
            oauth_login_status(&OAuthLoginError::InvalidState),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            oauth_login_status(&OAuthLoginError::NotConfigured("google".into())),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[tokio::test]
    async fn begin_builds_a_valid_authorize_url() {
        let providers = providers_with("http://unused", true);
        let url = providers.begin("google", &cfg()).await.unwrap();
        assert!(url.starts_with("http://unused/authorize?"));
        assert!(url.contains("client_id=cid"));
        assert!(url.contains("state="));
    }

    #[test]
    fn default_providers_point_at_the_real_endpoints() {
        let providers = default_providers();
        let (google_name, google, google_id_key, google_secret_key, google_userinfo) = &providers[0];
        assert_eq!(*google_name, "google");
        assert_eq!(google.authorize_url, "https://accounts.google.com/o/oauth2/v2/auth");
        assert_eq!(google.token_url, "https://oauth2.googleapis.com/token");
        assert_eq!(*google_userinfo, "https://www.googleapis.com/oauth2/v3/userinfo");
        assert_eq!(*google_id_key, names::GOOGLE_LOGIN_CLIENT_ID);
        assert_eq!(*google_secret_key, names::GOOGLE_LOGIN_CLIENT_SECRET);

        let (twitch_name, twitch, twitch_id_key, twitch_secret_key, twitch_userinfo) = &providers[1];
        assert_eq!(*twitch_name, "twitch");
        assert_eq!(twitch.authorize_url, "https://id.twitch.tv/oauth2/authorize");
        assert_eq!(twitch.token_url, "https://id.twitch.tv/oauth2/token");
        assert_eq!(*twitch_userinfo, "https://api.twitch.tv/helix/users");
        assert_eq!(*twitch_id_key, names::TWITCH_CLIENT_ID);
        assert_eq!(*twitch_secret_key, names::TWITCH_CLIENT_SECRET);
    }

    #[tokio::test]
    async fn begin_rejects_unconfigured_provider() {
        let providers = providers_with("http://unused", false);
        let err = providers.begin("google", &cfg()).await.unwrap_err();
        assert!(matches!(err, OAuthLoginError::NotConfigured(_)));
    }

    #[tokio::test]
    async fn begin_rejects_unknown_provider() {
        let providers = providers_with("http://unused", true);
        let err = providers.begin("kick", &cfg()).await.unwrap_err();
        assert!(matches!(err, OAuthLoginError::UnknownProvider(_)));
    }

    async fn body_text(resp: Response) -> String {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        String::from_utf8_lossy(&bytes).to_string()
    }

    #[tokio::test]
    async fn callback_returns_json_by_default_on_missing_code() {
        let app = skeleton_state(UserStore::in_memory(), LoginProviders::empty());
        // integration.frontend_base defaults to "" — no redirect.

        let resp = callback(
            Path("google".to_string()),
            Query(CallbackQuery { code: None, state: None, error: None }),
            State(app),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert!(resp.headers().get("location").is_none());
        assert!(body_text(resp).await.contains("missing code or state"));
    }

    #[tokio::test]
    async fn callback_redirects_to_frontend_on_error_when_configured() {
        let app = skeleton_state(UserStore::in_memory(), LoginProviders::empty());
        app.service_config.set("integration.frontend_base", "https://panel.example.com");

        let resp = callback(
            Path("google".to_string()),
            Query(CallbackQuery { code: None, state: None, error: Some("access_denied".to_string()) }),
            State(app),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        let location = resp.headers().get("location").and_then(|v| v.to_str().ok()).unwrap_or_default();
        assert!(location.starts_with("https://panel.example.com/login/oauth-complete?"));
        assert!(location.contains("provider=google"));
        assert!(location.contains("error="));
    }

    #[tokio::test]
    async fn callback_redirects_with_token_in_the_fragment_on_success() {
        let base = spawn_google_mock("streamer@example.com").await;
        let providers = providers_with(&base, true);
        let state_param = begin_and_capture_state(&providers).await;

        let app = skeleton_state(UserStore::in_memory(), providers);
        app.service_config.set("integration.frontend_base", "https://panel.example.com");

        let resp = callback(
            Path("google".to_string()),
            Query(CallbackQuery {
                code: Some("code-1".to_string()),
                state: Some(state_param),
                error: None,
            }),
            State(app),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        let location = resp.headers().get("location").and_then(|v| v.to_str().ok()).unwrap_or_default();
        // Token rides the fragment (never sent to the server, unlike a query param).
        assert!(location.starts_with("https://panel.example.com/login/oauth-complete#token="));
    }
}
