//! Self-hosted GPU STT sağlayıcısı (roadmap 2.6 adım 3).
//!
//! `DeepgramStt` ile aynı REST/prerecorded şekil, ama sabit bir API key yerine
//! `SttServerStore::active_ready()`'yi her çağrıda okur: admin panelden hangi
//! sunucu `Ready` ise (bkz. `stt_servers` modülü, adım 1-2) o kullanılır —
//! restart gerekmeden yeni bir sunucuya geçilebilir. Hiç `Ready` sunucu yoksa
//! `SttError::Unavailable` döner; mevcut `stt.fallback` zinciri (Gateway,
//! değişmedi) devreye girer.
//!
//! Yanıt şekli Deepgram'ın aksine iç içe değil: kendi `stt-server` servisimiz
//! (adım 4) `{"text": ..., "is_final": true}` döner, bu yüzden ayrıştırma
//! doğrudan `Transcript`'e map'lenir.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;

use super::stt::{SttError, SttProvider, Transcript};
use crate::stt_servers::SttServerStore;

/// stt-server'ın `/transcribe` yanıtı.
#[derive(Debug, Deserialize)]
struct TranscribeResponse {
    text: String,
    is_final: bool,
}

/// Self-hosted GPU STT REST client'ı.
pub struct SelfHostedStt {
    servers: Arc<SttServerStore>,
    client: reqwest::Client,
}

impl SelfHostedStt {
    pub fn new(servers: Arc<SttServerStore>) -> Self {
        SelfHostedStt {
            servers,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl SttProvider for SelfHostedStt {
    fn name(&self) -> &str {
        "selfhosted"
    }

    async fn transcribe(&self, audio: &[u8], source_lang: &str) -> Result<Transcript, SttError> {
        if audio.is_empty() {
            return Err(SttError::Request("empty audio".into()));
        }

        let (server, api_token) = self
            .servers
            .active_ready()
            .ok_or_else(|| SttError::Unavailable("no self-hosted STT server is ready".into()))?;
        let endpoint = server
            .endpoint_url
            .as_deref()
            .ok_or_else(|| SttError::Unavailable("ready server has no endpoint_url".into()))?;

        let url = format!("{}/transcribe?language={source_lang}", endpoint.trim_end_matches('/'));

        let resp = self
            .client
            .post(&url)
            .bearer_auth(api_token)
            .body(audio.to_vec())
            .send()
            .await
            .map_err(|e| SttError::Request(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(SttError::Request(format!(
                "self-hosted stt server returned status {}",
                resp.status()
            )));
        }

        let body: TranscribeResponse = resp
            .json()
            .await
            .map_err(|e| SttError::Request(e.to_string()))?;

        Ok(Transcript {
            text: body.text,
            is_final: body.is_final,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::post;
    use axum::{Json, Router};

    /// stt-server'ı taklit eder: doğru bearer token ister, byte sayısını yansıtır.
    async fn spawn_mock() -> String {
        async fn transcribe(headers: HeaderMap, body: axum::body::Bytes) -> (StatusCode, Json<serde_json::Value>) {
            if headers.get("Authorization").and_then(|v| v.to_str().ok()) != Some("Bearer test-token")
            {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({ "error": "invalid token" })),
                );
            }
            (
                StatusCode::OK,
                Json(serde_json::json!({ "text": format!("{} bytes heard", body.len()), "is_final": true })),
            )
        }

        let app = Router::new().route("/transcribe", post(transcribe));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn transcribes_using_active_ready_server() {
        let store = Arc::new(SttServerStore::in_memory());
        let server = store.add("gpu-1", "1.2.3.4", 22, "root").expect("add");
        let endpoint = spawn_mock().await;
        store
            .mark_ready(&server.id, &endpoint, "test-token", Some("RTX 3070".into()), Some(8192))
            .expect("mark_ready");

        let stt = SelfHostedStt::new(store);
        let t = stt.transcribe(&[1, 2, 3], "tr").await.unwrap();
        assert!(t.is_final);
        assert_eq!(t.text, "3 bytes heard");
    }

    #[tokio::test]
    async fn no_ready_server_is_unavailable() {
        let store = Arc::new(SttServerStore::in_memory());
        let stt = SelfHostedStt::new(store);
        assert!(matches!(
            stt.transcribe(&[1, 2, 3], "tr").await,
            Err(SttError::Unavailable(_))
        ));
    }

    #[tokio::test]
    async fn empty_audio_is_request_error() {
        let store = Arc::new(SttServerStore::in_memory());
        let stt = SelfHostedStt::new(store);
        assert!(matches!(
            stt.transcribe(&[], "tr").await,
            Err(SttError::Request(_))
        ));
    }

    #[tokio::test]
    async fn wrong_token_is_request_error() {
        let store = Arc::new(SttServerStore::in_memory());
        let server = store.add("gpu-1", "1.2.3.4", 22, "root").expect("add");
        let endpoint = spawn_mock().await;
        store
            .mark_ready(&server.id, &endpoint, "wrong-token", None, None)
            .expect("mark_ready");

        let stt = SelfHostedStt::new(store);
        assert!(matches!(
            stt.transcribe(&[1, 2, 3], "tr").await,
            Err(SttError::Request(_))
        ));
    }
}
