//! DeepL çeviri sağlayıcısı (Faz 1.8) — REST `/v2/translate`.
//!
//! [`TranslateProvider`] trait'ini implemente eder; gateway config'te
//! `translate.provider = deepl` ise devreye girer. API key koda gömülmez,
//! `SecretStore`'dan (`DEEPL_KEY`) gelir. Free key'ler `:fx` ile biter ve
//! `api-free.deepl.com` endpoint'ini kullanır.

use std::sync::Arc;

use async_trait::async_trait;

use super::translate::{TranslateError, TranslateProvider};
use crate::secrets::{names, SecretRef, SecretStore};

const FREE_ENDPOINT: &str = "https://api-free.deepl.com/v2/translate";
const PRO_ENDPOINT: &str = "https://api.deepl.com/v2/translate";

/// Free key'ler `:fx` ile biter ve free endpoint'ini kullanır.
fn endpoint_for_key(api_key: &str) -> &'static str {
    if api_key.ends_with(":fx") {
        FREE_ENDPOINT
    } else {
        PRO_ENDPOINT
    }
}

/// DeepL REST client'ı.
pub struct DeeplTranslate {
    key: SecretRef,
    /// `None` = endpoint çağrı anında key'e göre seçilir (free/pro).
    endpoint: Option<String>,
    client: reqwest::Client,
}

impl DeeplTranslate {
    /// API key'e göre free/pro endpoint'ini seçerek kurar (sabit key).
    pub fn new(api_key: impl Into<String>) -> Self {
        DeeplTranslate {
            key: SecretRef::fixed(api_key),
            endpoint: None,
            client: reqwest::Client::new(),
        }
    }

    /// Key'i her çağrıda store'dan (`DEEPL_KEY`) çözerek kurar; endpoint de
    /// o anki key'e göre (free/pro) seçilir.
    pub fn managed(store: Arc<dyn SecretStore>) -> Self {
        DeeplTranslate {
            key: SecretRef::store(store, names::DEEPL_KEY),
            endpoint: None,
            client: reqwest::Client::new(),
        }
    }

