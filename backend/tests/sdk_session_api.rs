//! Canlı SDK cihaz presence'ı uçtan-uca testleri (`sdk_session` modülü).
//!
//! Akış: pairing claim → `sdk_session_id` + `heartbeat_secret` alınır → SDK
//! bunlarla imzalı heartbeat gönderir → panel `GET /api/sdk/devices` ile
//! canlı durumu görür → `DELETE /api/sdk/devices/{id}` ile cihazı kaldırır.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use sha2::Sha256;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use streamdubber_backend::{
    app,
    auth::jwt::{Claims, Jwt},
    config::ServiceConfig,
    gateway::{stt::MockStt, translate::MockTranslate, Gateway},
    state::AppState,
};

const SECRET: &[u8] = b"sdk-session-api-test-secret";
type HmacSha256 = Hmac<Sha256>;

async fn spawn_server() -> SocketAddr {
    let service_config = Arc::new(ServiceConfig::with_defaults());
    let mut gateway = Gateway::new(Arc::clone(&service_config));
    gateway.register_stt(Arc::new(MockStt::new("deepgram")));
    gateway.register_translate(Arc::new(MockTranslate::new("deepl")));

    let state = AppState::skeleton(Jwt::new(SECRET), Arc::clone(&service_config), gateway);

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        axum::serve(listener, app::router(state)).await.expect("serve");
    });
    addr
}

fn broadcaster_token(sub: &str) -> String {
    let jwt = Jwt::new(SECRET);
    let claims = Claims {
        sub: sub.into(),
        role: "broadcaster".into(),
        plan: "free".into(),
        exp: chrono::Utc::now().timestamp() + 3600,
    };
    jwt.issue(&claims).expect("issue token")
}

async fn http(addr: SocketAddr, request: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    stream.write_all(request.as_bytes()).await.expect("write");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.expect("read");
    let text = String::from_utf8_lossy(&buf).to_string();
    let status: u16 = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .expect("status line");
    let body = text.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    (status, body)
}

