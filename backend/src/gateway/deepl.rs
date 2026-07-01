//! DeepL çeviri sağlayıcısı (Faz 1.8) — REST `/v2/translate`.
//!
//! [`TranslateProvider`] trait'ini implemente eder; gateway config'te
//! `translate.provider = deepl` ise devreye girer. API key koda gömülmez,
//! `SecretStore`'dan (`DEEPL_KEY`) gelir. Free key'ler `:fx` ile biter ve
//! `api-free.deepl.com` endpoint'ini kullanır.

use async_trait::async_trait;

use super::translate::{TranslateError, TranslateProvider};

const FREE_ENDPOINT: &str = "https://api-free.deepl.com/v2/translate";
const PRO_ENDPOINT: &str = "https://api.deepl.com/v2/translate";

/// DeepL REST client'ı.
pub struct DeeplTranslate {
    api_key: String,
    endpoint: String,
    client: reqwest::Client,
}

impl DeeplTranslate {
    /// API key'e göre free/pro endpoint'ini seçerek kurar.
    pub fn new(api_key: impl Into<String>) -> Self {
        let api_key = api_key.into();
        let endpoint = if api_key.ends_with(":fx") {
            FREE_ENDPOINT
        } else {
            PRO_ENDPOINT
        };
        DeeplTranslate {
            api_key,
            endpoint: endpoint.to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Test veya self-host için özel endpoint ile kurar.
    pub fn with_endpoint(api_key: impl Into<String>, endpoint: impl Into<String>) -> Self {
        DeeplTranslate {
            api_key: api_key.into(),
            endpoint: endpoint.into(),
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

        // DeepL dil kodları büyük harf (EN, DE, TR).
        let params = [
            ("text", text.to_string()),
            ("source_lang", source.to_uppercase()),
            ("target_lang", target.to_uppercase()),
        ];

        let resp = self
            .client
            .post(&self.endpoint)
            .header("Authorization", format!("DeepL-Auth-Key {}", self.api_key))
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

    #[tokio::test]
    async fn free_key_selects_free_endpoint() {
        let tr = DeeplTranslate::new("abc:fx");
        assert_eq!(tr.endpoint, FREE_ENDPOINT);
        let pro = DeeplTranslate::new("abc");
        assert_eq!(pro.endpoint, PRO_ENDPOINT);
    }
}
