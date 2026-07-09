//! İmzalı canlılık sinyali (heartbeat) — backend'in `sdk_session` modülüyle
//! birebir eşleşen taraf.
//!
//! Pairing claim yanıtından gelen `sdk_session_id` + `heartbeat_secret`
//! (hex) kullanılarak her tur `HMAC-SHA256(secret, "{session_id}.{ts}")`
//! hesaplanır ve `POST /api/sdk/heartbeat`e (device JWT auth) gönderilir.
//! Backend, `ts`'i şimdiki zamana yakın (±60sn) ve imzayı doğru bulmazsa
//! reddeder — bu, panelin "bağlı" göstergesinin sahte üretilememesini
//! (secret'ı bilmeyen biri imza üretemez) ve eski bir isteğin tekrar
//! oynatılamamasını (replay — `ts` penceresi dışına düşer) sağlar.

use hmac::{Hmac, Mac};
use serde::Serialize;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, thiserror::Error)]
pub enum HeartbeatError {
    #[error("http request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("server rejected the heartbeat: {0}")]
    Server(reqwest::StatusCode),
}

fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_secs() as i64
}

/// Backend'in `sdk_session::compute_heartbeat_sig`'iyle birebir aynı hesap.
fn sign(secret_hex: &str, session_id: &str, ts: i64) -> Option<String> {
    let key = hex::decode(secret_hex).ok()?;
    let mut mac = HmacSha256::new_from_slice(&key).ok()?;
    mac.update(session_id.as_bytes());
    mac.update(b".");
    mac.update(ts.to_string().as_bytes());
    Some(hex::encode(mac.finalize().into_bytes()))
}

#[derive(Debug, Serialize)]
struct HeartbeatRequest<'a> {
    session_id: &'a str,
    ts: i64,
    sig: String,
}

/// Tek bir imzalı heartbeat gönderir.
pub async fn send_heartbeat(
    http: &reqwest::Client,
    base_url: &str,
    device_token: &str,
    session_id: &str,
    heartbeat_secret: &str,
) -> Result<(), HeartbeatError> {
    let ts = now_ts();
    let sig = sign(heartbeat_secret, session_id, ts)
        .expect("heartbeat secret is server-issued hex, always valid");

    let response = http
        .post(format!(
            "{}/api/sdk/heartbeat",
            base_url.trim_end_matches('/')
        ))
        .bearer_auth(device_token)
        .json(&HeartbeatRequest { session_id, ts, sig })
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(HeartbeatError::Server(response.status()));
    }
    Ok(())
}

/// [`send_heartbeat`]'i sabit aralıkla, görev iptal/drop edilene kadar
/// koşturur (bkz. `stt_control::run_polling_loop` — aynı desen: bir
/// heartbeat başarısızlığı (ağ kopması, backend restart) sadece loglanır ve
/// yutulur, tray ajanını asla düşürmez, bir sonraki turda tekrar dener).
pub async fn run_heartbeat_loop(
    http: reqwest::Client,
    base_url: String,
    device_token: String,
    session_id: String,
    heartbeat_secret: String,
    interval: std::time::Duration,
) {
    let mut ticker = tokio::time::interval(interval);
    loop {
        ticker.tick().await;
        if let Err(err) = send_heartbeat(&http, &base_url, &device_token, &session_id, &heartbeat_secret).await
        {
            tracing::warn!(%err, "heartbeat failed, will retry next tick");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{extract::State, routing::post, Json, Router};
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Captured(Arc<Mutex<Vec<serde_json::Value>>>);

    async fn capture(
        State(store): State<Captured>,
        Json(body): Json<serde_json::Value>,
    ) -> axum::http::StatusCode {
        store.0.lock().unwrap().push(body);
        axum::http::StatusCode::NO_CONTENT
    }

    async fn spawn_capture_server() -> (std::net::SocketAddr, Captured) {
        let store = Captured::default();
        let app = Router::new()
            .route("/api/sdk/heartbeat", post(capture))
            .with_state(store.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (addr, store)
    }

    #[test]
    fn sign_is_deterministic_and_context_bound() {
        let secret = hex::encode([7u8; 32]);
        let a = sign(&secret, "sess-1", 1000).unwrap();
        let b = sign(&secret, "sess-1", 1000).unwrap();
        let c = sign(&secret, "sess-2", 1000).unwrap();
        let d = sign(&secret, "sess-1", 1001).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
    }

    #[tokio::test]
    async fn send_heartbeat_posts_signed_payload() {
        let (addr, store) = spawn_capture_server().await;
        let http = reqwest::Client::new();
        let secret = hex::encode([9u8; 32]);

        send_heartbeat(
            &http,
            &format!("http://{addr}"),
            "device-jwt",
            "sess-abc",
            &secret,
        )
        .await
        .expect("heartbeat ok");

        let captured = store.0.lock().unwrap();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0]["session_id"], "sess-abc");
        assert!(captured[0]["ts"].as_i64().is_some());
        let expected = sign(&secret, "sess-abc", captured[0]["ts"].as_i64().unwrap()).unwrap();
        assert_eq!(captured[0]["sig"], expected);
    }
}
