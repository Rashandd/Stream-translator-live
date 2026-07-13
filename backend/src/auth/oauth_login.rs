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
//!
//! **Hesap linkleme**: girişli bir kullanıcı ayarlar ekranından ikinci bir
//! sağlayıcı bağlayabilir (örn. Google ile kayıt olan biri Twitch'i de
//! bağlar). Bu, login akışının aynısını reuse eder — tek fark `state`'e
//! `NO_USER_YET` yerine gerçek `user_id`'nin gömülmesi (bkz.
//! `integrations::begin_inner`, aynı desen): `GET
//! /api/auth/oauth/{provider}/link/start` (auth zorunlu, JSON authorize_url
//! döner — `integrations::authorize` ile aynı desen, çünkü tarayıcı
//! navigasyonu Authorization header taşıyamaz) → aynı
//! `/api/auth/oauth/{provider}/callback` → `complete()` `state`'ten gerçek
//! bir `user_id` çıktığını görünce yeni hesap açmak/login olmak yerine
//! [`crate::users::UserStore::link_identity`] çağırır.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::auth::authz::AuthUser;
use crate::config::ServiceConfig;
use crate::integrations::oauth::{exchange_code, OAuthProvider};
use crate::integrations::{urlencoding_light, CallbackQuery};
use crate::logging::LogLevel;
use crate::secrets::{names, SecretRef, SecretStore};
use crate::state::AppState;
use crate::users::{issue_user_token, LoginResponse, OAuthIdentityInfo, UserError, UserRecord};
#[cfg(test)]
use crate::users::UserStore;

/// OAuth başlatma niyeti — `state`'e gömülür ve callback davranışını belirler
/// (A-03). Login butonu bilinmeyen kimlik için sessizce hesap AÇMAZ; yalnız
/// signup butonu hesap oluşturabilir. Link, girişli kullanıcının ek sağlayıcı
/// bağlaması içindir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OAuthIntent {
    Login,
    Signup,
    Link,
}

impl OAuthIntent {
    /// Bilinmeyen/eksik değer en kısıtlı niyete (Login — hesap açmaz) düşer.
    fn parse(raw: Option<&str>) -> OAuthIntent {
        match raw {
            Some("signup") => OAuthIntent::Signup,
            Some("link") => OAuthIntent::Link,
            _ => OAuthIntent::Login,
        }
    }
}

/// authorize adımında üretilip callback'te tüketilen tek kullanımlık CSRF
/// state kaydı. Paylaşılan [`crate::integrations::oauth::StateStore`]'dan
/// AYRI tutulur: o yalnız `(user, provider, redirect_uri)` taşır; browser auth
/// akışı ayrıca niyet, locale ve (open-redirect korumalı) dönüş yolunu da
/// server-side taşımak zorundadır (A-03) — bunları paylaşılan store'a eklemek
/// yayın-hedefi bağlama akışını gereksiz yere kirletirdi (plan §5 bu ayrımı
/// açıkça öngörüyor).
struct LoginState {
    provider: String,
    intent: OAuthIntent,
    /// Kullanıcıya döneceğimiz frontend locale segmenti (örn. `en`, `tr`).
    /// `sanitize_locale` ile temizlenir; path injection'a izin vermez.
    locale: String,
    /// Yalnız aynı origin'de, `/` ile başlayan relative route (A-03). Mutlak
    /// URL / `//host` reddedilip `None`'a düşer.
    return_to: Option<String>,
    redirect_uri: String,
    /// Link akışında bağlanacak mevcut kullanıcı; login/signup'ta `None`.
    existing_user_id: Option<String>,
    created: Instant,
}

/// Browser auth OAuth `state`'lerini tutar (tek kullanımlık, süreli).
struct LoginStateStore {
    inner: Mutex<HashMap<String, LoginState>>,
    ttl: Duration,
}

impl LoginStateStore {
    fn new(ttl: Duration) -> Self {
        LoginStateStore {
            inner: Mutex::new(HashMap::new()),
            ttl,
        }
    }

    fn issue(
        &self,
        provider: &str,
        intent: OAuthIntent,
        locale: &str,
        return_to: Option<String>,
        redirect_uri: &str,
        existing_user_id: Option<String>,
    ) -> String {
        let state = uuid::Uuid::new_v4().simple().to_string();
        if let Ok(mut map) = self.inner.lock() {
            map.insert(
                state.clone(),
                LoginState {
                    provider: provider.to_string(),
                    intent,
                    locale: locale.to_string(),
                    return_to,
                    redirect_uri: redirect_uri.to_string(),
                    existing_user_id,
                    created: Instant::now(),
                },
            );
        }
        state
    }

