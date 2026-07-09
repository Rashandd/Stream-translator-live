//! Ses klonlama + admin onay kuyruğu uçtan-uca testleri (Faz 2.1).
//!
//! Akış: yayıncı (pro plan, has_voice_clone) örnek yollar → model
//! `pending_review` ile oluşur → admin onaylar → yayıncının kendi listesi
//! güncel durumu yansıtır. Plan gating (free reddedilir) ve tts.provider
//! "disabled" varsayılanı da ayrıca doğrulanır.

use std::net::SocketAddr;
use std::sync::Arc;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use streamdubber_backend::{
    app,
    auth::jwt::{Claims, Jwt},
    config::ServiceConfig,
    gateway::{stt::MockStt, translate::MockTranslate, tts::MockTts, Gateway},
    state::AppState,
};

const SECRET: &[u8] = b"voice-models-api-test-secret";

async fn spawn_server(with_tts: bool) -> (SocketAddr, Arc<ServiceConfig>) {
    let service_config = Arc::new(ServiceConfig::with_defaults());
    let mut gateway = Gateway::new(Arc::clone(&service_config));
    gateway.register_stt(Arc::new(MockStt::new("deepgram")));
    gateway.register_translate(Arc::new(MockTranslate::new("deepl")));
    if with_tts {
        gateway.register_tts(Arc::new(MockTts::new("elevenlabs")));
        service_config.set("tts.provider", "elevenlabs");
    }

    let state = AppState::skeleton(Jwt::new(SECRET), Arc::clone(&service_config), gateway);

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        axum::serve(listener, app::router(state)).await.expect("serve");
    });
    (addr, service_config)
}