    /// Test veya self-host için özel endpoint ile kurar.
    pub fn with_endpoint(api_key: impl Into<String>, endpoint: impl Into<String>) -> Self {
        DeeplTranslate {
            key: SecretRef::fixed(api_key),
            endpoint: Some(endpoint.into()),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl TranslateProvider for DeeplTranslate {
    fn name(&self) -> &str {
        "deepl"
    }

    async fn translate(
        &self,
        text: &str,
        source: &str,
        target: &str,
    ) -> Result<String, TranslateError> {
        if text.is_empty() {
            return Err(TranslateError::Request("empty text".into()));
        }

        let api_key = self
            .key
            .resolve()
            .await
            .ok_or_else(|| TranslateError::Unavailable("DEEPL_KEY not configured".into()))?;
        let endpoint = self
            .endpoint
            .clone()
            .unwrap_or_else(|| endpoint_for_key(&api_key).to_string());

        // DeepL dil kodları büyük harf (EN, DE, TR). `source_lang=auto`
        // (Faz 2.0 — yayıncı kaynak dili elle seçmek istemezse) DeepL'in
        // kendi oto-algılamasını tetiklemek için parametrenin TAMAMEN
        // GÖNDERİLMEMESİ gerekir — DeepL "AUTO" diye bir kaynak kodu
        // tanımıyor, `source_lang` alanı yoksa oto-algılar.
        let mut params = vec![
            ("text", text.to_string()),
            ("target_lang", target.to_uppercase()),
        ];
        if !source.eq_ignore_ascii_case("auto") {
            params.push(("source_lang", source.to_uppercase()));
        }

        let resp = self
            .client
            .post(&endpoint)
            .header("Authorization", format!("DeepL-Auth-Key {api_key}"))
            .form(&params)
            .send()
            .await
            .map_err(|e| TranslateError::Request(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(TranslateError::Request(format!(
                "deepl returned status {}",
                resp.status()
            )));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| TranslateError::Request(e.to_string()))?;

        body["translations"][0]["text"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| TranslateError::Request("no translation in response".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::Form;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::post;
    use axum::{Json, Router};
    use std::collections::HashMap;

    /// DeepL `/v2/translate`'i taklit eder: doğru auth ister, `TARGET:text` döner.
    async fn spawn_deepl_mock() -> String {
        async fn translate(
            headers: HeaderMap,
            Form(form): Form<HashMap<String, String>>,
        ) -> (StatusCode, Json<serde_json::Value>) {
            if headers.get("Authorization").and_then(|v| v.to_str().ok())
                != Some("DeepL-Auth-Key test-key")
            {
                return (
                    StatusCode::FORBIDDEN,
                    Json(serde_json::json!({ "message": "auth failed" })),
                );
            }
            let text = form.get("text").cloned().unwrap_or_default();
            let target = form.get("target_lang").cloned().unwrap_or_default();
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "translations": [ { "detected_source_language": "TR", "text": format!("{target}:{text}") } ]
                })),
            )
        }

        let app = Router::new().route("/v2/translate", post(translate));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}/v2/translate")
    }

    /// `spawn_deepl_mock` gibi ama her isteğin `source_lang` alanını
    /// gönderip göndermediğini paylaşılan bir listeye kaydeder — auto-detect
    /// testi bunu bekleyip doğrular.
    async fn spawn_deepl_mock_recording_source_lang() -> (String, std::sync::Arc<std::sync::Mutex<Vec<bool>>>) {
        let seen: std::sync::Arc<std::sync::Mutex<Vec<bool>>> = Default::default();
        let seen_for_route = std::sync::Arc::clone(&seen);

        async fn translate(
            axum::extract::State(seen): axum::extract::State<std::sync::Arc<std::sync::Mutex<Vec<bool>>>>,
            Form(form): Form<HashMap<String, String>>,
        ) -> Json<serde_json::Value> {
            seen.lock().unwrap().push(form.contains_key("source_lang"));
            let text = form.get("text").cloned().unwrap_or_default();
            let target = form.get("target_lang").cloned().unwrap_or_default();
            Json(serde_json::json!({
                "translations": [ { "detected_source_language": "TR", "text": format!("{target}:{text}") } ]
            }))
        }

        let app = Router::new()
            .route("/v2/translate", post(translate))
            .with_state(seen_for_route);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (format!("http://{addr}/v2/translate"), seen)
    }

    #[tokio::test]
    async fn auto_source_lang_omits_the_form_field() {
        let (endpoint, seen) = spawn_deepl_mock_recording_source_lang().await;
        let tr = DeeplTranslate::with_endpoint("test-key", endpoint);

        tr.translate("hello", "auto", "tr").await.unwrap();
        tr.translate("hello", "AUTO", "tr").await.unwrap(); // case-insensitive
        tr.translate("merhaba", "tr", "en").await.unwrap(); // normal — alan gönderilmeli

        let seen = seen.lock().unwrap().clone();
        assert_eq!(seen, vec![false, false, true]);
    }

    #[tokio::test]
    async fn translates_and_uppercases_lang() {
        let endpoint = spawn_deepl_mock().await;
        let tr = DeeplTranslate::with_endpoint("test-key", endpoint);
        // target "en" büyük harfe (EN) çevrilip gönderilmeli.
        assert_eq!(tr.translate("merhaba", "tr", "en").await.unwrap(), "EN:merhaba");
    }

    #[tokio::test]
    async fn empty_text_is_request_error() {
        let tr = DeeplTranslate::with_endpoint("test-key", "http://127.0.0.1:1/v2/translate");
        assert!(matches!(
            tr.translate("", "tr", "en").await,
            Err(TranslateError::Request(_))
        ));
    }

    #[tokio::test]
    async fn bad_key_is_request_error() {
        let endpoint = spawn_deepl_mock().await;
        let tr = DeeplTranslate::with_endpoint("wrong-key", endpoint);
        assert!(matches!(
            tr.translate("merhaba", "tr", "en").await,
            Err(TranslateError::Request(_))
        ));
    }

    #[test]
    fn free_key_selects_free_endpoint() {
        // Endpoint artık çağrı anında key'e göre seçilir (managed key'ler
        // runtime'da değişebildiği için yapım anında sabitlenemez).
        assert_eq!(endpoint_for_key("abc:fx"), FREE_ENDPOINT);
        assert_eq!(endpoint_for_key("abc"), PRO_ENDPOINT);
    }

    #[tokio::test]
    async fn managed_key_set_at_runtime_reaches_real_endpoint() {
        use crate::secrets::{MemorySecretStore, SecretStore};
        use std::sync::Arc;

        // Store'a bağlı client: key sonradan girilse de çağrı anında çözülür.
        let endpoint = spawn_deepl_mock().await;
        let store = Arc::new(MemorySecretStore::new());
        let tr = DeeplTranslate {
            key: crate::secrets::SecretRef::store(
                Arc::clone(&store) as Arc<dyn SecretStore>,
                "DEEPL_KEY",
            ),
            endpoint: Some(endpoint),
            client: reqwest::Client::new(),
        };

        // Key yok → Unavailable (gateway fallback'ini tetikleyen tür).
        assert!(matches!(
            tr.translate("merhaba", "tr", "en").await,
            Err(TranslateError::Unavailable(_))
        ));

        // Key girildi (admin panel) → restart'sız gerçek endpoint'e ulaşır.
        store.insert("DEEPL_KEY", "test-key");
        let out = tr.translate("merhaba", "tr", "en").await.unwrap();
        assert_eq!(out, "EN:merhaba");
    }
}