    /// `state`'i tüketir (tek kullanımlık). Bilinmiyor veya süresi geçmişse `None`.
    fn consume(&self, state: &str) -> Option<LoginState> {
        let mut map = self.inner.lock().ok()?;
        let entry = map.remove(state)?;
        if entry.created.elapsed() > self.ttl {
            return None;
        }
        Some(entry)
    }
}

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
    states: LoginStateStore,
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
            states: LoginStateStore::new(Duration::from_secs(600)),
            http: reqwest::Client::new(),
        }
    }

    /// Sağlayıcısı olmayan boş registry — testler ve entegrasyonu
    /// yapılandırılmamış ortamlar için (bkz. `Integrations::empty`, aynı desen).
    pub fn empty() -> Self {
        LoginProviders {
            providers: HashMap::new(),
            states: LoginStateStore::new(Duration::from_secs(600)),
            http: reqwest::Client::new(),
        }
    }

    #[cfg(test)]
    fn from_parts(providers: HashMap<String, LoginProviderCreds>) -> Self {
        LoginProviders {
            providers,
            states: LoginStateStore::new(Duration::from_secs(600)),
            http: reqwest::Client::new(),
        }
    }

    /// authorize adımı: yayıncının yönlendirileceği sağlayıcı URL'sini kurar.
    /// `intent` (login/signup) ile döneceğimiz `locale`/`return_to` `state`'e
    /// gömülür; callback bunları locale-safe ve open-redirect korumalı bir
    /// yönlendirme kurmak için kullanır.
    async fn begin(
        &self,
        provider_name: &str,
        intent: OAuthIntent,
        locale: &str,
        return_to: Option<String>,
        cfg: &ServiceConfig,
    ) -> Result<String, OAuthLoginError> {
        self.authorize_url_for(provider_name, intent, locale, return_to, None, cfg)
            .await
    }

    /// Girişli bir kullanıcının hesabına ek bir sağlayıcı bağlamak için
    /// authorize adımı — `begin` ile aynı, ama `state`'e mevcut `user_id`
    /// gömülür (intent = Link); `complete()` bunu görünce login/signup yerine
    /// linkleme yapar.
    async fn begin_link(
        &self,
        user_id: &str,
        provider_name: &str,
        locale: &str,
        cfg: &ServiceConfig,
    ) -> Result<String, OAuthLoginError> {
        self.authorize_url_for(
            provider_name,
            OAuthIntent::Link,
            locale,
            None,
            Some(user_id.to_string()),
            cfg,
        )
        .await
    }

    async fn authorize_url_for(
        &self,
        provider_name: &str,
        intent: OAuthIntent,
        locale: &str,
        return_to: Option<String>,
        existing_user_id: Option<String>,
        cfg: &ServiceConfig,
    ) -> Result<String, OAuthLoginError> {
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
        let state = self.states.issue(
            provider_name,
            intent,
            locale,
            return_to,
            &redirect_uri,
            existing_user_id,
        );
        Ok(creds.provider.authorize_redirect(&client_id, &redirect_uri, &state))
    }

    /// callback adımı: `code`'u token'a çevirir, sağlayıcıdan kimlik çeker.
    /// `state`'ten çıkan `user_id` gerçekse (bkz. `begin_link`) bunu mevcut
    /// hesaba bağlar; `NO_USER_YET` ise normal login/signup akışı işler:
    /// var olan hesabı bulur ya da (e-posta çakışmıyorsa) yenisini açar, ve
    /// e-posta/şifre login'iyle aynı şekilde bir JWT döner.
    async fn complete(
        &self,
        app: &AppState,
        provider_name: &str,
        code: &str,
        state: &str,
    ) -> Result<OAuthOutcome, OAuthLoginError> {
        let st = self.states.consume(state).ok_or(OAuthLoginError::InvalidState)?;
        if st.provider != provider_name {
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

        let token = exchange_code(&self.http, &creds.provider, &client_id, &client_secret, code, &st.redirect_uri)
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

        if st.intent == OAuthIntent::Link {
            let user_id = st
                .existing_user_id
                .as_deref()
                .ok_or(OAuthLoginError::InvalidState)?;
            app.users.link_identity(user_id, provider_name, &identity.external_id)?;
            app.logger.log(
                LogLevel::Info,
                "auth",
                &format!("oauth account linked: {provider_name}"),
                Some(user_id),
                None,
            );
            return Ok(OAuthOutcome::Linked {
                provider: provider_name.to_string(),
                locale: st.locale,
            });
        }

        if let Some(existing) = app.users.find_by_oauth(provider_name, &identity.external_id) {
            let token = issue_token_string(app, &existing)?;
            return Ok(OAuthOutcome::LoggedIn {
                token,
                locale: st.locale,
                return_to: st.return_to,
            });
        }
        if app.users.get(&identity.email).is_some() {
            return Err(OAuthLoginError::EmailExists);
        }

        // Bilinmeyen kimlik + e-posta çakışması yok. Login butonu buradan
        // sessizce hesap AÇMAZ (A-03) — yalnız signup niyeti oluşturur.
        if st.intent == OAuthIntent::Login {
            return Err(OAuthLoginError::AccountNotFound);
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
        let token = issue_token_string(app, &user)?;
        Ok(OAuthOutcome::LoggedIn {
            token,
            locale: st.locale,
            return_to: st.return_to,
        })
    }
}

/// `complete()` sonucu — login/signup mı yoksa mevcut hesaba bağlama mı.
/// `locale`/`return_to` `state`'ten taşınır ki callback locale-safe ve
/// open-redirect korumalı bir yönlendirme kursun (A-02).
#[derive(Debug)]
enum OAuthOutcome {
    LoggedIn {
        token: String,
        locale: String,
        return_to: Option<String>,
    },
    Linked {
        provider: String,
        locale: String,
    },
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
    #[error("no account is linked to this identity — sign up first")]
    AccountNotFound,
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
        OAuthLoginError::AccountNotFound => StatusCode::NOT_FOUND,
        OAuthLoginError::Account(UserError::OAuthIdentityTaken | UserError::AlreadyLinked) => StatusCode::CONFLICT,
        OAuthLoginError::Account(UserError::LastLoginMethod) => StatusCode::CONFLICT,
        OAuthLoginError::Account(UserError::NotFound) => StatusCode::NOT_FOUND,
        OAuthLoginError::Account(_) | OAuthLoginError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// A-02 allowlist: callback URL'sine (veya JSON hata gövdesine) yalnız bu sabit
/// kodlardan biri yazılır — ham provider/backend hata metni asla kullanıcıya,
/// URL'ye veya erişim loglarına sızmaz. Kullanıcıya gösterilen metin frontend
/// dictionary'sinden gelir.
fn oauth_error_code(err: &OAuthLoginError) -> &'static str {
    match err {
        OAuthLoginError::AccountNotFound => "account_not_found",
        OAuthLoginError::EmailExists
        | OAuthLoginError::Account(UserError::OAuthIdentityTaken)
        | OAuthLoginError::Account(UserError::AlreadyLinked) => "email_exists",
        OAuthLoginError::InvalidState | OAuthLoginError::UnknownProvider(_) => "invalid_state",
        OAuthLoginError::NotConfigured(_) => "provider_unavailable",
        _ => "oauth_failed",
    }
}

/// `sanitize_locale`: yalnız `xx` veya `xx-yy` biçimindeki küçük-harf locale
/// segmentini kabul eder; aksi halde `default`'a düşer. `/`, `.`, `..`, boşluk
/// gibi path-injection'a yol açacak her şeyi eler (locale doğrudan redirect
/// path'ine gömülüyor).
fn sanitize_locale(raw: Option<&str>, default: &str) -> String {
    let value = raw.map(str::trim).unwrap_or("");
    let mut parts = value.split('-');
    let primary = parts.next().unwrap_or("");
    let region = parts.next();
    let extra = parts.next();
    let two_lower = |s: &str| s.len() == 2 && s.chars().all(|c| c.is_ascii_lowercase());
    let ok = extra.is_none()
        && two_lower(primary)
        && region.map(|r| r.len() == 2 && r.chars().all(|c| c.is_ascii_alphabetic())).unwrap_or(true);
    if ok {
        value.to_string()
    } else {
        default.to_string()
    }
}

/// `return_to` yalnız aynı origin'de, `/` ile başlayan relative route olabilir
/// (A-03). Mutlak URL, `//host`, backslash, scheme veya kontrol/boşluk karakteri
/// içeren her şey reddedilip `None`'a düşer — open redirect imkânsız olur.
fn sanitize_return_to(raw: Option<&str>) -> Option<String> {
    let value = raw?.trim();
    if !value.starts_with('/') || value.starts_with("//") {
        return None;
    }
    if value.contains('\\') || value.contains("://") {
        return None;
    }
    if value.chars().any(|c| c.is_control() || c == ' ') {
        return None;
    }
    Some(value.to_string())
}

/// `integration.frontend_base` yalnız origin (scheme://host[:port]) olarak
/// kullanılır; yanlışlıkla girilmiş path/trailing slash kırpılır ki locale
/// segmenti tek bir yardımcıda (`frontend_url`) güvenle eklensin. Geçersiz veya
/// http/https dışı bir değer `None` döner (redirect yerine JSON'a düşülür).
fn frontend_origin(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let url = reqwest::Url::parse(raw).ok()?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return None;
    }
    let host = url.host_str()?;
    let mut origin = format!("{}://{host}", url.scheme());
    if let Some(port) = url.port() {
        origin.push_str(&format!(":{port}"));
    }
    Some(origin)
}

