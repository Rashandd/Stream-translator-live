//! Admin "self-hosted STT sunucuları" API uçtan-uca testleri (roadmap 2.6).
//!
//! Sözleşme: sunucu eklemek SSH keypair üretir + private key/token hiçbir
//! yanıtta düz metin dönmez; provisioning arka planda çalışır (`202`), panel
//! `GET .../{id}` ile durumu pollar; tüm uçlar sadece admin'e açık.

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
    stt_servers::provision::{FakeProvisioner, ProvisionError, ProvisionOutcome},
    stt_servers::SttServerStore,
};

const SECRET: &[u8] = b"stt-servers-api-test-secret";

async fn spawn_server(provisioner: Arc<dyn streamdubber_backend::stt_servers::provision::Provisioner>) -> SocketAddr {
    let service_config = Arc::new(ServiceConfig::with_defaults());
    let mut gateway = Gateway::new(Arc::clone(&service_config));
    gateway.register_stt(Arc::new(MockStt::new("deepgram")));
    gateway.register_translate(Arc::new(MockTranslate::new("deepl")));

    let mut state = AppState::skeleton(Jwt::new(SECRET), Arc::clone(&service_config), gateway);
    state.stt_servers = Arc::new(SttServerStore::in_memory());
    state.stt_provisioner = provisioner;
    let state = state;

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        axum::serve(listener, app::router(state)).await.expect("serve");
    });
    addr
}

