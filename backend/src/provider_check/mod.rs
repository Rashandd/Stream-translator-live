//! Sağlayıcı canlı bağlantı testi — `POST /admin/providers/{name}/test`.
//!
//! Admin panelden girilen key'in GERÇEKTEN çalıştığını doğrulamak için her
//! sağlayıcıya hafif, yan etkisiz bir doğrulama çağrısı yapar (Deepgram:
//! proje listesi, AssemblyAI: transcript listesi, DeepL: usage, Google:
//! dil listesi, Stripe: balance). Deploy sonrası "key'i girdim, çalışıyor
//! mu?" sorusunun cevabı bu endpoint'tir (roadmap 1.6 "her bağlantı için
//! test aracı" + doğrulama listesi STT/çeviri canlı testleri).
//!
//! Test URL'leri `ServiceConfig`'ten okunur (`providers.<ad>.test_url`) —
//! hard-coded değildir; testlerde mock sunucuya yönlendirilir. Key
//! [`SecretStore`]'dan çağrı anında çözülür; yoksa `NotConfigured`.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

use crate::auth::authz::AdminUser;
use crate::config::ServiceConfig;
use crate::logging::{LogLevel, Logger};
use crate::secrets::{names, ManagedSecrets, SecretStore};

/// Bir canlı testin sonucu. `ok=false` da geçerli bir sonuçtur (test koştu,
/// sağlayıcı reddetti) — HTTP 200 ile döner.
#[derive(Debug, Serialize)]
pub struct CheckOutcome {
    pub provider: String,
    pub ok: bool,
    /// Sağlayıcının döndürdüğü HTTP durum kodu (ağ hatasında `None`).
    pub status: Option<u16>,
    pub detail: String,
}

/// Test edilemeyen durumlar (sonuç bile üretilemez).
#[derive(Debug, thiserror::Error)]
pub enum CheckError {
    #[error("unknown provider: {0}")]
    UnknownProvider(String),
    #[error("{0} not configured — enter it under Admin > Secrets first")]
    NotConfigured(&'static str),
}

/// Bilinen sağlayıcı test tanımı.
struct CheckDef {
    provider: &'static str,
    secret_name: &'static str,
    /// `providers.<ad>.test_url` config anahtarı.
    url_key: &'static str,
    /// Config'te URL yoksa kullanılacak üretim varsayılanı. `{key}` yer
    /// tutucusu (Google) key ile değiştirilir.
    default_url: &'static str,
    auth: AuthStyle,
}

enum AuthStyle {
    /// `Authorization: Token <key>` (Deepgram)
    Token,
    /// `authorization: <key>` (AssemblyAI)
    Raw,
    /// `Authorization: DeepL-Auth-Key <key>` (DeepL)
    DeeplKey,
    /// Query param `?key=<key>` (Google) — URL şablonundaki `{key}`.
    QueryKey,
    /// HTTP Basic (kullanıcı=key, şifre boş) (Stripe)
    Basic,
    /// `xi-api-key: <key>` (ElevenLabs)
    XiApiKey,
}

fn defs() -> &'static [CheckDef] {
    &[
        CheckDef {
            provider: "deepgram",
            secret_name: names::DEEPGRAM_KEY,
            url_key: "providers.deepgram.test_url",
            default_url: "https://api.deepgram.com/v1/projects",
            auth: AuthStyle::Token,
        },
        CheckDef {
            provider: "assemblyai",
            secret_name: names::ASSEMBLYAI_KEY,
            url_key: "providers.assemblyai.test_url",
            default_url: "https://api.assemblyai.com/v2/transcript?limit=1",
            auth: AuthStyle::Raw,
        },
        CheckDef {
            provider: "deepl",
            secret_name: names::DEEPL_KEY,
            url_key: "providers.deepl.test_url",
            // Free key (`:fx`) çağrı anında free host'a çevrilir (aşağıda).
            default_url: "https://api.deepl.com/v2/usage",
            auth: AuthStyle::DeeplKey,
        },
        CheckDef {
            provider: "google",
            secret_name: names::GOOGLE_TRANSLATE_KEY,
            url_key: "providers.google.test_url",
            default_url: "https://translation.googleapis.com/language/translate/v2/languages?key={key}",
            auth: AuthStyle::QueryKey,
        },
        CheckDef {
            provider: "stripe",
            secret_name: names::STRIPE_SECRET_KEY,
            url_key: "providers.stripe.test_url",
            default_url: "https://api.stripe.com/v1/balance",
            auth: AuthStyle::Basic,
        },
        CheckDef {
            provider: "elevenlabs",
            secret_name: names::ELEVENLABS_KEY,
            url_key: "providers.elevenlabs.test_url",
            default_url: "https://api.elevenlabs.io/v1/user",
            auth: AuthStyle::XiApiKey,
        },
    ]
}

