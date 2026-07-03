//! Backend'den relay hedeflerini çeker ve SDK'nın RTMP relay motoruna
//! aktarılabilecek [`RelayTarget`] biçimine çevirir.
//!
//! Backend'in `{platform, rtmp_url, stream_key}` modeli (bkz.
//! `backend/src/relay/mod.rs`) SDK'nın `host/port/app/tc_url` çözümlenmiş
//! modelinden farklı — bu modül aradaki dönüşümü yapar. Kimlik doğrulama,
//! pairing'den alınan cihaz JWT'siyle aynıdır (`Authorization: Bearer`).

use serde::Deserialize;

use super::rtmp::{RelayTarget, TargetError};

/// Backend'in `GET /api/relay/targets` yanıtındaki tek bir kayıt.
#[derive(Debug, Deserialize)]
struct BackendRelayTarget {
    platform: String,
    rtmp_url: String,
    stream_key: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RelaySyncError {
    #[error("http request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("server rejected the request: {0}")]
    Server(reqwest::StatusCode),
    #[error("invalid target from server: {0}")]
    InvalidTarget(#[from] TargetError),
}

/// `GET {base_url}/api/relay/targets` — cihaz JWT'siyle çağrılır, backend'in
/// düz metin hedef listesini SDK'nın relay motoruna verilebilecek
/// [`RelayTarget`] listesine çevirir.
pub async fn fetch_relay_targets(
    http: &reqwest::Client,
    base_url: &str,
    device_token: &str,
) -> Result<Vec<RelayTarget>, RelaySyncError> {
    let url = format!("{}/api/relay/targets", base_url.trim_end_matches('/'));
    let response = http.get(&url).bearer_auth(device_token).send().await?;
    if !response.status().is_success() {
        return Err(RelaySyncError::Server(response.status()));
    }
    let raw: Vec<BackendRelayTarget> = response.json().await?;
    raw.into_iter()
        .enumerate()
        .map(|(i, t)| {
            RelayTarget::parse(format!("target-{i}"), t.platform.clone(), &t.rtmp_url, &t.stream_key)
                .map_err(RelaySyncError::from)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::get;
    use axum::{Json, Router};

    async fn spawn_mock(body: serde_json::Value, status: axum::http::StatusCode) -> String {
        async fn handler(
            axum::extract::State((body, status)): axum::extract::State<(serde_json::Value, axum::http::StatusCode)>,
        ) -> (axum::http::StatusCode, Json<serde_json::Value>) {
            (status, Json(body))
        }
        let app = Router::new()
            .route("/api/relay/targets", get(handler))
            .with_state((body, status));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn parses_backend_targets_into_relay_targets() {
        let base = spawn_mock(
            serde_json::json!([
                {"platform": "youtube", "rtmp_url": "rtmp://a.rtmp.youtube.com/live2", "stream_key": "yt-key"},
                {"platform": "twitch", "rtmp_url": "rtmp://live.twitch.tv:1935/app", "stream_key": "tw-key"},
            ]),
            axum::http::StatusCode::OK,
        )
        .await;

        let http = reqwest::Client::new();
        let targets = fetch_relay_targets(&http, &base, "device-token").await.expect("fetch");
        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0].label, "youtube");
        assert_eq!(targets[0].stream_key, "yt-key");
        assert_eq!(targets[0].port, 1935);
        assert_eq!(targets[1].host, "live.twitch.tv");
    }

    #[tokio::test]
    async fn server_error_status_is_reported() {
        let base = spawn_mock(serde_json::json!({"error": "unauthorized"}), axum::http::StatusCode::UNAUTHORIZED).await;
        let http = reqwest::Client::new();
        let err = fetch_relay_targets(&http, &base, "bad-token").await.unwrap_err();
        match err {
            RelaySyncError::Server(status) => assert_eq!(status.as_u16(), 401),
            other => panic!("expected Server error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn invalid_target_from_server_is_rejected() {
        let base = spawn_mock(
            serde_json::json!([{"platform": "youtube", "rtmp_url": "http://not-rtmp.example", "stream_key": "k"}]),
            axum::http::StatusCode::OK,
        )
        .await;
        let http = reqwest::Client::new();
        let err = fetch_relay_targets(&http, &base, "token").await.unwrap_err();
        assert!(matches!(err, RelaySyncError::InvalidTarget(_)));
    }
}