fn token(role: &str) -> String {
    let jwt = Jwt::new(SECRET);
    let claims = Claims {
        sub: "00000000-0000-0000-0000-000000000042".into(),
        role: role.into(),
        plan: "pro".into(),
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

fn success_provisioner() -> Arc<dyn streamdubber_backend::stt_servers::provision::Provisioner> {
    Arc::new(FakeProvisioner::always(Ok(ProvisionOutcome {
        endpoint_url: "https://gpu-1:8443".into(),
        api_token: "super-secret-token-123456".into(),
        gpu_name: Some("RTX 3070".into()),
        vram_mb: Some(8192),
        host_fingerprint: Some("SHA256:fake-fingerprint-for-test".into()),
    })))
}

fn failing_provisioner() -> Arc<dyn streamdubber_backend::stt_servers::provision::Provisioner> {
    Arc::new(FakeProvisioner::always(Err(ProvisionError::Connection(
        "ssh timeout".into(),
    ))))
}

#[tokio::test]
async fn create_list_and_get_server() {
    let addr = spawn_server(success_provisioner()).await;
    let admin = token("admin");

    let (status, body) = http(
        addr,
        &request(
            "POST",
            "/admin/stt-servers",
            Some(&admin),
            Some(r#"{"name":"gpu-1","hostname":"1.2.3.4"}"#),
        ),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    let created: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(created["status"], "pending");
    assert_eq!(created["ssh_port"], 22);
    assert_eq!(created["ssh_user"], "root");
    assert!(created["ssh_public_key"].as_str().unwrap().starts_with("ssh-ed25519"));
    // Private key hiçbir zaman yanıtta yer almaz.
    assert!(!body.contains("ssh_private_key"));
    assert!(!body.contains("PRIVATE KEY"));

    let id = created["id"].as_str().unwrap().to_string();

    let (status, body) = http(addr, &request("GET", "/admin/stt-servers", Some(&admin), None)).await;
    assert_eq!(status, 200);
    let list: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(list.as_array().unwrap().len(), 1);

    let (status, body) = http(
        addr,
        &request("GET", &format!("/admin/stt-servers/{id}"), Some(&admin), None),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    let fetched: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(fetched["id"], id);
}

#[tokio::test]
async fn create_with_vast_ai_style_public_endpoint_override() {
    // Vast.ai: SSH portu ve STT servis portu host tarafından farklı NAT'lı
    // dış portlara eşlenebilir — admin bunu ekleme anında ayrıca girer.
    let addr = spawn_server(success_provisioner()).await;
    let admin = token("admin");

    let (status, body) = http(
        addr,
        &request(
            "POST",
            "/admin/stt-servers",
            Some(&admin),
            Some(
                r#"{"name":"vast-1","hostname":"142.214.185.187","ssh_port":20544,"public_endpoint_override":"http://142.214.185.187:41234"}"#,
            ),
        ),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    let created: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(created["ssh_port"], 20544);
    assert_eq!(
        created["public_endpoint_override"],
        "http://142.214.185.187:41234"
    );
}

#[tokio::test]
async fn provision_success_transitions_to_ready() {
    let addr = spawn_server(success_provisioner()).await;
    let admin = token("admin");

    let (_, body) = http(
        addr,
        &request(
            "POST",
            "/admin/stt-servers",
            Some(&admin),
            Some(r#"{"name":"gpu-1","hostname":"1.2.3.4"}"#),
        ),
    )
    .await;
    let id = serde_json::from_str::<serde_json::Value>(&body).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let (status, _) = http(
        addr,
        &request("POST", &format!("/admin/stt-servers/{id}/provision"), Some(&admin), None),
    )
    .await;
    assert_eq!(status, 202);

    // Arka plan görevi tamamlanana kadar kısa bekleme + poll (flaky olmayan
    // en fazla ~1s'lik best-effort döngü).
    let mut fetched = serde_json::json!({});
    for _ in 0..20 {
        let (_, body) = http(
            addr,
            &request("GET", &format!("/admin/stt-servers/{id}"), Some(&admin), None),
        )
        .await;
        fetched = serde_json::from_str(&body).expect("json");
        if fetched["status"] == "ready" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert_eq!(fetched["status"], "ready", "fetched: {fetched}");
    assert_eq!(fetched["endpoint_url"], "https://gpu-1:8443");
    assert_eq!(fetched["gpu_name"], "RTX 3070");
    assert_eq!(fetched["vram_mb"], 8192);
    // API token hiçbir yanıtta düz metin dönmez.
    assert!(!serde_json::to_string(&fetched).unwrap().contains("super-secret-token-123456"));
}

#[tokio::test]
async fn provision_failure_transitions_to_error_with_detail() {
    let addr = spawn_server(failing_provisioner()).await;
    let admin = token("admin");

    let (_, body) = http(
        addr,
        &request(
            "POST",
            "/admin/stt-servers",
            Some(&admin),
            Some(r#"{"name":"gpu-1","hostname":"1.2.3.4"}"#),
        ),
    )
    .await;
    let id = serde_json::from_str::<serde_json::Value>(&body).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let (status, _) = http(
        addr,
        &request("POST", &format!("/admin/stt-servers/{id}/provision"), Some(&admin), None),
    )
    .await;
    assert_eq!(status, 202);

    let mut fetched = serde_json::json!({});
    for _ in 0..20 {
        let (_, body) = http(
            addr,
            &request("GET", &format!("/admin/stt-servers/{id}"), Some(&admin), None),
        )
        .await;
        fetched = serde_json::from_str(&body).expect("json");
        if fetched["status"] == "error" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert_eq!(fetched["status"], "error", "fetched: {fetched}");
    assert!(fetched["status_detail"]
        .as_str()
        .unwrap()
        .contains("ssh timeout"));
}

#[tokio::test]
async fn delete_removes_server() {
    let addr = spawn_server(success_provisioner()).await;
    let admin = token("admin");

    let (_, body) = http(
        addr,
        &request(
            "POST",
            "/admin/stt-servers",
            Some(&admin),
            Some(r#"{"name":"gpu-1","hostname":"1.2.3.4"}"#),
        ),
    )
    .await;
    let id = serde_json::from_str::<serde_json::Value>(&body).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let (status, _) = http(
        addr,
        &request("DELETE", &format!("/admin/stt-servers/{id}"), Some(&admin), None),
    )
    .await;
    assert_eq!(status, 204);

    let (status, _) = http(
        addr,
        &request("GET", &format!("/admin/stt-servers/{id}"), Some(&admin), None),
    )
    .await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn stt_servers_api_requires_admin() {
    let addr = spawn_server(success_provisioner()).await;

    let (status, _) = http(addr, &request("GET", "/admin/stt-servers", None, None)).await;
    assert_eq!(status, 401);

    let (status, _) = http(
        addr,
        &request("GET", "/admin/stt-servers", Some(&token("broadcaster")), None),
    )
    .await;
    assert_eq!(status, 403);

    let (status, _) = http(
        addr,
        &request(
            "POST",
            "/admin/stt-servers",
            Some(&token("broadcaster")),
            Some(r#"{"name":"gpu-1","hostname":"1.2.3.4"}"#),
        ),
    )
    .await;
    assert_eq!(status, 403);
}