fn token(sub: &str, role: &str, plan: &str) -> String {
    let jwt = Jwt::new(SECRET);
    jwt.issue(&Claims {
        sub: sub.into(),
        role: role.into(),
        plan: plan.into(),
        exp: chrono::Utc::now().timestamp() + 3600,
    })
    .expect("issue token")
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

fn clone_request_body(name: &str, sample: &[u8]) -> String {
    format!(
        r#"{{"name":"{name}","sample_base64":"{}","language_mode":"single_to_multi","source_languages":["tr"],"consent_text_version":"v1","is_own_voice":true}}"#,
        B64.encode(sample)
    )
}

#[tokio::test]
async fn full_clone_and_admin_approval_flow() {
    let (addr, _cfg) = spawn_server(true).await;
    let pro = token("00000000-0000-0000-0000-0000000000p1", "broadcaster", "pro");
    let admin = token("admin-1", "admin", "streamer");

    // 1) Yayıncı örnek yollar — model pending_review ile oluşur.
    let (status, body) = http(
        addr,
        &request(
            "POST",
            "/api/voice/clone",
            Some(&pro),
            Some(&clone_request_body("My Voice", b"fake wav bytes")),
        ),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    let model: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(model["status"], "pending_review");
    assert_eq!(model["name"], "My Voice");
    let model_id = model["id"].as_str().expect("id").to_string();

    // 2) Yayıncı kendi listesinde görür.
    let (status, body) = http(addr, &request("GET", "/api/voice/models", Some(&pro), None)).await;
    assert_eq!(status, 200);
    let mine: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(mine.as_array().unwrap().len(), 1);
    assert_eq!(mine[0]["id"], model_id);

    // 3) Admin onay kuyruğunda görür.
    let (status, body) = http(addr, &request("GET", "/admin/voice/models", Some(&admin), None)).await;
    assert_eq!(status, 200);
    let queue: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(queue.as_array().unwrap().len(), 1);
    assert_eq!(queue[0]["status"], "pending_review");

    // 4) Admin onaylar.
    let (status, body) = http(
        addr,
        &request(
            "POST",
            &format!("/admin/voice/models/{model_id}/approve"),
            Some(&admin),
            Some("{}"),
        ),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    let approved: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(approved["status"], "approved");

    // 5) Yayıncının kendi listesi de güncel durumu yansıtır.
    let (_, body) = http(addr, &request("GET", "/api/voice/models", Some(&pro), None)).await;
    let mine: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(mine[0]["status"], "approved");
}

#[tokio::test]
async fn free_plan_is_rejected_before_calling_the_provider() {
    let (addr, _cfg) = spawn_server(true).await;
    let free = token("00000000-0000-0000-0000-0000000000f1", "broadcaster", "free");

    let (status, body) = http(
        addr,
        &request(
            "POST",
            "/api/voice/clone",
            Some(&free),
            Some(&clone_request_body("x", b"sample")),
        ),
    )
    .await;
    assert_eq!(status, 403, "body: {body}");
}

#[tokio::test]
async fn disabled_tts_provider_surfaces_as_bad_gateway() {
    // spawn_server(false): tts.provider config seed'i "disabled" kalır.
    let (addr, _cfg) = spawn_server(false).await;
    let pro = token("00000000-0000-0000-0000-0000000000p2", "broadcaster", "pro");

    let (status, body) = http(
        addr,
        &request(
            "POST",
            "/api/voice/clone",
            Some(&pro),
            Some(&clone_request_body("x", b"sample")),
        ),
    )
    .await;
    assert_eq!(status, 502, "body: {body}");
}

#[tokio::test]
async fn empty_sample_and_bad_base64_are_rejected() {
    let (addr, _cfg) = spawn_server(true).await;
    let pro = token("00000000-0000-0000-0000-0000000000p3", "broadcaster", "pro");

    let (status, _) = http(
        addr,
        &request(
            "POST",
            "/api/voice/clone",
            Some(&pro),
            Some(r#"{"name":"x","sample_base64":"","consent_text_version":"v1","is_own_voice":true}"#),
        ),
    )
    .await;
    assert_eq!(status, 422);

    let (status, _) = http(
        addr,
        &request(
            "POST",
            "/api/voice/clone",
            Some(&pro),
            Some(r#"{"name":"x","sample_base64":"not-valid-base64!!!","consent_text_version":"v1","is_own_voice":true}"#),
        ),
    )
    .await;
    assert_eq!(status, 422);
}

#[tokio::test]
async fn missing_or_false_consent_is_rejected_before_calling_the_provider() {
    let (addr, _cfg) = spawn_server(true).await;
    let pro = token("00000000-0000-0000-0000-0000000000c1", "broadcaster", "pro");

    // Boş consent_text_version reddedilir.
    let (status, _) = http(
        addr,
        &request(
            "POST",
            "/api/voice/clone",
            Some(&pro),
            Some(&format!(
                r#"{{"name":"x","sample_base64":"{}","language_mode":"single_to_multi","source_languages":["tr"],"consent_text_version":"","is_own_voice":true}}"#,
                B64.encode(b"sample")
            )),
        ),
    )
    .await;
    assert_eq!(status, 422);

    // is_own_voice=false reddedilir.
    let (status, _) = http(
        addr,
        &request(
            "POST",
            "/api/voice/clone",
            Some(&pro),
            Some(&format!(
                r#"{{"name":"x","sample_base64":"{}","language_mode":"single_to_multi","source_languages":["tr"],"consent_text_version":"v1","is_own_voice":false}}"#,
                B64.encode(b"sample")
            )),
        ),
    )
    .await;
    assert_eq!(status, 422);

    // İkisi de geçerliyse kabul edilir ve rıza kaydı modelde görünür.
    let (status, body) = http(
        addr,
        &request(
            "POST",
            "/api/voice/clone",
            Some(&pro),
            Some(&clone_request_body("consented voice", b"sample")),
        ),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    let model: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(model["consent"]["text_version"], "v1");
    assert_eq!(model["consent"]["is_own_voice"], true);
}

#[tokio::test]
async fn voice_clone_requires_auth_and_admin_routes_reject_broadcasters() {
    let (addr, _cfg) = spawn_server(true).await;
    let pro = token("00000000-0000-0000-0000-0000000000p4", "broadcaster", "pro");

    let (status, _) = http(
        addr,
        &request("POST", "/api/voice/clone", None, Some(&clone_request_body("x", b"s"))),
    )
    .await;
    assert_eq!(status, 401);

    let (status, _) = http(addr, &request("GET", "/admin/voice/models", None, None)).await;
    assert_eq!(status, 401);

    let (status, _) = http(addr, &request("GET", "/admin/voice/models", Some(&pro), None)).await;
    assert_eq!(status, 403);
}

#[tokio::test]
async fn users_do_not_see_each_others_voice_models() {
    let (addr, _cfg) = spawn_server(true).await;
    let user_a = token("00000000-0000-0000-0000-0000000000aa", "broadcaster", "pro");
    let user_b = token("00000000-0000-0000-0000-0000000000bb", "broadcaster", "pro");

    let (status, _) = http(
        addr,
        &request(
            "POST",
            "/api/voice/clone",
            Some(&user_a),
            Some(&clone_request_body("A's voice", b"sample-a")),
        ),
    )
    .await;
    assert_eq!(status, 200);

    let (_, body) = http(addr, &request("GET", "/api/voice/models", Some(&user_b), None)).await;
    let list: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert!(list.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn language_mode_source_language_count_is_validated() {
    let (addr, _cfg) = spawn_server(true).await;
    let pro = token("00000000-0000-0000-0000-0000000000p5", "broadcaster", "pro");

    // single_to_single iki kaynak dille reddedilir.
    let (status, _) = http(
        addr,
        &request(
            "POST",
            "/api/voice/clone",
            Some(&pro),
            Some(&format!(
                r#"{{"name":"x","sample_base64":"{}","language_mode":"single_to_single","source_languages":["tr","en"],"consent_text_version":"v1","is_own_voice":true}}"#,
                B64.encode(b"sample")
            )),
        ),
    )
    .await;
    assert_eq!(status, 422);

    // multi_to_single tek kaynak dille reddedilir.
    let (status, _) = http(
        addr,
        &request(
            "POST",
            "/api/voice/clone",
            Some(&pro),
            Some(&format!(
                r#"{{"name":"x","sample_base64":"{}","language_mode":"multi_to_single","source_languages":["tr"],"consent_text_version":"v1","is_own_voice":true}}"#,
                B64.encode(b"sample")
            )),
        ),
    )
    .await;
    assert_eq!(status, 422);

    // Geçerli kombinasyon kabul edilir.
    let (status, _) = http(
        addr,
        &request(
            "POST",
            "/api/voice/clone",
            Some(&pro),
            Some(&format!(
                r#"{{"name":"x","sample_base64":"{}","language_mode":"multi_to_single","source_languages":["tr","en"],"consent_text_version":"v1","is_own_voice":true}}"#,
                B64.encode(b"sample")
            )),
        ),
    )
    .await;
    assert_eq!(status, 200);
}

#[tokio::test]
async fn admin_assign_activates_model_for_owner_and_language() {
    let (addr, _cfg) = spawn_server(true).await;
    let pro = token("00000000-0000-0000-0000-0000000000p6", "broadcaster", "pro");
    let admin = token("admin-2", "admin", "streamer");

    let (_, body) = http(
        addr,
        &request(
            "POST",
            "/api/voice/clone",
            Some(&pro),
            Some(&clone_request_body("My Voice", b"sample")),
        ),
    )
    .await;
    let model: serde_json::Value = serde_json::from_str(&body).expect("json");
    let model_id = model["id"].as_str().expect("id").to_string();

    // Onaylanmadan atama reddedilir.
    let (status, _) = http(
        addr,
        &request(
            "POST",
            &format!("/admin/voice/models/{model_id}/assign"),
            Some(&admin),
            Some(r#"{"target_language":"en"}"#),
        ),
    )
    .await;
    assert_eq!(status, 422);

    http(
        addr,
        &request(
            "POST",
            &format!("/admin/voice/models/{model_id}/approve"),
            Some(&admin),
            Some("{}"),
        ),
    )
    .await;

    let (status, body) = http(
        addr,
        &request(
            "POST",
            &format!("/admin/voice/models/{model_id}/assign"),
            Some(&admin),
            Some(r#"{"target_language":"en"}"#),
        ),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    let assigned: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(assigned["target_languages"], serde_json::json!(["en"]));

    // Broadcaster admin uç noktasına erişemez.
    let (status, _) = http(
        addr,
        &request(
            "POST",
            &format!("/admin/voice/models/{model_id}/assign"),
            Some(&pro),
            Some(r#"{"target_language":"de"}"#),
        ),
    )
    .await;
    assert_eq!(status, 403);
}