fn request(method: &str, path: &str, token: Option<&str>, json: Option<&str>) -> String {
    let auth = token
        .map(|t| format!("Authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    match json {
        Some(body) => format!(
            "{method} {path} HTTP/1.1\r\nHost: t\r\n{auth}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ),
        None => format!("{method} {path} HTTP/1.1\r\nHost: t\r\n{auth}Connection: close\r\n\r\n"),
    }
}

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// `sdk_session::compute_heartbeat_sig` ile birebir aynı hesap — SDK'nın
/// yapacağı imzalamayı taklit eder.
fn sign(secret_hex: &str, session_id: &str, ts: i64) -> String {
    let key = hex::decode(secret_hex).expect("hex secret");
    let mut mac = HmacSha256::new_from_slice(&key).expect("hmac key");
    mac.update(session_id.as_bytes());
    mac.update(b".");
    mac.update(ts.to_string().as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

async fn pair_device(addr: SocketAddr, user_sub: &str, device_name: &str) -> serde_json::Value {
    let user_token = broadcaster_token(user_sub);
    let (status, body) = http(
        addr,
        &request("POST", "/api/pairing/code", Some(&user_token), Some("{}")),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    let code = serde_json::from_str::<serde_json::Value>(&body).unwrap()["code"]
        .as_str()
        .unwrap()
        .to_string();

    let claim_body = format!(
        r#"{{"pairing_code":"{code}","device_name":"{device_name}","sdk_version":"0.1.0"}}"#
    );
    let (status, body) = http(
        addr,
        &request("POST", "/api/pairing/claim", None, Some(&claim_body)),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    serde_json::from_str(&body).unwrap()
}

#[tokio::test]
async fn heartbeat_then_device_list_shows_connected() {
    let addr = spawn_server().await;
    let claimed = pair_device(addr, "00000000-0000-0000-0000-00000000cccc", "Windows PC").await;
    let device_token = claimed["token"].as_str().unwrap();
    let session_id = claimed["sdk_session_id"].as_str().unwrap();
    let secret = claimed["heartbeat_secret"].as_str().unwrap();

    // Paired anında bile listede görünür ve bağlı sayılır (last_seen_at = paired_at).
    let (status, body) = http(
        addr,
        &request("GET", "/api/sdk/devices", Some(device_token), None),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    let devices: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(devices.as_array().unwrap().len(), 1);
    assert_eq!(devices[0]["device_name"], "Windows PC");
    assert_eq!(devices[0]["connected"], true);

    // Geçerli imzalı heartbeat kabul edilir.
    let ts = now_ts();
    let sig = sign(secret, session_id, ts);
    let hb_body = format!(r#"{{"session_id":"{session_id}","ts":{ts},"sig":"{sig}"}}"#);
    let (status, body) = http(
        addr,
        &request("POST", "/api/sdk/heartbeat", Some(device_token), Some(&hb_body)),
    )
    .await;
    assert_eq!(status, 204, "body: {body}");
}

#[tokio::test]
async fn heartbeat_rejects_bad_signature_and_stale_timestamp() {
    let addr = spawn_server().await;
    let claimed = pair_device(addr, "00000000-0000-0000-0000-00000000dddd", "Linux PC").await;
    let device_token = claimed["token"].as_str().unwrap();
    let session_id = claimed["sdk_session_id"].as_str().unwrap();
    let secret = claimed["heartbeat_secret"].as_str().unwrap();

    // Yanlış imza.
    let ts = now_ts();
    let bad_body = format!(r#"{{"session_id":"{session_id}","ts":{ts},"sig":"deadbeef"}}"#);
    let (status, _) = http(
        addr,
        &request("POST", "/api/sdk/heartbeat", Some(device_token), Some(&bad_body)),
    )
    .await;
    assert_eq!(status, 401);

    // Eski (geçerlilik penceresi dışı) zaman damgası — doğru imzayla bile reddedilir.
    let stale_ts = ts - 3600;
    let stale_sig = sign(secret, session_id, stale_ts);
    let stale_body = format!(
        r#"{{"session_id":"{session_id}","ts":{stale_ts},"sig":"{stale_sig}"}}"#
    );
    let (status, _) = http(
        addr,
        &request("POST", "/api/sdk/heartbeat", Some(device_token), Some(&stale_body)),
    )
    .await;
    assert_eq!(status, 401);

    // Başka bir kullanıcının device token'ıyla (farklı sub) bu session'a heartbeat atılamaz —
    // sahiplik kontrolü, session_id sızsa bile çapraz kullanıcı erişimini engeller.
    let other_token = broadcaster_token("00000000-0000-0000-0000-00000000eeee");
    let good_sig = sign(secret, session_id, ts);
    let good_body = format!(r#"{{"session_id":"{session_id}","ts":{ts},"sig":"{good_sig}"}}"#);
    let (status, _) = http(
        addr,
        &request("POST", "/api/sdk/heartbeat", Some(&other_token), Some(&good_body)),
    )
    .await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn devices_require_auth_and_are_isolated_per_user() {
    let addr = spawn_server().await;
    let claimed = pair_device(addr, "00000000-0000-0000-0000-00000000ffff", "Mac").await;
    let owner_token = broadcaster_token("00000000-0000-0000-0000-00000000ffff");

    let (status, _) = http(addr, &request("GET", "/api/sdk/devices", None, None)).await;
    assert_eq!(status, 401);

    let other_token = broadcaster_token("00000000-0000-0000-0000-00000000aaaa");
    let (status, body) = http(
        addr,
        &request("GET", "/api/sdk/devices", Some(&other_token), None),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(body.trim(), "[]", "other user must not see this device");

    let (status, body) = http(
        addr,
        &request("GET", "/api/sdk/devices", Some(&owner_token), None),
    )
    .await;
    assert_eq!(status, 200);
    let devices: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(devices.as_array().unwrap().len(), 1);
    let _ = claimed;
}

#[tokio::test]
async fn remove_device_stops_future_heartbeats() {
    let addr = spawn_server().await;
    let claimed = pair_device(addr, "00000000-0000-0000-0000-000000001111", "Stolen Laptop").await;
    let device_token = claimed["token"].as_str().unwrap();
    let session_id = claimed["sdk_session_id"].as_str().unwrap();
    let secret = claimed["heartbeat_secret"].as_str().unwrap();
    let owner_token = broadcaster_token("00000000-0000-0000-0000-000000001111");

    // Başka kullanıcı kaldıramaz.
    let other_token = broadcaster_token("00000000-0000-0000-0000-000000002222");
    let (status, _) = http(
        addr,
        &request(
            "DELETE",
            &format!("/api/sdk/devices/{session_id}"),
            Some(&other_token),
            None,
        ),
    )
    .await;
    assert_eq!(status, 404);

    // Sahibi kaldırabilir.
    let (status, _) = http(
        addr,
        &request(
            "DELETE",
            &format!("/api/sdk/devices/{session_id}"),
            Some(&owner_token),
            None,
        ),
    )
    .await;
    assert_eq!(status, 204);

    // Liste artık boş.
    let (_, body) = http(
        addr,
        &request("GET", "/api/sdk/devices", Some(&owner_token), None),
    )
    .await;
    assert_eq!(body.trim(), "[]");

    // Kaldırılan cihazın (eski) device token'ı ve geçerli imzasıyla bile heartbeat artık reddedilir.
    let ts = now_ts();
    let sig = sign(secret, session_id, ts);
    let hb_body = format!(r#"{{"session_id":"{session_id}","ts":{ts},"sig":"{sig}"}}"#);
    let (status, _) = http(
        addr,
        &request("POST", "/api/sdk/heartbeat", Some(device_token), Some(&hb_body)),
    )
    .await;
    assert_eq!(status, 404);
}