/// Panelin "hangi secret'ın test butonu var" eşlemesi için: secret adı →
/// sağlayıcı test adı.
pub fn provider_for_secret(secret_name: &str) -> Option<&'static str> {
    defs()
        .iter()
        .find(|d| d.secret_name == secret_name)
        .map(|d| d.provider)
}

/// Test edilebilir (API-key stili) sağlayıcıların adı + secret adı çiftleri.
/// Admin "Production Setup" sayfası provider profil listesini bundan kurar.
pub fn provider_defs() -> Vec<(&'static str, &'static str)> {
    defs().iter().map(|d| (d.provider, d.secret_name)).collect()
}

/// Bir canlı testin en son sonucu — "last tested at" görünürlüğü için.
#[derive(Debug, Clone, Serialize)]
pub struct ProviderTestRecord {
    pub ok: bool,
    pub detail: String,
    pub tested_at_ms: i64,
}

/// Sağlayıcı başına en son test sonucunu bellek içinde tutar. Test
/// stateless'tir (`run_check` her çağrıda ağa çıkar); bu store sadece
/// "en son ne zaman, ne sonuçla test edildi" sorusuna cevap vermek için var
/// (admin panel üzerinde görünürlük — Faz 1'de kalıcılık gerekmiyor,
/// restart'ta sıfırlanması zararsız).
#[derive(Default)]
pub struct ProviderTestStore(RwLock<HashMap<String, ProviderTestRecord>>);

impl ProviderTestStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Bir sağlayıcı için en son test sonucunu kaydeder.
    pub fn record(&self, provider: &str, ok: bool, detail: &str) {
        if let Ok(mut map) = self.0.write() {
            map.insert(
                provider.to_string(),
                ProviderTestRecord {
                    ok,
                    detail: detail.to_string(),
                    tested_at_ms: chrono::Utc::now().timestamp_millis(),
                },
            );
        }
    }

    /// Bir sağlayıcının en son test sonucunu okur (hiç test edilmediyse `None`).
    pub fn get(&self, provider: &str) -> Option<ProviderTestRecord> {
        self.0.read().ok().and_then(|map| map.get(provider).cloned())
    }
}

/// Testi koşar. Key store'dan çözülür; URL config'ten (yoksa varsayılan).
pub async fn run_check(
    provider: &str,
    secrets: &ManagedSecrets,
    cfg: &ServiceConfig,
    http: &reqwest::Client,
) -> Result<CheckOutcome, CheckError> {
    let def = defs()
        .iter()
        .find(|d| d.provider == provider)
        .ok_or_else(|| CheckError::UnknownProvider(provider.to_string()))?;

    let key = secrets
        .get(def.secret_name)
        .await
        .ok()
        .filter(|k| !k.is_empty())
        .ok_or(CheckError::NotConfigured(def.secret_name))?;

    let mut url = cfg
        .get(def.url_key)
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| def.default_url.to_string());

    // DeepL free key'leri (`:fx`) free host'u kullanır — sadece varsayılan
    // URL'de uygulanır; admin özel URL girdiyse ona dokunulmaz.
    if def.provider == "deepl" && key.ends_with(":fx") && url == def.default_url {
        url = url.replace("https://api.deepl.com", "https://api-free.deepl.com");
    }
    let url = url.replace("{key}", &key);

    let request = match def.auth {
        AuthStyle::Token => http.get(&url).header("Authorization", format!("Token {key}")),
        AuthStyle::Raw => http.get(&url).header("authorization", key.clone()),
        AuthStyle::DeeplKey => http
            .get(&url)
            .header("Authorization", format!("DeepL-Auth-Key {key}")),
        AuthStyle::QueryKey => http.get(&url),
        AuthStyle::Basic => http.get(&url).basic_auth(&key, Some("")),
        AuthStyle::XiApiKey => http.get(&url).header("xi-api-key", key.clone()),
    };

    match request.send().await {
        Ok(resp) => {
            let status = resp.status();
            Ok(CheckOutcome {
                provider: def.provider.to_string(),
                ok: status.is_success(),
                status: Some(status.as_u16()),
                detail: if status.is_success() {
                    format!("provider responded {status}")
                } else {
                    format!("provider rejected the key ({status})")
                },
            })
        }
        Err(err) => Ok(CheckOutcome {
            provider: def.provider.to_string(),
            ok: false,
            status: None,
            detail: format!("request failed: {err}"),
        }),
    }
}

