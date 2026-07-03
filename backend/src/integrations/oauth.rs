//! OAuth2 authorization-code çekirdeği — sağlayıcıdan bağımsız.
//!
//! Sağlayıcı uç noktaları (authorize/token URL, scope'lar) secret değildir;
//! `client_id`/`client_secret` [`crate::secrets::SecretStore`]'dan gelir. Burada
//! sadece akış mantığı var: authorize URL kurma, CSRF `state` yönetimi ve
//! token değişimi. Persist (token'ı AES ile şifreleyip DB'ye yazma) çağıran
//! katmanın işi (follow-up).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Bir OAuth2 sağlayıcısının (örn. Streamlabs) uç noktaları.
#[derive(Debug, Clone)]
pub struct OAuthProvider {
    pub name: String,
    pub authorize_url: String,
    pub token_url: String,
    pub scopes: Vec<String>,
    /// Sağlayıcıya özgü ek authorize parametreleri — örn. Google'ın offline
    /// refresh token'ı için `access_type=offline&prompt=consent`.
    pub extra_authorize_params: Vec<(String, String)>,
}

impl OAuthProvider {
    /// authorize adımı için tam yönlendirme URL'sini (query encode'lu) kurar.
    pub fn authorize_redirect(&self, client_id: &str, redirect_uri: &str, state: &str) -> String {
        let scope = self.scopes.join(" ");
        let mut params = vec![
            ("response_type", "code"),
            ("client_id", client_id),
            ("redirect_uri", redirect_uri),
            ("scope", scope.as_str()),
            ("state", state),
        ];
        for (key, value) in &self.extra_authorize_params {
            params.push((key.as_str(), value.as_str()));
        }
        // reqwest::Url ile güvenli percent-encoding.
        match reqwest::Url::parse_with_params(&self.authorize_url, params) {
            Ok(url) => url.to_string(),
            // authorize_url sabit ve geçerli; parse hatası pratikte olmaz.
            Err(_) => self.authorize_url.clone(),
        }
    }
}

/// Token uç noktasının döndürdüğü OAuth2 yanıtı.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_in: Option<u64>,
    #[serde(default)]
    pub token_type: Option<String>,
}

/// OAuth akış hataları.
#[derive(Debug, thiserror::Error)]
pub enum OAuthError {
    #[error("unknown provider: {0}")]
    UnknownProvider(String),
    #[error("provider not configured: {0}")]
    NotConfigured(String),
    #[error("invalid or expired state")]
    InvalidState,
    #[error("plan '{plan}' is not allowed for {provider}")]
    PlanNotAllowed { provider: String, plan: String },
    #[error("token exchange failed: {0}")]
    Exchange(String),
}

/// Virgülle ayrılmış plan listesinde `plan` var mı?
pub fn plan_allowed(plans_csv: &str, plan: &str) -> bool {
    plans_csv
        .split(',')
        .map(str::trim)
        .any(|allowed| allowed == plan)
}

/// authorization-code değişimi: `code` → access token.
pub async fn exchange_code(
    http: &reqwest::Client,
    provider: &OAuthProvider,
    client_id: &str,
    client_secret: &str,
    code: &str,
    redirect_uri: &str,
) -> Result<TokenResponse, OAuthError> {
    let params = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", client_id),
        ("client_secret", client_secret),
    ];

    let resp = http
        .post(&provider.token_url)
        .form(&params)
        .send()
        .await
        .map_err(|e| OAuthError::Exchange(e.to_string()))?;

    if !resp.status().is_success() {
        return Err(OAuthError::Exchange(format!(
            "token endpoint returned {}",
            resp.status()
        )));
    }

    resp.json::<TokenResponse>()
        .await
        .map_err(|e| OAuthError::Exchange(e.to_string()))
}

/// CSRF `state` → (kullanıcı, sağlayıcı, redirect_uri) eşlemesi (süreli).
struct StateEntry {
    user_id: String,
    provider: String,
    redirect_uri: String,
    created: Instant,
}

/// authorize adımında üretilen `state`'leri tutar, callback'te doğrular.
pub struct StateStore {
    inner: Mutex<HashMap<String, StateEntry>>,
    ttl: Duration,
}

impl StateStore {
    /// Verilen TTL ile boş bir store kurar.
    pub fn new(ttl: Duration) -> Self {
        StateStore {
            inner: Mutex::new(HashMap::new()),
            ttl,
        }
    }

    /// Yeni bir `state` üretir, saklar ve döndürür.
    pub fn issue(&self, user_id: &str, provider: &str, redirect_uri: &str) -> String {
        let state = uuid::Uuid::new_v4().simple().to_string();
        if let Ok(mut map) = self.inner.lock() {
            map.insert(
                state.clone(),
                StateEntry {
                    user_id: user_id.to_string(),
                    provider: provider.to_string(),
                    redirect_uri: redirect_uri.to_string(),
                    created: Instant::now(),
                },
            );
        }
        state
    }

