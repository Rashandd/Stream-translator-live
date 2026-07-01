//! Deepgram STT sağlayıcısı (Faz 1.7) — REST prerecorded `/v1/listen`.
//!
//! [`SttProvider`] trait'i request/response tipinde olduğu için burada
//! Deepgram'ın **prerecorded** REST endpoint'i kullanılır (bir ses parçası →
//! transkript). Gerçek zamanlı **streaming WebSocket** yolu ayrı bir
//! streaming-trait + pipeline değişikliği gerektirir; bu, canlı ses için
//! follow-up olarak planlanır (roadmap 1.7 not).
//!
//! API key koda gömülmez, `SecretStore`'dan (`DEEPGRAM_KEY`) gelir.

use async_trait::async_trait;

use super::stt::{SttError, SttProvider, Transcript};

const DEFAULT_ENDPOINT: &str = "https://api.deepgram.com/v1/listen";
const DEFAULT_MODEL: &str = "nova-2";

/// Deepgram prerecorded REST client'ı.
pub struct DeepgramStt {
    api_key: String,
    endpoint: String,
    model: String,
    client: reqwest::Client,
}

impl DeepgramStt {
    /// Nova-2 modeliyle üretim endpoint'ini kullanarak kurar.
    pub fn new(api_key: impl Into<String>) -> Self {
        DeepgramStt {
            api_key: api_key.into(),
            endpoint: DEFAULT_ENDPOINT.to_string(),
            model: DEFAULT_MODEL.to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Test veya self-host için özel endpoint ile kurar.
    pub fn with_endpoint(api_key: impl Into<String>, endpoint: impl Into<String>) -> Self {
        DeepgramStt {
            api_key: api_key.into(),
            endpoint: endpoint.into(),
            model: DEFAULT_MODEL.to_string(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl SttProvider for DeepgramStt {
    fn name(&self) -> &str {
        "deepgram"
    }

    async fn transcribe(&self, audio: &[u8], source_lang: &str) -> Result<Transcript, SttError> {
        if audio.is_empty() {
            return Err(SttError::Request("empty audio".into()));
        }

        let url = format!(
            "{}?model={}&language={}",
            self.endpoint, self.model, source_lang
        );

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Token {}", self.api_key))
            .body(audio.to_vec())
            .send()
            .await
            .map_err(|e| SttError::Request(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(SttError::Request(format!(
                "deepgram returned status {}",
                resp.status()
            )));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SttError::Request(e.to_string()))?;

        let text = body["results"]["channels"][0]["alternatives"][0]["transcript"]
            .as_str()
            .ok_or_else(|| SttError::Request("no transcript in response".into()))?;

        Ok(Transcript {
            text: text.to_string(),
            is_final: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Bytes;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::post;
    use axum::{Json, Router};

    /// Deepgram `/v1/listen`'i taklit eder: doğru token ister, byte sayısını yansıtır.
    async fn spawn_deepgram_mock() -> String {
        async fn listen(headers: HeaderMap, body: Bytes) -> (StatusCode, Json<serde_json::Value>) {
            if headers.get("Authorization").and_then(|v| v.to_str().ok()) != Some("Token test-key")
            {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({ "err_code": "INVALID_AUTH" })),
                );
            }
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "results": { "channels": [ { "alternatives": [
                        { "transcript": format!("{} bytes heard", body.len()), "confidence": 0.99 }
                    ] } ] }
                })),
            )
        }

        let app = Router::new().route("/v1/listen", post(listen));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}/v1/listen")
    }

    #[tokio::test]
    async fn transcribes_audio() {
        let endpoint = spawn_deepgram_mock().await;
        let stt = DeepgramStt::with_endpoint("test-key", endpoint);
        let t = stt.transcribe(&[1, 2, 3], "tr").await.unwrap();
        assert!(t.is_final);
        assert_eq!(t.text, "3 bytes heard");
    }

    #[tokio::test]
    async fn empty_audio_is_request_error() {
        let stt = DeepgramStt::with_endpoint("test-key", "http://127.0.0.1:1/v1/listen");
        assert!(matches!(
            stt.transcribe(&[], "tr").await,
            Err(SttError::Request(_))
        ));
    }

    #[tokio::test]
    async fn bad_token_is_request_error() {
        let endpoint = spawn_deepgram_mock().await;
        let stt = DeepgramStt::with_endpoint("wrong-key", endpoint);
        assert!(matches!(
            stt.transcribe(&[1, 2, 3], "tr").await,
            Err(SttError::Request(_))
        ));
    }
}