/// `POST /admin/providers/{name}/test` (sadece admin).
pub async fn test_provider(
    admin: AdminUser,
    Path(name): Path<String>,
    State(secrets): State<Arc<ManagedSecrets>>,
    State(cfg): State<Arc<ServiceConfig>>,
    State(logger): State<Arc<Logger>>,
    State(test_store): State<Arc<ProviderTestStore>>,
) -> Result<Json<CheckOutcome>, (StatusCode, String)> {
    let http = reqwest::Client::new();
    match run_check(&name, &secrets, &cfg, &http).await {
        Ok(outcome) => {
            logger.log(
                LogLevel::Info,
                "providers",
                &format!(
                    "live check: {} → {}",
                    outcome.provider,
                    if outcome.ok { "ok" } else { "failed" }
                ),
                Some(&admin.claims.sub),
                None,
            );
            test_store.record(&outcome.provider, outcome.ok, &outcome.detail);
            Ok(Json(outcome))
        }
        Err(err @ CheckError::UnknownProvider(_)) => Err((StatusCode::NOT_FOUND, err.to_string())),
        Err(err @ CheckError::NotConfigured(_)) => {
            Err((StatusCode::SERVICE_UNAVAILABLE, err.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::MemorySecretStore;
    use axum::http::HeaderMap;
    use axum::routing::get;
    use axum::Router;

    /// Deepgram/AssemblyAI/DeepL/Stripe davranışını taklit eden mock:
    /// doğru kimlik → 200, yanlış → 401.
    async fn spawn_provider_mock() -> String {
        async fn deepgram(headers: HeaderMap) -> StatusCode {
            if headers.get("Authorization").and_then(|v| v.to_str().ok()) == Some("Token good-key") {
                StatusCode::OK
            } else {
                StatusCode::UNAUTHORIZED
            }
        }
        async fn google(
            axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
        ) -> StatusCode {
            if q.get("key").map(String::as_str) == Some("good-key") {
                StatusCode::OK
            } else {
                StatusCode::FORBIDDEN
            }
        }
        let app = Router::new()
            .route("/v1/projects", get(deepgram))
            .route("/v2/languages", get(google));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    fn managed_with(pairs: &[(&str, &str)]) -> ManagedSecrets {
        let store = MemorySecretStore::new();
        for (k, v) in pairs {
            store.insert(*k, *v);
        }
        ManagedSecrets::new(Box::new(store), "env")
    }

    #[tokio::test]
    async fn good_key_reports_ok_and_bad_key_reports_failure() {
        let base = spawn_provider_mock().await;
        let cfg = ServiceConfig::with_defaults();
        cfg.set("providers.deepgram.test_url", format!("{base}/v1/projects"));
        let http = reqwest::Client::new();

        let good = managed_with(&[(names::DEEPGRAM_KEY, "good-key")]);
        let outcome = run_check("deepgram", &good, &cfg, &http).await.unwrap();
        assert!(outcome.ok);
        assert_eq!(outcome.status, Some(200));

        let bad = managed_with(&[(names::DEEPGRAM_KEY, "bad-key")]);
        let outcome = run_check("deepgram", &bad, &cfg, &http).await.unwrap();
        assert!(!outcome.ok);
        assert_eq!(outcome.status, Some(401));
    }

    #[tokio::test]
    async fn query_key_style_replaces_placeholder() {
        let base = spawn_provider_mock().await;
        let cfg = ServiceConfig::with_defaults();
        cfg.set(
            "providers.google.test_url",
            format!("{base}/v2/languages?key={{key}}"),
        );
        let http = reqwest::Client::new();

        let good = managed_with(&[(names::GOOGLE_TRANSLATE_KEY, "good-key")]);
        assert!(run_check("google", &good, &cfg, &http).await.unwrap().ok);

        let bad = managed_with(&[(names::GOOGLE_TRANSLATE_KEY, "nope")]);
        assert!(!run_check("google", &bad, &cfg, &http).await.unwrap().ok);
    }

    #[tokio::test]
    async fn missing_key_is_not_configured() {
        let cfg = ServiceConfig::with_defaults();
        let http = reqwest::Client::new();
        let empty = managed_with(&[]);
        let err = run_check("deepgram", &empty, &cfg, &http).await.unwrap_err();
        assert!(matches!(err, CheckError::NotConfigured(_)));
    }

    #[tokio::test]
    async fn unknown_provider_is_rejected() {
        let cfg = ServiceConfig::with_defaults();
        let http = reqwest::Client::new();
        let empty = managed_with(&[]);
        let err = run_check("ghost", &empty, &cfg, &http).await.unwrap_err();
        assert!(matches!(err, CheckError::UnknownProvider(_)));
    }

    #[tokio::test]
    async fn network_failure_is_ok_false_not_error() {
        // Kapalı porta istek → test koşar ama ok=false döner (500 değil).
        let cfg = ServiceConfig::with_defaults();
        cfg.set("providers.deepgram.test_url", "http://127.0.0.1:1/v1/projects");
        let http = reqwest::Client::new();
        let good = managed_with(&[(names::DEEPGRAM_KEY, "good-key")]);
        let outcome = run_check("deepgram", &good, &cfg, &http).await.unwrap();
        assert!(!outcome.ok);
        assert_eq!(outcome.status, None);
        assert!(outcome.detail.contains("request failed"));
    }

    #[test]
    fn secret_to_provider_mapping_covers_testable_secrets() {
        assert_eq!(provider_for_secret(names::DEEPGRAM_KEY), Some("deepgram"));
        assert_eq!(provider_for_secret(names::STRIPE_SECRET_KEY), Some("stripe"));
        assert_eq!(provider_for_secret(names::STRIPE_WEBHOOK_SECRET), None);
        assert_eq!(provider_for_secret(names::YOUTUBE_CLIENT_ID), None);
    }

    #[test]
    fn provider_defs_lists_all_testable_providers_with_secret_names() {
        let defs = provider_defs();
        assert_eq!(defs.len(), 6);
        assert!(defs.contains(&("deepgram", names::DEEPGRAM_KEY)));
        assert!(defs.contains(&("stripe", names::STRIPE_SECRET_KEY)));
        assert!(defs.contains(&("elevenlabs", names::ELEVENLABS_KEY)));
    }

    #[test]
    fn provider_test_store_roundtrips_and_defaults_to_none() {
        let store = ProviderTestStore::new();
        assert!(store.get("deepgram").is_none());

        store.record("deepgram", true, "provider responded 200 OK");
        let record = store.get("deepgram").expect("recorded");
        assert!(record.ok);
        assert_eq!(record.detail, "provider responded 200 OK");
        assert!(record.tested_at_ms > 0);

        // Yeniden test → en son sonuçla üzerine yazar.
        store.record("deepgram", false, "provider rejected the key (401)");
        let record = store.get("deepgram").expect("recorded");
        assert!(!record.ok);

        assert!(store.get("stripe").is_none());
    }

    #[tokio::test]
    async fn deepl_free_key_switches_to_free_host_on_default_url_only() {
        // Varsayılan URL'de :fx → free host; özel URL'de dokunulmaz.
        // (Ağa çıkmadan sadece URL kurulumunu sınamak için kapalı porta özel
        // URL veriyoruz; free-host değişimi default URL'de network hatasında
        // bile detail'e yansımaz, bu yüzden burada sadece özel URL yolunu
        // doğruluyoruz.)
        let cfg = ServiceConfig::with_defaults();
        cfg.set("providers.deepl.test_url", "http://127.0.0.1:1/v2/usage");
        let http = reqwest::Client::new();
        let ms = managed_with(&[(names::DEEPL_KEY, "abc:fx")]);
        let outcome = run_check("deepl", &ms, &cfg, &http).await.unwrap();
        assert!(!outcome.ok); // kapalı port — ama free-host'a yönlenmedi, özel URL kullanıldı
        assert_eq!(outcome.status, None);
    }
}