    /// `state`'i tüketir (tek kullanımlık). Geçerliyse (user, provider,
    /// redirect_uri) döner; bilinmiyor veya süresi geçmişse `None`.
    pub fn consume(&self, state: &str) -> Option<(String, String, String)> {
        let mut map = self.inner.lock().ok()?;
        let entry = map.remove(state)?;
        if entry.created.elapsed() > self.ttl {
            return None;
        }
        Some((entry.user_id, entry.provider, entry.redirect_uri))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::Form;
    use axum::http::StatusCode;
    use axum::routing::post;
    use axum::{Json, Router};
    use std::collections::HashMap as Map;

    fn provider() -> OAuthProvider {
        OAuthProvider {
            name: "streamlabs".into(),
            authorize_url: "https://streamlabs.com/api/v2.0/authorize".into(),
            token_url: "https://streamlabs.com/api/v2.0/token".into(),
            scopes: vec!["socket.token".into(), "stream.default.read".into()],
            extra_authorize_params: Vec::new(),
        }
    }

    #[test]
    fn authorize_redirect_encodes_params() {
        let url = provider().authorize_redirect("cid-123", "https://app/cb", "st-abc");
        assert!(url.starts_with("https://streamlabs.com/api/v2.0/authorize?"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("client_id=cid-123"));
        assert!(url.contains("state=st-abc"));
        // redirect_uri ve boşluklu scope percent-encode edilmeli.
        assert!(url.contains("redirect_uri=https%3A%2F%2Fapp%2Fcb"));
        assert!(url.contains("socket.token+stream.default.read"));
    }

    #[test]
    fn authorize_redirect_appends_extra_params() {
        // Google offline refresh token deseni (YouTube Native CC — 1.10).
        let mut p = provider();
        p.extra_authorize_params = vec![
            ("access_type".into(), "offline".into()),
            ("prompt".into(), "consent".into()),
        ];
        let url = p.authorize_redirect("cid", "https://app/cb", "st");
        assert!(url.contains("access_type=offline"));
        assert!(url.contains("prompt=consent"));
    }

    #[test]
    fn plan_allowed_matches_csv() {
        assert!(plan_allowed("free,pro", "pro"));
        assert!(plan_allowed("free, pro", "free"));
        assert!(!plan_allowed("pro", "free"));
        assert!(!plan_allowed("", "free"));
    }

    #[test]
    fn state_roundtrip_and_single_use() {
        let store = StateStore::new(Duration::from_secs(60));
        let st = store.issue("user-1", "streamlabs", "https://app/cb");
        let got = store.consume(&st).expect("valid state");
        assert_eq!(got, ("user-1".into(), "streamlabs".into(), "https://app/cb".into()));
        // İkinci kullanım reddedilir (tek kullanımlık).
        assert!(store.consume(&st).is_none());
    }

    #[test]
    fn state_unknown_and_expired_rejected() {
        assert!(StateStore::new(Duration::from_secs(60)).consume("nope").is_none());
        let store = StateStore::new(Duration::from_millis(0));
        let st = store.issue("u", "streamlabs", "cb");
        std::thread::sleep(Duration::from_millis(5));
        assert!(store.consume(&st).is_none());
    }

    /// Token uç noktasını taklit eder: doğru client_secret ister, token döner.
    async fn spawn_token_mock() -> String {
        async fn token(Form(form): Form<Map<String, String>>) -> (StatusCode, Json<serde_json::Value>) {
            if form.get("client_secret").map(String::as_str) != Some("sec-1")
                || form.get("grant_type").map(String::as_str) != Some("authorization_code")
            {
                return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": "invalid_client" })));
            }
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "access_token": "at-xyz", "refresh_token": "rt-xyz",
                    "expires_in": 3600, "token_type": "Bearer"
                })),
            )
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
    async fn exchange_returns_token_on_success() {
        let token_url = spawn_token_mock().await;
        let mut p = provider();
        p.token_url = token_url;
        let http = reqwest::Client::new();
        let tok = exchange_code(&http, &p, "cid", "sec-1", "code-1", "https://app/cb")
            .await
            .unwrap();
        assert_eq!(tok.access_token, "at-xyz");
        assert_eq!(tok.expires_in, Some(3600));
        assert_eq!(tok.refresh_token.as_deref(), Some("rt-xyz"));
    }

    #[tokio::test]
    async fn exchange_errors_on_bad_secret() {
        let token_url = spawn_token_mock().await;
        let mut p = provider();
        p.token_url = token_url;
        let http = reqwest::Client::new();
        let err = exchange_code(&http, &p, "cid", "wrong", "code-1", "https://app/cb")
            .await
            .unwrap_err();
        assert!(matches!(err, OAuthError::Exchange(_)));
    }
}
