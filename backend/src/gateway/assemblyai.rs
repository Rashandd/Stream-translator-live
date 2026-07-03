//! AssemblyAI STT sağlayıcısı (Faz 1.7) — Deepgram fallback.
//!
//! AssemblyAI'ın prerecorded API'si asenkron (upload → create transcript →
//! poll). [`SttProvider::transcribe`] senkron bir sonuç beklediği için bu
//! akış burada tek çağrıda yürütülür: ses yüklenir, transkript job'ı açılır,
//! `completed`/`error` olana kadar kısa aralıklarla poll edilir.
//!
//! API key koda gömülmez, `SecretStore`'dan (`ASSEMBLYAI_KEY`) gelir.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;

use super::stt::{SttError, SttProvider, Transcript};
use crate::secrets::{names, SecretRef, SecretStore};

const DEFAULT_BASE_URL: &str = "https://api.assemblyai.com";
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(1);
const DEFAULT_MAX_POLLS: u32 = 60;

#[derive(Deserialize)]
struct UploadResponse {
    upload_url: String,
}

#[derive(Deserialize)]
struct TranscriptResponse {
    id: String,
    status: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

/// AssemblyAI prerecorded REST client'ı.
pub struct AssemblyAiStt {
    key: SecretRef,
    base_url: String,
    client: reqwest::Client,
    poll_interval: Duration,
    max_polls: u32,
}

impl AssemblyAiStt {
    /// Üretim endpoint'i ve varsayılan poll ayarlarıyla kurar (sabit key).
    pub fn new(api_key: impl Into<String>) -> Self {
        AssemblyAiStt {
            key: SecretRef::fixed(api_key),
            base_url: DEFAULT_BASE_URL.to_string(),
            client: reqwest::Client::new(),
            poll_interval: DEFAULT_POLL_INTERVAL,
            max_polls: DEFAULT_MAX_POLLS,
        }
    }

    /// Key'i her çağrıda store'dan (`ASSEMBLYAI_KEY`) çözerek kurar.
    pub fn managed(store: Arc<dyn SecretStore>) -> Self {
        AssemblyAiStt {
            key: SecretRef::store(store, names::ASSEMBLYAI_KEY),
            base_url: DEFAULT_BASE_URL.to_string(),
            client: reqwest::Client::new(),
            poll_interval: DEFAULT_POLL_INTERVAL,
            max_polls: DEFAULT_MAX_POLLS,
        }
    }

    /// Test/self-host için özel base URL + hızlı poll ayarlarıyla kurar.
    #[cfg(test)]
    fn with_config(api_key: impl Into<String>, base_url: impl Into<String>, poll_interval: Duration, max_polls: u32) -> Self {
        AssemblyAiStt {
            key: SecretRef::fixed(api_key),
            base_url: base_url.into(),
            client: reqwest::Client::new(),
            poll_interval,
            max_polls,
        }
    }
}

#[async_trait]
impl SttProvider for AssemblyAiStt {
    fn name(&self) -> &str {
        "assemblyai"
    }