/// Locale segmentini tek yerde ekleyen yardımcı — path her zaman `/` ile
/// başlar (örn. `/login/oauth-complete`). Query/fragment boşsa eklenmez.
fn frontend_url(origin: &str, locale: &str, path: &str, query: &str, fragment: &str) -> String {
    let mut url = format!("{origin}/{locale}{path}");
    if !query.is_empty() {
        url.push('?');
        url.push_str(query);
    }
    if !fragment.is_empty() {
        url.push('#');
        url.push_str(fragment);
    }
    url
}

fn default_locale(cfg: &ServiceConfig) -> String {
    cfg.get("auth.default_locale").unwrap_or_else(|| "en".to_string())
}

/// `GET /api/auth/oauth/{provider}/start` başlatma parametreleri. Tarayıcı
/// navigasyonu (`<a href>`) olduğu için query string ile taşınır.
#[derive(Debug, Default, Deserialize)]
pub struct StartQuery {
    /// `login` | `signup` — bilinmeyen/eksik değer en kısıtlı niyete (login,
    /// hesap açmaz) düşer. `link` bu public uçtan kabul edilmez.
    #[serde(default)]
    pub intent: Option<String>,
    #[serde(default)]
    pub locale: Option<String>,
    #[serde(default)]
    pub return_to: Option<String>,
}

