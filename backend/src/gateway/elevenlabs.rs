//! ElevenLabs ses klonlama + TTS sağlayıcısı (Faz 2.1/2.2).
//!
//! [`TtsProvider`] trait'ini implemente eder. İki gerçek ElevenLabs uç
//! noktası kullanılır: `POST /v1/voices/add` (multipart — ses örneğinden
//! model oluşturur) ve `POST /v1/text-to-speech/{voice_id}` (JSON — metni
//! o modelle seslendirir, ham ses baytı döner). API key koda gömülmez,
//! `SecretStore`'dan (`ELEVENLABS_KEY`) gelir.

use std::sync::Arc;

use async_trait::async_trait;

use super::tts::{TtsError, TtsProvider};
use crate::secrets::{names, SecretRef, SecretStore};

const DEFAULT_BASE_URL: &str = "https://api.elevenlabs.io";
const DEFAULT_MODEL_ID: &str = "eleven_multilingual_v2";

/// ElevenLabs REST client'ı.
pub struct ElevenLabsTts {
    key: SecretRef,
    base_url: String,
    client: reqwest::Client,
}

impl ElevenLabsTts {
    /// Sabit key ile üretim base URL'ini kullanarak kurar.
    pub fn new(api_key: impl Into<String>) -> Self {
        ElevenLabsTts {
            key: SecretRef::fixed(api_key),
            base_url: DEFAULT_BASE_URL.to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Key'i her çağrıda store'dan (`ELEVENLABS_KEY`) çözerek kurar — admin
    /// panelden girilen key restart gerektirmeden devreye girer.
    pub fn managed(store: Arc<dyn SecretStore>) -> Self {
        ElevenLabsTts {
            key: SecretRef::store(store, names::ELEVENLABS_KEY),
            base_url: DEFAULT_BASE_URL.to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Test veya self-host için özel base URL ile kurar.
    pub fn with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        ElevenLabsTts {
            key: SecretRef::fixed(api_key),
            base_url: base_url.into(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl TtsProvider for ElevenLabsTts {
    fn name(&self) -> &str {
        "elevenlabs"
    }

    async fn clone_voice(&self, name: &str, sample: &[u8]) -> Result<String, TtsError> {
        if sample.is_empty() {
            return Err(TtsError::Request("empty sample".into()));
        }
        let api_key = self
            .key
            .resolve()
            .await
            .ok_or_else(|| TtsError::Unavailable("ELEVENLABS_KEY not configured".into()))?;

        let part = reqwest::multipart::Part::bytes(sample.to_vec())
            .file_name("sample.wav")
            .mime_str("audio/wav")
            .map_err(|e| TtsError::Request(e.to_string()))?;
        let form = reqwest::multipart::Form::new()
            .text("name", name.to_string())
            .part("files", part);

        let resp = self
            .client
            .post(format!("{}/v1/voices/add", self.base_url))
            .header("xi-api-key", api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|e| TtsError::Request(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(TtsError::Request(format!(
                "elevenlabs returned status {}",
                resp.status()
            )));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| TtsError::Request(e.to_string()))?;

        body["voice_id"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| TtsError::Request("no voice_id in response".into()))
    }

    async fn synthesize(&self, voice_id: &str, text: &str) -> Result<Vec<u8>, TtsError> {
        if text.is_empty() {
            return Err(TtsError::Request("empty text".into()));
        }
        let api_key = self
            .key
            .resolve()
            .await
            .ok_or_else(|| TtsError::Unavailable("ELEVENLABS_KEY not configured".into()))?;

        let resp = self
            .client
            .post(format!("{}/v1/text-to-speech/{voice_id}", self.base_url))
            .header("xi-api-key", api_key)
            .json(&serde_json::json!({ "text": text, "model_id": DEFAULT_MODEL_ID }))
            .send()
            .await
            .map_err(|e| TtsError::Request(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(TtsError::Request(format!(
                "elevenlabs returned status {}",
                resp.status()
            )));
        }

        resp.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| TtsError::Request(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::Multipart;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::post;
    use axum::{Json, Router};

    /// ElevenLabs `/v1/voices/add` + `/v1/text-to-speech/{id}`'i taklit eder.
    async fn spawn_elevenlabs_mock() -> String {
        async fn voices_add(
            headers: HeaderMap,
            mut multipart: Multipart,
        ) -> Result<Json<serde_json::Value>, StatusCode> {
            if headers.get("xi-api-key").and_then(|v| v.to_str().ok()) != Some("test-key") {
                return Err(StatusCode::UNAUTHORIZED);
            }
            let mut name = None;
            let mut sample_len = 0usize;
            while let Some(field) = multipart.next_field().await.map_err(|_| StatusCode::BAD_REQUEST)? {
                match field.name() {
                    Some("name") => name = Some(field.text().await.map_err(|_| StatusCode::BAD_REQUEST)?),
                    Some("files") => {
                        sample_len = field.bytes().await.map_err(|_| StatusCode::BAD_REQUEST)?.len();
                    }
                    _ => {}
                }
            }
            Ok(Json(serde_json::json!({
                "voice_id": format!("voice-{}-{sample_len}", name.unwrap_or_default()),
            })))
        }

        async fn text_to_speech(headers: HeaderMap, Json(body): Json<serde_json::Value>) -> Result<Vec<u8>, StatusCode> {
            if headers.get("xi-api-key").and_then(|v| v.to_str().ok()) != Some("test-key") {
                return Err(StatusCode::UNAUTHORIZED);
            }
            let text = body["text"].as_str().unwrap_or_default();
            Ok(format!("audio-bytes:{text}").into_bytes())
        }

        let app = Router::new()
            .route("/v1/voices/add", post(voices_add))
            .route("/v1/text-to-speech/{voice_id}", post(text_to_speech));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn clones_a_voice() {
        let base = spawn_elevenlabs_mock().await;
        let tts = ElevenLabsTts::with_base_url("test-key", base);
        let voice_id = tts.clone_voice("my voice", &[1, 2, 3, 4]).await.unwrap();
        assert_eq!(voice_id, "voice-my voice-4");
    }

    #[tokio::test]
    async fn synthesizes_speech() {
        let base = spawn_elevenlabs_mock().await;
        let tts = ElevenLabsTts::with_base_url("test-key", base);
        let audio = tts.synthesize("voice-1", "merhaba").await.unwrap();
        assert_eq!(audio, b"audio-bytes:merhaba");
    }

    #[tokio::test]
    async fn empty_sample_and_text_are_request_errors() {
        let tts = ElevenLabsTts::with_base_url("test-key", "http://127.0.0.1:1");
        assert!(matches!(tts.clone_voice("x", &[]).await, Err(TtsError::Request(_))));
        assert!(matches!(tts.synthesize("v", "").await, Err(TtsError::Request(_))));
    }

    #[tokio::test]
    async fn bad_key_is_request_error() {
        let base = spawn_elevenlabs_mock().await;
        let tts = ElevenLabsTts::with_base_url("wrong-key", base);
        assert!(matches!(
            tts.clone_voice("x", &[1, 2, 3]).await,
            Err(TtsError::Request(_))
        ));
    }
}