    async fn transcribe(&self, audio: &[u8], source_lang: &str) -> Result<Transcript, SttError> {
        if audio.is_empty() {
            return Err(SttError::Request("empty audio".into()));
        }

        let api_key = self
            .key
            .resolve()
            .await
            .ok_or_else(|| SttError::Unavailable("ASSEMBLYAI_KEY not configured".into()))?;

        let upload: UploadResponse = self
            .client
            .post(format!("{}/v2/upload", self.base_url))
            .header("authorization", &api_key)
            .body(audio.to_vec())
            .send()
            .await
            .map_err(|e| SttError::Request(e.to_string()))?
            .error_for_status()
            .map_err(|e| SttError::Request(e.to_string()))?
            .json()
            .await
            .map_err(|e| SttError::Request(e.to_string()))?;

        let created: TranscriptResponse = self
            .client
            .post(format!("{}/v2/transcript", self.base_url))
            .header("authorization", &api_key)
            .json(&serde_json::json!({
                "audio_url": upload.upload_url,
                "language_code": source_lang,
            }))
            .send()
            .await
            .map_err(|e| SttError::Request(e.to_string()))?
            .error_for_status()
            .map_err(|e| SttError::Request(e.to_string()))?
            .json()
            .await
            .map_err(|e| SttError::Request(e.to_string()))?;

        self.poll_until_done(&created.id, &api_key).await
    }
}

impl AssemblyAiStt {
    /// Transkript job'ı bitene kadar poll eder. `completed` → metin, `error` →
    /// sağlayıcı hatası, tur sınırı aşılırsa zaman aşımı hatası.
    async fn poll_until_done(&self, id: &str, api_key: &str) -> Result<Transcript, SttError> {
        let url = format!("{}/v2/transcript/{id}", self.base_url);
        for _ in 0..self.max_polls {
            let poll: TranscriptResponse = self
                .client
                .get(&url)
                .header("authorization", api_key)
                .send()
                .await
                .map_err(|e| SttError::Request(e.to_string()))?
                .error_for_status()
                .map_err(|e| SttError::Request(e.to_string()))?
                .json()
                .await
                .map_err(|e| SttError::Request(e.to_string()))?;

            match poll.status.as_str() {
                "completed" => {
                    let text = poll.text.unwrap_or_default();
                    return Ok(Transcript { text, is_final: true });
                }
                "error" => {
                    return Err(SttError::Request(
                        poll.error.unwrap_or_else(|| "assemblyai transcription error".to_string()),
                    ));
                }
                _ => tokio::time::sleep(self.poll_interval).await,
            }
        }
        Err(SttError::Request("assemblyai polling timed out".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::{Path, State};
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::{get, post};
    use axum::{Json, Router};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[derive(Clone)]
    struct MockState {
        polls_until_done: Arc<AtomicU32>,
        fail: bool,
    }

    async fn upload(headers: HeaderMap) -> Result<Json<serde_json::Value>, StatusCode> {
        if headers.get("authorization").and_then(|v| v.to_str().ok()) != Some("test-key") {
            return Err(StatusCode::UNAUTHORIZED);
        }
        Ok(Json(serde_json::json!({ "upload_url": "https://upload.example/audio.raw" })))
    }

    async fn create_transcript() -> Json<serde_json::Value> {
        Json(serde_json::json!({ "id": "job-1", "status": "queued" }))
    }

    async fn get_transcript(
        State(state): State<MockState>,
        Path(_id): Path<String>,
    ) -> Json<serde_json::Value> {
        if state.fail {
            return Json(serde_json::json!({ "id": "job-1", "status": "error", "error": "boom" }));
        }
        let remaining = state.polls_until_done.fetch_sub(1, Ordering::SeqCst);
        if remaining <= 1 {
            Json(serde_json::json!({ "id": "job-1", "status": "completed", "text": "merhaba dunya" }))
        } else {
            Json(serde_json::json!({ "id": "job-1", "status": "processing" }))
        }
    }

    async fn spawn_mock(polls_until_done: u32, fail: bool) -> String {
        let state = MockState {
            polls_until_done: Arc::new(AtomicU32::new(polls_until_done)),
            fail,
        };
        let app = Router::new()
            .route("/v2/upload", post(upload))
            .route("/v2/transcript", post(create_transcript))
            .route("/v2/transcript/{id}", get(get_transcript))
            .with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn empty_audio_is_request_error() {
        let stt = AssemblyAiStt::with_config("test-key", "http://127.0.0.1:1", Duration::from_millis(1), 3);
        assert!(matches!(stt.transcribe(&[], "tr").await, Err(SttError::Request(_))));
    }

    #[tokio::test]
    async fn transcribes_after_polling() {
        let base = spawn_mock(3, false).await;
        let stt = AssemblyAiStt::with_config("test-key", base, Duration::from_millis(1), 10);
        let t = stt.transcribe(&[1, 2, 3], "tr").await.unwrap();
        assert!(t.is_final);
        assert_eq!(t.text, "merhaba dunya");
    }

    #[tokio::test]
    async fn bad_key_is_request_error() {
        let base = spawn_mock(1, false).await;
        let stt = AssemblyAiStt::with_config("wrong-key", base, Duration::from_millis(1), 10);
        assert!(matches!(
            stt.transcribe(&[1, 2, 3], "tr").await,
            Err(SttError::Request(_))
        ));
    }

    #[tokio::test]
    async fn provider_error_status_is_surfaced() {
        let base = spawn_mock(1, true).await;
        let stt = AssemblyAiStt::with_config("test-key", base, Duration::from_millis(1), 10);
        let err = stt.transcribe(&[1, 2, 3], "tr").await.unwrap_err();
        assert!(matches!(err, SttError::Request(msg) if msg.contains("boom")));
    }

    #[tokio::test]
    async fn polling_timeout_is_request_error() {
        let base = spawn_mock(100, false).await;
        let stt = AssemblyAiStt::with_config("test-key", base, Duration::from_millis(1), 3);
        let err = stt.transcribe(&[1, 2, 3], "tr").await.unwrap_err();
        assert!(matches!(err, SttError::Request(msg) if msg.contains("timed out")));
    }
}