/// `GET /api/auth/oauth/{provider}/start` (auth yok) — sağlayıcıya 302.
pub async fn start(
    Path(provider): Path<String>,
    Query(q): Query<StartQuery>,
    State(app): State<AppState>,
) -> Result<Response, (StatusCode, String)> {
    let intent = match OAuthIntent::parse(q.intent.as_deref()) {
        // Link yalnız authenticated `link/start` üzerinden başlar; public start
        // onu login'e düşürür (aksi halde user_id'siz bir link state'i olurdu).
        OAuthIntent::Link => OAuthIntent::Login,
        other => other,
    };
    let locale = sanitize_locale(q.locale.as_deref(), &default_locale(&app.service_config));
    let return_to = sanitize_return_to(q.return_to.as_deref());
    let url = app
        .oauth_login
        .begin(&provider, intent, &locale, return_to, &app.service_config)
        .await
        .map_err(|e| (oauth_login_status(&e), e.to_string()))?;
    Ok(Redirect::to(&url).into_response())
}

/// `begin`/`link_start` ile aynı şekil — `integrations::AuthorizeResponse`
/// ile aynı desen: tarayıcı navigasyonu (`window.location.href = ...`)
/// Authorization header taşıyamadığı için önce authenticated bir `fetch` ile
/// URL alınır, sonra oraya gidilir.
#[derive(Debug, Serialize)]
pub struct LinkStartResponse {
    pub authorize_url: String,
}

/// `GET /api/auth/oauth/{provider}/link/start` (auth zorunlu) — girişli
/// kullanıcının hesabına ek bir sağlayıcı bağlamak için authorize URL'sini
/// JSON olarak döner.
pub async fn link_start(
    user: AuthUser,
    Path(provider): Path<String>,
    Query(q): Query<StartQuery>,
    State(app): State<AppState>,
) -> Result<Json<LinkStartResponse>, (StatusCode, String)> {
    let locale = sanitize_locale(q.locale.as_deref(), &default_locale(&app.service_config));
    let url = app
        .oauth_login
        .begin_link(&user.claims.sub, &provider, &locale, &app.service_config)
        .await
        .map_err(|e| (oauth_login_status(&e), e.to_string()))?;
    Ok(Json(LinkStartResponse { authorize_url: url }))
}

/// `GET /api/auth/oauth/identities` yanıtı.
#[derive(Debug, Serialize)]
pub struct IdentitiesResponse {
    /// Şifre de bir giriş yöntemidir — frontend son yöntemi unlink etmeden
    /// önce bunu göz önünde bulundurur.
    pub has_password: bool,
    pub identities: Vec<OAuthIdentityInfo>,
}

