//! Google Cloud Translate sağlayıcısı (Faz 1.8) — DeepL fallback.
//!
//! REST `v2/translate` (API key, `key` query param — service account/OAuth
//! değil, basit API key akışı). API key koda gömülmez, `SecretStore`'dan
//! (`GOOGLE_TRANSLATE_KEY`) gelir.

use std::sync::Arc;

use async_trait::async_trait;

use super::translate::{TranslateError, TranslateProvider};
use crate::secrets::{names, SecretRef, SecretStore};

const DEFAULT_ENDPOINT: &str = "https://translation.googleapis.com/language/translate/v2";

/// Google Cloud Translate REST client'ı.
pub struct GoogleTranslate {
    key: SecretRef,
    endpoint: String,
    client: reqwest::Client,
}

impl GoogleTranslate {
    /// Üretim endpoint'iyle kurar (sabit key).
    pub fn new(api_key: impl Into<String>) -> Self {
        GoogleTranslate {
            key: SecretRef::fixed(api_key),
            endpoint: DEFAULT_ENDPOINT.to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Key'i her çağrıda store'dan (`GOOGLE_TRANSLATE_KEY`) çözerek kurar.
    pub fn managed(store: Arc<dyn SecretStore>) -> Self {
        GoogleTranslate {
            key: SecretRef::store(store, names::GOOGLE_TRANSLATE_KEY),
            endpoint: DEFAULT_ENDPOINT.to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Test veya self-host için özel endpoint ile kurar.
    pub fn with_endpoint(api_key: impl Into<String>, endpoint: impl Into<String>) -> Self {
        GoogleTranslate {
            key: SecretRef::fixed(api_key),
            endpoint: endpoint.into(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl TranslateProvider for GoogleTranslate {
    fn name(&self) -> &str {
        "google"
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

        let api_key = self.key.resolve().await.ok_or_else(|| {
            TranslateError::Unavailable("GOOGLE_TRANSLATE_KEY not configured".into())
        })?;

        let resp = self
            .client
            .post(&self.endpoint)
            .query(&[("key", api_key.as_str())])
            .json(&serde_json::json!({
                "q": text,
                "source": source,
                "target": target,
                "format": "text",
            }))
            .send()
            .await
            .map_err(|e| TranslateError::Request(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(TranslateError::Request(format!(
                "google translate returned status {}",
                resp.status()
            )));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| TranslateError::Request(e.to_string()))?;

        body["data"]["translations"][0]["translatedText"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| TranslateError::Request("no translation in response".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::{Json as JsonExtract, Query};
    use axum::http::StatusCode;
    use axum::routing::post;
    use axum::{Json, Router};
    use std::collections::HashMap;

    /// Google `v2/translate`'i taklit eder: `key` query param ister,
    /// `TARGET:text` döner.
    async fn spawn_google_mock() -> String {
        async fn translate(
            Query(params): Query<HashMap<String, String>>,
            JsonExtract(body): JsonExtract<serde_json::Value>,
        ) -> (StatusCode, Json<serde_json::Value>) {
            if params.get("key").map(String::as_str) != Some("test-key") {
                return (
                    StatusCode::FORBIDDEN,
                    Json(serde_json::json!({ "error": { "message": "invalid key" } })),
                );
            }
            let text = body["q"].as_str().unwrap_or_default();
            let target = body["target"].as_str().unwrap_or_default();
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "data": { "translations": [ { "translatedText": format!("{target}:{text}") } ] }
                })),
            )
        }

        let app = Router::new().route("/language/translate/v2", post(translate));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}/language/translate/v2")
    }

    #[tokio::test]
    async fn translates_via_rest() {
        let endpoint = spawn_google_mock().await;
        let tr = GoogleTranslate::with_endpoint("test-key", endpoint);
        assert_eq!(tr.translate("merhaba", "tr", "en").await.unwrap(), "en:merhaba");
    }

    #[tokio::test]
    async fn empty_text_is_request_error() {
        let tr = GoogleTranslate::with_endpoint("test-key", "http://127.0.0.1:1/language/translate/v2");
        assert!(matches!(tr.translate("", "tr", "en").await, Err(TranslateError::Request(_))));
    }

    #[tokio::test]
    async fn bad_key_is_request_error() {
        let endpoint = spawn_google_mock().await;
        let tr = GoogleTranslate::with_endpoint("wrong-key", endpoint);
        assert!(matches!(
            tr.translate("merhaba", "tr", "en").await,
            Err(TranslateError::Request(_))
        ));
    }
}
