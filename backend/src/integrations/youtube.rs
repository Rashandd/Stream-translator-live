//! YouTube Live Native Closed Captions (Faz 1.10).
//!
//! YouTube Live yayınları harici altyazı akışını
//! `liveBroadcasts.contentDetails.closedCaptionsIngestionUrl` üzerinden kabul
//! eder: bu URL, OAuth bağlantısı kurulduktan sonra Data API'den çözülür,
//! ardından düz UTF-8 metin bu URL'e POST edilir (sıralama için `seq` query
//! param). Bkz. YouTube Data API — "Transcribing video with captions".
//!
//! NOT: Google'ın offline refresh token akışı authorize isteğine
//! `access_type=offline&prompt=consent` eklenmesini gerektirir; paylaşılan
//! `integrations::oauth` çekirdeği bunu henüz desteklemiyor (diğer
//! sağlayıcılarla ortak akış). Canlı entegrasyon öncesi follow-up.

use async_trait::async_trait;

const YOUTUBE_API_BASE: &str = "https://www.googleapis.com/youtube/v3";

/// YouTube Data API hataları.
#[derive(Debug, thiserror::Error)]
pub enum YouTubeError {
    #[error("youtube api request failed: {0}")]
    Request(String),
    #[error("broadcast '{0}' has no closed captions ingestion url (canlı yayında mı?)")]
    NoIngestionUrl(String),
}

/// YouTube Live closed-caption REST client'ı.
pub struct YouTubeCaptions {
    client: reqwest::Client,
    api_base: String,
}

impl Default for YouTubeCaptions {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
pub trait CaptionsSink: Send + Sync {
    async fn fetch_ingestion_url(&self, access_token: &str, broadcast_id: &str) -> Result<String, YouTubeError>;
    async fn push_caption(&self, ingestion_url: &str, text: &str, sequence: u64) -> Result<(), YouTubeError>;
}

impl YouTubeCaptions {
    /// Üretim `youtube/v3` API tabanıyla kurar.
    pub fn new() -> Self {
        YouTubeCaptions {
            client: reqwest::Client::new(),
            api_base: YOUTUBE_API_BASE.to_string(),
        }
    }

    /// Test/self-host için özel API tabanıyla kurar.
    #[cfg(test)]
    fn with_api_base(api_base: impl Into<String>) -> Self {
        YouTubeCaptions {
            client: reqwest::Client::new(),
            api_base: api_base.into(),
        }
    }
}

#[async_trait]
impl CaptionsSink for YouTubeCaptions {
    /// `liveBroadcasts.list` ile `broadcast_id`'nin closed-caption ingestion
    /// URL'sini çözer. `access_token`, `integrations::oauth` ile bağlanan
    /// yayıncının (şifresi çözülmüş) YouTube OAuth token'ıdır.
    async fn fetch_ingestion_url(&self, access_token: &str, broadcast_id: &str) -> Result<String, YouTubeError> {
        let resp = self
            .client
            .get(format!("{}/liveBroadcasts", self.api_base))
            .bearer_auth(access_token)
            .query(&[("part", "contentDetails"), ("id", broadcast_id)])
            .send()
            .await
            .map_err(|e| YouTubeError::Request(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(YouTubeError::Request(format!(
                "youtube liveBroadcasts.list returned status {}",
                resp.status()
            )));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| YouTubeError::Request(e.to_string()))?;

        body["items"][0]["contentDetails"]["closedCaptionsIngestionUrl"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| YouTubeError::NoIngestionUrl(broadcast_id.to_string()))
    }

    /// Bir altyazı parçasını ingestion URL'ine gönderir. `sequence` artan bir
    /// sayaçtır — YouTube parçaları sıraya koymak için kullanır.
    async fn push_caption(&self, ingestion_url: &str, text: &str, sequence: u64) -> Result<(), YouTubeError> {
        let resp = self
            .client
            .post(ingestion_url)
            .query(&[("seq", sequence.to_string())])
            .header("Content-Type", "text/plain; charset=utf-8")
            .body(format!("{text}\n"))
            .send()
            .await
            .map_err(|e| YouTubeError::Request(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(YouTubeError::Request(format!(
                "youtube caption ingestion returned status {}",
                resp.status()
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::{Query, State};
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::{get, post};
    use axum::{Json, Router};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    async fn live_broadcasts(headers: HeaderMap, Query(params): Query<HashMap<String, String>>) -> (StatusCode, Json<serde_json::Value>) {
        if headers.get("authorization").and_then(|v| v.to_str().ok()) != Some("Bearer test-token") {
            return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "unauthorized" })));
        }
        match params.get("id").map(String::as_str) {
            Some("bc-live") => (
                StatusCode::OK,
                Json(serde_json::json!({
                    "items": [ { "contentDetails": { "closedCaptionsIngestionUrl": "http://ingest.example/cc?cid=abc" } } ]
                })),
            ),
            _ => (StatusCode::OK, Json(serde_json::json!({ "items": [] }))),
        }
    }

    #[derive(Clone, Default)]
    struct Captured(Arc<Mutex<Vec<(String, String)>>>);

    async fn ingest(
        State(captured): State<Captured>,
        Query(params): Query<HashMap<String, String>>,
        body: String,
    ) -> StatusCode {
        let seq = params.get("seq").cloned().unwrap_or_default();
        captured.0.lock().unwrap().push((seq, body));
        StatusCode::OK
    }

    async fn spawn_api_mock() -> String {
        let app = Router::new().route("/liveBroadcasts", get(live_broadcasts));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    async fn spawn_ingest_mock() -> (String, Captured) {
        let captured = Captured::default();
        let app = Router::new()
            .route("/cc", post(ingest))
            .with_state(captured.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (format!("http://{addr}/cc"), captured)
    }

    #[tokio::test]
    async fn resolves_ingestion_url_for_live_broadcast() {
        let base = spawn_api_mock().await;
        let yt = YouTubeCaptions::with_api_base(base);
        let url = yt.fetch_ingestion_url("test-token", "bc-live").await.unwrap();
        assert_eq!(url, "http://ingest.example/cc?cid=abc");
    }

    #[tokio::test]
    async fn missing_broadcast_has_no_ingestion_url() {
        let base = spawn_api_mock().await;
        let yt = YouTubeCaptions::with_api_base(base);
        let err = yt.fetch_ingestion_url("test-token", "bc-unknown").await.unwrap_err();
        assert!(matches!(err, YouTubeError::NoIngestionUrl(_)));
    }

    #[tokio::test]
    async fn bad_token_is_request_error() {
        let base = spawn_api_mock().await;
        let yt = YouTubeCaptions::with_api_base(base);
        let err = yt.fetch_ingestion_url("wrong-token", "bc-live").await.unwrap_err();
        assert!(matches!(err, YouTubeError::Request(_)));
    }

    #[tokio::test]
    async fn pushes_caption_with_sequence_number() {
        let (url, captured) = spawn_ingest_mock().await;
        let yt = YouTubeCaptions::new();
        yt.push_caption(&url, "merhaba dunya", 7).await.unwrap();
        let calls = captured.0.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "7");
        assert_eq!(calls[0].1, "merhaba dunya\n");
    }
}