/// `GET /api/auth/oauth/identities` (auth zorunlu) — bağlı giriş
/// sağlayıcılarının listesi (ayarlar ekranı "Connected accounts").
pub async fn list_identities(
    user: AuthUser,
    State(app): State<AppState>,
) -> Result<Json<IdentitiesResponse>, (StatusCode, String)> {
    let record = app
        .users
        .get_by_id(&user.claims.sub)
        .ok_or((StatusCode::NOT_FOUND, "user not found".to_string()))?;
    Ok(Json(IdentitiesResponse {
        has_password: record.password_hash.is_some(),
        identities: app.users.oauth_identities(&user.claims.sub),
    }))
}

/// `DELETE /api/auth/oauth/{provider}` (auth zorunlu) — bir sağlayıcının
/// bağlantısını kaldırır. Hesabın *son* giriş yöntemiyse 409 döner.
pub async fn unlink(
    user: AuthUser,
    Path(provider): Path<String>,
    State(app): State<AppState>,
) -> Result<StatusCode, (StatusCode, String)> {
    app.users
        .unlink_identity(&user.claims.sub, &provider)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|err| {
            let status = match err {
                UserError::LastLoginMethod | UserError::OAuthIdentityTaken | UserError::AlreadyLinked => {
                    StatusCode::CONFLICT
                }
                UserError::NotFound => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, err.to_string())
        })
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
    let origin = frontend_origin(&app.service_config.get("integration.frontend_base").unwrap_or_default());
    // Hata yolunda `state` henüz tüketilmediği/geçersiz olduğu için locale
    // bilinmez → yapılandırılmış varsayılan locale kullanılır.
    let default_locale = default_locale(&app.service_config);

    // Sağlayıcının kendi hata parametresi (örn. kullanıcı consent'i reddetti):
    // ham metin yalnız server loguna yazılır, URL'ye sabit kod gider (A-02).
    if let Some(err) = q.error {
        app.logger.log(
            LogLevel::Warn,
            "auth",
            &format!("oauth provider error for {provider}: {err}"),
            None,
            None,
        );
        return error_response(origin.as_deref(), &default_locale, "oauth_failed", StatusCode::BAD_GATEWAY);
    }
    let (Some(code), Some(state)) = (q.code, q.state) else {
        return error_response(origin.as_deref(), &default_locale, "invalid_state", StatusCode::BAD_REQUEST);
    };

    match app.oauth_login.complete(&app, &provider, &code, &state).await {
        Ok(OAuthOutcome::LoggedIn { token, locale, return_to }) => {
            app.logger.log(LogLevel::Info, "auth", &format!("oauth login completed via {provider}"), None, None);
            match origin {
                None => axum::Json(serde_json::json!({ "token": token })).into_response(),
                Some(origin) => {
                    // Faz 1: token hâlâ fragment'ta taşınır (cookie migration Faz 2);
                    // fark yalnız locale-safe path ve isteğe bağlı return_to.
                    let query = return_to
                        .map(|r| format!("return_to={}", urlencoding_light(&r)))
                        .unwrap_or_default();
                    let url = frontend_url(&origin, &locale, "/login/oauth-complete", &query, &format!("token={token}"));
                    Redirect::to(&url).into_response()
                }
            }
        }
        Ok(OAuthOutcome::Linked { provider, locale }) => match origin {
            None => axum::Json(serde_json::json!({ "linked": provider })).into_response(),
            Some(origin) => {
                let url = frontend_url(&origin, &locale, "/dashboard/account", &format!("linked={provider}"), "");
                Redirect::to(&url).into_response()
            }
        },
        Err(err) => {
            app.logger.log(
                LogLevel::Warn,
                "auth",
                &format!("oauth login failed for {provider}: {err}"),
                None,
                None,
            );
            let code = oauth_error_code(&err);
            error_response(origin.as_deref(), &default_locale, code, oauth_login_status(&err))
        }
    }
}

