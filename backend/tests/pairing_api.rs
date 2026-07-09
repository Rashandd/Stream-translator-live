//! Pairing + relay hedefleri uçtan-uca testleri.
//!
//! Akış: panel kod üretir (auth) → SDK kodu claim eder (auth'suz, kod
//! capability) → aldığı token'la relay hedeflerini çeker. Stream key'ler
//! yanıt gövdesinde (TLS) döner ama loglara asla düşmez.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use streamdubber_backend::{
    app,
    auth::jwt::{Claims, Jwt},
    config::ServiceConfig,
    gateway::{stt::MockStt, translate::MockTranslate, Gateway},
    state::AppState,
};

const SECRET: &[u8] = b"pairing-api-test-secret";

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

fn broadcaster_token() -> String {
    let jwt = Jwt::new(SECRET);
    let claims = Claims {
        sub: "00000000-0000-0000-0000-00000000aaaa".into(),
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

#[tokio::test]
async fn full_pairing_flow_yields_working_sdk_token() {
    let addr = spawn_server().await;

    // 1) Panel kod üretir (auth zorunlu).
    let (status, _) = http(addr, &request("POST", "/api/pairing/code", None, Some("{}"))).await;
    assert_eq!(status, 401);

    let (status, body) = http(
        addr,
        &request("POST", "/api/pairing/code", Some(&broadcaster_token()), Some("{}")),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("json");
    let code = parsed["code"].as_str().expect("code").to_string();
    assert_eq!(code.len(), 6);
    assert!(parsed["expires_in_secs"].as_u64().unwrap() > 0);

    // 2) SDK claim eder (auth başlığı yok — kod capability'dir).
    let claim_body = format!(
        r#"{{"pairing_code":"{code}","device_name":"Test PC","sdk_version":"0.1.0"}}"#
    );
    let (status, body) = http(
        addr,
        &request("POST", "/api/pairing/claim", None, Some(&claim_body)),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("json");
    let sdk_token = parsed["token"].as_str().expect("token").to_string();
    assert_eq!(parsed["user_id"], "00000000-0000-0000-0000-00000000aaaa");

    // Token gerçekten yayıncı token'ı: claims doğru + korumalı uca erişiyor.
    let claims = Jwt::new(SECRET).verify(&sdk_token).expect("valid");
    assert_eq!(claims.role, "broadcaster");
    assert_eq!(claims.sub, "00000000-0000-0000-0000-00000000aaaa");

    // 3) Kod tek kullanımlık: ikinci claim 404.
    let (status, _) = http(
        addr,
        &request("POST", "/api/pairing/claim", None, Some(&claim_body)),
    )
    .await;
    assert_eq!(status, 404);

    // 4) Panel hedefleri yazar, SDK kendi token'ıyla aynı listeyi okur.
    // (youtube relay hedefi olarak YASAK — bkz. relay_targets_reject_youtube; burada
    // twitch + kick kullanılıyor, tıpkı gerçek bir çift-platform relay kurulumu gibi.)
    let targets = r#"{"targets":[
        {"platform":"twitch","rtmp_url":"rtmps://ingest.twitch.tv/app","stream_key":"tw-key-456"},
        {"platform":"kick","rtmp_url":"rtmp://ingest.kick.example/live","stream_key":"kick-key-789"}
    ]}"#;
    let (status, body) = http(
        addr,
        &request("PUT", "/api/relay/targets", Some(&broadcaster_token()), Some(targets)),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");

    let (status, body) = http(
        addr,
        &request("GET", "/api/relay/targets", Some(&sdk_token), None),
    )
    .await;
    assert_eq!(status, 200);
    let list: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(list.as_array().unwrap().len(), 2);
    assert_eq!(list[0]["stream_key"], "tw-key-456");
    assert_eq!(list[1]["rtmp_url"], "rtmp://ingest.kick.example/live");

    // 5) Stream key hiçbir log kaydına düşmemiş olmalı (redaction + adet loglama).
    let admin = {
        let jwt = Jwt::new(SECRET);
        jwt.issue(&Claims {
            sub: "admin-1".into(),
            role: "admin".into(),
            plan: "streamer".into(),
            exp: chrono::Utc::now().timestamp() + 3600,
        })
        .unwrap()
    };
    let (_, logs) = http(addr, &request("GET", "/admin/logs", Some(&admin), None)).await;
    assert!(!logs.contains("tw-key-456"), "stream key leaked to logs: {logs}");
    assert!(!logs.contains("kick-key-789"));
}

#[tokio::test]
async fn claim_rejects_bad_and_unknown_codes() {
    let addr = spawn_server().await;

    // Format hatası → 400.
    let (status, _) = http(
        addr,
        &request(
            "POST",
            "/api/pairing/claim",
            None,
            Some(r#"{"pairing_code":"12ab56","device_name":"PC"}"#),
        ),
    )
    .await;
    assert_eq!(status, 400);

    // Bilinmeyen kod → 404.
    let (status, _) = http(
        addr,
        &request(
            "POST",
            "/api/pairing/claim",
            None,
            Some(r#"{"pairing_code":"000000","device_name":"PC"}"#),
        ),
    )
    .await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn claim_is_rate_limited_per_client() {
    let addr = spawn_server().await;

    // Aynı istemciden (forwarded IP sabit) art arda kod yokmuş gibi denemeler:
    // limit dolduktan sonra 429 dönmeli — bu, auth'suz `claim` ucuna karşı
    // 6 haneli kodun brute-force edilmesini engelliyor.
    let bad_claim = |code: &str| {
        format!(r#"{{"pairing_code":"{code}","device_name":"Attacker"}}"#)
    };

    let mut saw_429 = false;
    for i in 0..30u32 {
        let code = format!("{:06}", i);
        let (status, _) = http(
            addr,
            &format!(
                "POST /api/pairing/claim HTTP/1.1\r\nHost: t\r\nX-Forwarded-For: 9.9.9.9\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                bad_claim(&code).len(),
                bad_claim(&code)
            ),
        )
        .await;
        if status == 429 {
            saw_429 = true;
            break;
        }
        assert_eq!(status, 404, "expected unknown-code 404 before limit kicks in");
    }
    assert!(saw_429, "expected rate limiter to eventually return 429");

    // Farklı istemci (farklı IP) hâlâ isteğini yapabilmeli.
    let (status, _) = http(
        addr,
        &format!(
            "POST /api/pairing/claim HTTP/1.1\r\nHost: t\r\nX-Forwarded-For: 1.1.1.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            bad_claim("000000").len(),
            bad_claim("000000")
        ),
    )
    .await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn relay_targets_require_auth_and_validation() {
    let addr = spawn_server().await;

    let (status, _) = http(addr, &request("GET", "/api/relay/targets", None, None)).await;
    assert_eq!(status, 401);

    // Geçersiz hedef (http URL) → 422.
    let bad = r#"{"targets":[{"platform":"kick","rtmp_url":"http://x","stream_key":"k"}]}"#;
    let (status, _) = http(
        addr,
        &request("PUT", "/api/relay/targets", Some(&broadcaster_token()), Some(bad)),
    )
    .await;
    assert_eq!(status, 422);

    // Kullanıcılar birbirinin hedefini görmez.
    let ok = r#"{"targets":[{"platform":"kick","rtmp_url":"rtmp://kick.example/live","stream_key":"kk"}]}"#;
    let (status, _) = http(
        addr,
        &request("PUT", "/api/relay/targets", Some(&broadcaster_token()), Some(ok)),
    )
    .await;
    assert_eq!(status, 200);

    let other = {
        let jwt = Jwt::new(SECRET);
        jwt.issue(&Claims {
            sub: "00000000-0000-0000-0000-00000000bbbb".into(),
            role: "broadcaster".into(),
            plan: "free".into(),
            exp: chrono::Utc::now().timestamp() + 3600,
        })
        .unwrap()
    };
    let (status, body) = http(addr, &request("GET", "/api/relay/targets", Some(&other), None)).await;
    assert_eq!(status, 200);
    assert_eq!(body.trim(), "[]");
}

#[tokio::test]
async fn relay_targets_reject_youtube() {
    let addr = spawn_server().await;

    // YouTube zaten OBS'ten doğrudan yayın alır — relay hedefi olarak reddedilmeli
    // (native captions/multi-audio OAuth entegrasyonundan gelir, RTMP relay'den değil).
    let youtube = r#"{"targets":[{"platform":"youtube","rtmp_url":"rtmp://a.rtmp.youtube.com/live2","stream_key":"k"}]}"#;
    let (status, body) = http(
        addr,
        &request("PUT", "/api/relay/targets", Some(&broadcaster_token()), Some(youtube)),
    )
    .await;
    assert_eq!(status, 422, "body: {body}");
    assert!(body.to_lowercase().contains("youtube"), "body: {body}");
}