/// Hata yanıtı: origin varsa locale-safe
/// `/{locale}/login/oauth-complete?error=<code>` redirect'i, yoksa
/// `{ "error": <code> }` JSON gövdesi. Her iki yolda da yalnız A-02 allowlist
/// kodu kullanılır — ham hata metni asla URL'ye/gövdeye girmez.
fn error_response(origin: Option<&str>, locale: &str, code: &str, status: StatusCode) -> Response {
    match origin {
        Some(origin) => {
            let url = frontend_url(origin, locale, "/login/oauth-complete", &format!("error={code}"), "");
            Redirect::to(&url).into_response()
        }
        None => (status, axum::Json(serde_json::json!({ "error": code }))).into_response(),
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
    async fn begin_and_capture_state(providers: &LoginProviders, intent: OAuthIntent) -> String {
        let url = providers.begin("google", intent, "en", None, &cfg()).await.unwrap();
        state_param_of(&url)
    }

    fn state_param_of(url: &str) -> String {
        reqwest::Url::parse(url)
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
        // Signup butonu (yeni hesap açabilen tek niyet — A-03).
        let state_param = begin_and_capture_state(&providers, OAuthIntent::Signup).await;

        let app = skeleton_state(UserStore::in_memory(), providers);
        let outcome = app
            .oauth_login
            .complete(&app, "google", "code-1", &state_param)
            .await
            .expect("first login creates an account");

        let OAuthOutcome::LoggedIn { token, .. } = outcome else { panic!("expected a login outcome") };
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
        let state_param = begin_and_capture_state(&app.oauth_login, OAuthIntent::Login).await;

        let outcome = app
            .oauth_login
            .complete(&app, "google", "code-1", &state_param)
            .await
            .expect("existing identity logs in");
        let OAuthOutcome::LoggedIn { token, .. } = outcome else { panic!("expected a login outcome") };
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
        let state_param = begin_and_capture_state(&app.oauth_login, OAuthIntent::Signup).await;

        let err = app
            .oauth_login
            .complete(&app, "google", "code-1", &state_param)
            .await
            .unwrap_err();
        assert!(matches!(err, OAuthLoginError::EmailExists));

        // No new (OAuth-only) account was created for this identity.
        assert!(app.users.find_by_oauth("google", "g-123").is_none());
    }

    /// `begin_link()` üzerinden gerçek bir `state` üretip callback'in
    /// bekleyeceği kod/state ikilisini simüle eder (linkleme akışı).
    async fn begin_link_and_capture_state(providers: &LoginProviders, user_id: &str) -> String {
        let url = providers.begin_link(user_id, "google", "en", &cfg()).await.unwrap();
        state_param_of(&url)
    }

    #[tokio::test]
    async fn complete_links_a_second_provider_to_the_currently_logged_in_account() {
        let base = spawn_google_mock("streamer@example.com").await;
        let providers = providers_with(&base, true);

        let users = UserStore::in_memory();
        let existing = users.signup("existing@example.com", "hunter2-strong", "free").unwrap();
        let app = skeleton_state(users, providers);
        let state_param = begin_link_and_capture_state(&app.oauth_login, &existing.id).await;

        let outcome = app
            .oauth_login
            .complete(&app, "google", "code-1", &state_param)
            .await
            .expect("link succeeds");
        assert!(matches!(outcome, OAuthOutcome::Linked { provider, .. } if provider == "google"));

        let linked = app.users.find_by_oauth("google", "g-123").expect("identity now resolves");
        assert_eq!(linked.id, existing.id);
    }

    #[tokio::test]
    async fn complete_rejects_linking_an_identity_already_owned_by_another_account() {
        let base = spawn_google_mock("streamer@example.com").await;
        let providers = providers_with(&base, true);

        let users = UserStore::in_memory();
        users.create_oauth_user("google", "g-123", "owner@example.com", "free").unwrap();
        let other = users.signup("other@example.com", "hunter2-strong", "free").unwrap();
        let app = skeleton_state(users, providers);
        let state_param = begin_link_and_capture_state(&app.oauth_login, &other.id).await;

        let err = app
            .oauth_login
            .complete(&app, "google", "code-1", &state_param)
            .await
            .unwrap_err();
        assert!(matches!(err, OAuthLoginError::Account(UserError::OAuthIdentityTaken)));
    }

    fn authed(user_id: &str) -> crate::auth::authz::AuthUser {
        crate::auth::authz::AuthUser {
            claims: crate::auth::jwt::Claims {
                sub: user_id.to_string(),
                role: "broadcaster".to_string(),
                plan: "free".to_string(),
                exp: chrono::Utc::now().timestamp() + 3600,
            },
            role: crate::auth::authz::Role::Broadcaster,
        }
    }

    #[tokio::test]
    async fn link_start_returns_an_authorize_url_for_the_logged_in_user() {
        let providers = providers_with("http://unused", true);
        let users = UserStore::in_memory();
        let user = users.signup("streamer@example.com", "hunter2-strong", "free").unwrap();
        let app = skeleton_state(users, providers);

        let Json(resp) = link_start(
            authed(&user.id),
            Path("google".to_string()),
            Query(StartQuery::default()),
            State(app),
        )
        .await
        .expect("link start succeeds");
        assert!(resp.authorize_url.starts_with("http://unused/authorize?"));
        assert!(resp.authorize_url.contains("state="));
    }

    #[tokio::test]
    async fn list_identities_reports_password_and_linked_providers() {
        let users = UserStore::in_memory();
        let user = users.signup("streamer@example.com", "hunter2-strong", "free").unwrap();
        users.link_identity(&user.id, "twitch", "t-1").unwrap();
        let app = skeleton_state(users, LoginProviders::empty());

        let Json(resp) = list_identities(authed(&user.id), State(app)).await.expect("list succeeds");
        assert!(resp.has_password);
        assert_eq!(resp.identities.len(), 1);
        assert_eq!(resp.identities[0].provider, "twitch");
    }

    #[tokio::test]
    async fn unlink_rejects_removing_the_only_login_method() {
        let users = UserStore::in_memory();
        let user = users.create_oauth_user("google", "g-1", "oauth-only@example.com", "free").unwrap();
        let app = skeleton_state(users, LoginProviders::empty());

        let err = unlink(authed(&user.id), Path("google".to_string()), State(app))
            .await
            .unwrap_err();
        assert_eq!(err.0, StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn unlink_removes_a_linked_provider_when_another_method_remains() {
        let users = UserStore::in_memory();
        let user = users.signup("streamer@example.com", "hunter2-strong", "free").unwrap();
        users.link_identity(&user.id, "twitch", "t-1").unwrap();
        let app = skeleton_state(users, LoginProviders::empty());

        let status = unlink(authed(&user.id), Path("twitch".to_string()), State(app.clone()))
            .await
            .expect("unlink succeeds");
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert!(app.users.find_by_oauth("twitch", "t-1").is_none());
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
        let url = providers
            .begin("google", OAuthIntent::Login, "en", None, &cfg())
            .await
            .unwrap();
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
        let err = providers
            .begin("google", OAuthIntent::Login, "en", None, &cfg())
            .await
            .unwrap_err();
        assert!(matches!(err, OAuthLoginError::NotConfigured(_)));
    }

    #[tokio::test]
    async fn begin_rejects_unknown_provider() {
        let providers = providers_with("http://unused", true);
        let err = providers
            .begin("kick", OAuthIntent::Login, "en", None, &cfg())
            .await
            .unwrap_err();
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
        // JSON fallback carries only the opaque A-02 code, never raw text.
        let body = body_text(resp).await;
        assert!(body.contains("\"error\":\"invalid_state\""), "body was {body}");
        assert!(!body.contains("missing code or state"));
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
        // Locale-safe path (no more locale-less 404) + opaque code, no raw
        // provider text like "access_denied".
        assert!(location.starts_with("https://panel.example.com/en/login/oauth-complete?"));
        assert!(location.contains("error=oauth_failed"));
        assert!(!location.contains("access_denied"));
        assert!(!location.contains("provider error"));
    }

    #[tokio::test]
    async fn callback_redirects_with_token_in_the_fragment_on_success() {
        let base = spawn_google_mock("streamer@example.com").await;
        let providers = providers_with(&base, true);
        // Signup intent (capture a locale so the callback can build a
        // locale-safe path) + a valid same-origin return_to.
        let url = providers
            .begin("google", OAuthIntent::Signup, "tr", Some("/dashboard".to_string()), &cfg())
            .await
            .unwrap();
        let state_param = state_param_of(&url);

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
        // Locale-safe path; token still rides the fragment in Faz 1 (never sent
        // to the server, unlike a query param); return_to is forwarded as query.
        assert!(location.starts_with("https://panel.example.com/tr/login/oauth-complete?"));
        assert!(location.contains("return_to=/dashboard"));
        assert!(location.contains("#token="));
    }

    // --- Faz 1 davranış testleri (locale-safe + opaque callback) ---

    #[test]
    fn sanitize_locale_accepts_simple_tags_and_rejects_path_injection() {
        assert_eq!(sanitize_locale(Some("en"), "en"), "en");
        assert_eq!(sanitize_locale(Some("tr"), "en"), "tr");
        assert_eq!(sanitize_locale(Some("pt-BR"), "en"), "pt-BR");
        // Path injection / garbage falls back to the default.
        assert_eq!(sanitize_locale(Some("../admin"), "en"), "en");
        assert_eq!(sanitize_locale(Some("en/../.."), "en"), "en");
        assert_eq!(sanitize_locale(Some("EN"), "en"), "en");
        assert_eq!(sanitize_locale(None, "en"), "en");
        assert_eq!(sanitize_locale(Some(""), "en"), "en");
    }

    #[test]
    fn sanitize_return_to_allows_only_same_origin_relative_routes() {
        assert_eq!(sanitize_return_to(Some("/dashboard")), Some("/dashboard".to_string()));
        assert_eq!(
            sanitize_return_to(Some("/en/dashboard/billing")),
            Some("/en/dashboard/billing".to_string())
        );
        // Open-redirect vectors are rejected.
        assert_eq!(sanitize_return_to(Some("//evil.com")), None);
        assert_eq!(sanitize_return_to(Some("https://evil.com")), None);
        assert_eq!(sanitize_return_to(Some("/\\evil.com")), None);
        assert_eq!(sanitize_return_to(Some("dashboard")), None);
        assert_eq!(sanitize_return_to(Some("/foo bar")), None);
        assert_eq!(sanitize_return_to(None), None);
    }

    #[test]
    fn frontend_origin_strips_paths_and_rejects_non_http() {
        assert_eq!(
            frontend_origin("https://panel.example.com"),
            Some("https://panel.example.com".to_string())
        );
        // A stray path/trailing slash is trimmed to the bare origin.
        assert_eq!(
            frontend_origin("https://panel.example.com/login/"),
            Some("https://panel.example.com".to_string())
        );
        assert_eq!(
            frontend_origin("http://localhost:3000"),
            Some("http://localhost:3000".to_string())
        );
        assert_eq!(frontend_origin(""), None);
        assert_eq!(frontend_origin("javascript:alert(1)"), None);
    }

    #[test]
    fn oauth_error_code_maps_to_the_a02_allowlist_only() {
        assert_eq!(oauth_error_code(&OAuthLoginError::AccountNotFound), "account_not_found");
        assert_eq!(oauth_error_code(&OAuthLoginError::EmailExists), "email_exists");
        assert_eq!(oauth_error_code(&OAuthLoginError::InvalidState), "invalid_state");
        assert_eq!(
            oauth_error_code(&OAuthLoginError::NotConfigured("google".into())),
            "provider_unavailable"
        );
        assert_eq!(oauth_error_code(&OAuthLoginError::Exchange("boom".into())), "oauth_failed");
        assert_eq!(oauth_error_code(&OAuthLoginError::Identity("boom".into())), "oauth_failed");
    }

    #[tokio::test]
    async fn login_intent_does_not_silently_create_an_account() {
        let base = spawn_google_mock("newcomer@example.com").await;
        let providers = providers_with(&base, true);
        // Login button (not signup) for an identity that does not exist yet.
        let state_param = begin_and_capture_state(&providers, OAuthIntent::Login).await;

        let app = skeleton_state(UserStore::in_memory(), providers);
        let err = app
            .oauth_login
            .complete(&app, "google", "code-1", &state_param)
            .await
            .unwrap_err();
        assert!(matches!(err, OAuthLoginError::AccountNotFound));
        // No account was created — the login button must route to signup instead.
        assert!(app.users.find_by_oauth("google", "g-123").is_none());
    }

    #[tokio::test]
    async fn callback_error_redirect_is_locale_safe_for_a_non_default_locale() {
        // An invalid/expired state cannot reveal a locale, so the error path
        // uses the configured default locale (here overridden to tr).
        let app = skeleton_state(UserStore::in_memory(), LoginProviders::empty());
        app.service_config.set("integration.frontend_base", "https://panel.example.com");
        app.service_config.set("auth.default_locale", "tr");

        let resp = callback(
            Path("google".to_string()),
            Query(CallbackQuery {
                code: Some("code".to_string()),
                state: Some("bogus-state".to_string()),
                error: None,
            }),
            State(app),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        let location = resp.headers().get("location").and_then(|v| v.to_str().ok()).unwrap_or_default();
        assert_eq!(location, "https://panel.example.com/tr/login/oauth-complete?error=invalid_state");
    }

    #[tokio::test]
    async fn callback_normalizes_a_misconfigured_frontend_base_with_a_path() {
        // Admin accidentally saved a path on the base; the callback must still
        // produce a single well-formed locale path, not a double segment.
        let app = skeleton_state(UserStore::in_memory(), LoginProviders::empty());
        app.service_config
            .set("integration.frontend_base", "https://panel.example.com/oops/");

        let resp = callback(
            Path("google".to_string()),
            Query(CallbackQuery { code: None, state: None, error: None }),
            State(app),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        let location = resp.headers().get("location").and_then(|v| v.to_str().ok()).unwrap_or_default();
        assert_eq!(location, "https://panel.example.com/en/login/oauth-complete?error=invalid_state");
    }
}
