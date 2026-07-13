//! Config API uçtan-uca testleri — admin yazar, yayıncı okur, yetkisiz reddedilir.
//!
//! `service_config` runtime store'unun HTTP üzerinden değiştirilebildiğini ve
//! değişikliğin (aynı süreç içinde) anında okunabildiğini doğrular.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use streamdubber_backend::{
    app,
    auth::jwt::{Claims, Jwt},
    config::{Environment, ServiceConfig},
    gateway::{stt::MockStt, translate::MockTranslate, Gateway},
    state::AppState,
};

const SECRET: &[u8] = b"config-api-test-secret";

async fn spawn_server() -> SocketAddr {
    spawn_server_full(None, Environment::Development).await
}

/// `spawn_server` ile aynı, ama DB pool'u (varsa) state'e takar — `service_config`
/// tablosuna gerçek yazım/okuma gerektiren testler için.
async fn spawn_server_with_db(db: Option<sqlx::PgPool>) -> SocketAddr {
    spawn_server_full(db, Environment::Development).await
}

/// BH-02 production kısıtlarını (insecure URL, mock fallback) tetiklemek için
/// `Environment::Production` ile kurulmuş sunucu.
async fn spawn_server_production() -> SocketAddr {
    spawn_server_full(None, Environment::Production).await
}

async fn spawn_server_full(db: Option<sqlx::PgPool>, environment: Environment) -> SocketAddr {
    let service_config = Arc::new(ServiceConfig::with_defaults());
    let mut gateway = Gateway::new(Arc::clone(&service_config));
    gateway.register_stt(Arc::new(MockStt::new("deepgram")));
    gateway.register_translate(Arc::new(MockTranslate::new("deepl")));

    let mut state = AppState::skeleton(Jwt::new(SECRET), Arc::clone(&service_config), gateway);
    state.db = db;
    state.environment = environment;

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        axum::serve(listener, app::router(state))
            .await
            .expect("serve");
    });
    addr
}

fn token(role: &str) -> String {
    let jwt = Jwt::new(SECRET);
    let claims = Claims {
        sub: "00000000-0000-0000-0000-000000000001".into(),
        role: role.into(),
        plan: "pro".into(),
        exp: chrono::Utc::now().timestamp() + 3600,
    };
    jwt.issue(&claims).expect("issue token")
}

/// Tek bir HTTP isteği gönderir, (status, body) döndürür. `Connection: close`
/// sayesinde `read_to_end` yanıt bitince tamamlanır.
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

fn get(path: &str) -> String {
    format!("GET {path} HTTP/1.1\r\nHost: t\r\nConnection: close\r\n\r\n")
}

/// `get` ile aynı, ama Bearer token taşır — admin-only okuma uçları için.
fn get_auth(path: &str, token: &str) -> String {
    format!(
        "GET {path} HTTP/1.1\r\nHost: t\r\nAuthorization: Bearer {token}\r\nConnection: close\r\n\r\n"
    )
}

fn put_config(token: Option<&str>, json: &str) -> String {
    let auth = token
        .map(|t| format!("Authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    format!(
        "PUT /admin/config HTTP/1.1\r\nHost: t\r\n{auth}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
        json.len()
    )
}

#[tokio::test]
async fn public_get_returns_only_allowlisted_config() {
    // BH-01: public `/config` yalnız güvenli allowlist'i döndürür.
    let addr = spawn_server().await;
    let (status, body) = http(addr, &get("/config")).await;
    assert_eq!(status, 200);
    // Public alanlar mevcut (pricing/legal/footer bunları okur).
    assert!(
        body.contains("\"promo.founding.enabled\":\"false\""),
        "body: {body}"
    );
    assert!(body.contains("\"legal.terms.html\":\"\""), "body: {body}");
    // Internal alanlar public yanıtta YOK.
    for internal in [
        "stt.provider",
        "translate.provider",
        "providers.mock_fallback",
        "auth.email_2fa",
        "cors.allowed_origins",
        "dist.cloud.plans",
        "providers.youtube.test_url",
        "founding.gate.enabled",
    ] {
        assert!(
            !body.contains(internal),
            "internal key `{internal}` leaked to public /config: {body}"
        );
    }
}

#[tokio::test]
async fn admin_can_read_full_config_including_internal_keys() {
    let addr = spawn_server().await;
    let (status, body) = http(addr, &get_auth("/admin/config", &token("admin"))).await;
    assert_eq!(status, 200, "body: {body}");
    assert!(
        body.contains("\"stt.provider\":\"deepgram\""),
        "body: {body}"
    );
    assert!(
        body.contains("\"providers.mock_fallback\":\"true\""),
        "body: {body}"
    );
}

#[tokio::test]
async fn admin_config_read_requires_admin() {
    let addr = spawn_server().await;
    // Kimliksiz → 401.
    let (status, _) = http(addr, &get("/admin/config")).await;
    assert_eq!(status, 401);
    // Yayıncı → 403.
    let (status, _) = http(addr, &get_auth("/admin/config", &token("broadcaster"))).await;
    assert_eq!(status, 403);
}

#[tokio::test]
async fn admin_update_is_visible_to_admin_readers() {
    let addr = spawn_server().await;

    let (status, body) = http(
        addr,
        &put_config(
            Some(&token("admin")),
            r#"{"key":"stt.provider","value":"assemblyai"}"#,
        ),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");

    // Aynı süreç içinde admin full-config okuması anında yeni değeri görür;
    // stt.provider internal olduğu için public `/config`'te görünmez.
    let (status, body) = http(addr, &get_auth("/admin/config", &token("admin"))).await;
    assert_eq!(status, 200);
    assert!(
        body.contains("\"stt.provider\":\"assemblyai\""),
        "body: {body}"
    );
    let (_, public_body) = http(addr, &get("/config")).await;
    assert!(
        !public_body.contains("stt.provider"),
        "public body leaked stt.provider: {public_body}"
    );
}

#[tokio::test]
async fn legal_html_config_is_admin_editable() {
    let addr = spawn_server().await;

    let (status, body) = http(addr, &get("/config")).await;
    assert_eq!(status, 200);
    assert!(body.contains("\"legal.terms.html\":\"\""), "body: {body}");

    let html = "<h1>Terms</h1><p>Updated from admin panel.</p>";
    let payload = format!(
        r#"{{"key":"legal.terms.html","value":{}}}"#,
        serde_json::to_string(html).expect("html json")
    );
    let (status, body) = http(addr, &put_config(Some(&token("admin")), &payload)).await;
    assert_eq!(status, 200, "body: {body}");

    let (status, body) = http(addr, &get("/config")).await;
    assert_eq!(status, 200);
    assert!(body.contains("Updated from admin panel."), "body: {body}");
}

#[tokio::test]
async fn legal_html_config_strips_executable_markup_but_keeps_allowlisted_content() {
    let addr = spawn_server().await;

    let html = concat!(
        "<h1>Safe terms</h1>",
        "<script>alert(1)</script>",
        "<p onmouseover=alert(2)>Read <strong>carefully</strong>.</p>",
        "<a href=\"javascript:alert(3)\">unsafe</a>",
        "<a href=\"https://strovak.com/legal\" title=\"Legal\">safe link</a>",
        "<svg><a xlink:href=\"javascript:alert(4)\">svg</a></svg>",
    );
    let payload = format!(
        r#"{{"key":"legal.terms.html","value":{}}}"#,
        serde_json::to_string(html).expect("html json")
    );
    let (status, body) = http(addr, &put_config(Some(&token("admin")), &payload)).await;
    assert_eq!(status, 200, "body: {body}");

    let config: serde_json::Value = serde_json::from_str(&body).expect("config json");
    let sanitized = config["legal.terms.html"].as_str().expect("legal html");
    assert!(
        sanitized.contains("<h1>Safe terms</h1>"),
        "html: {sanitized}"
    );
    assert!(
        sanitized.contains("<strong>carefully</strong>"),
        "html: {sanitized}"
    );
    assert!(
        sanitized.contains("https://strovak.com/legal"),
        "html: {sanitized}"
    );
    for forbidden in [
        "<script",
        "alert(1)",
        "onmouseover",
        "javascript:",
        "<svg",
        "xlink:",
    ] {
        assert!(
            !sanitized.to_ascii_lowercase().contains(forbidden),
            "html: {sanitized}"
        );
    }
}

#[tokio::test]
async fn company_details_are_admin_editable_and_default_empty() {
    let addr = spawn_server().await;

    let (status, body) = http(addr, &get("/config")).await;
    assert_eq!(status, 200);
    assert!(body.contains("\"company.legal_name\":\"\""), "body: {body}");
    assert!(
        body.contains("\"company.registration_number\":\"\""),
        "body: {body}"
    );
    assert!(
        body.contains("\"company.registered_address\":\"\""),
        "body: {body}"
    );

    let (status, body) = http(
        addr,
        &put_config(
            Some(&token("admin")),
            r#"{"key":"company.legal_name","value":"Strovak"}"#,
        ),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");

    let (status, body) = http(addr, &get("/config")).await;
    assert_eq!(status, 200);
    assert!(
        body.contains("\"company.legal_name\":\"Strovak\""),
        "body: {body}"
    );
}

/// Gerçek Postgres ile: admin yazımı `service_config` tablosuna da işlenir ve
/// süreç yeniden başlasa (taze `ServiceConfig` + DB'den yükleme) değer kalıcı
/// kalır. Çalıştırmak için:
/// `TEST_DATABASE_URL=postgres://... cargo test -- --ignored legal_config_persists`
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL with a migrated database"]
async fn legal_config_persists_to_db_and_survives_restart() {
    let url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL");
    let pool = streamdubber_backend::db::connect(&url)
        .await
        .expect("connect");
    streamdubber_backend::db::run_migrations(&pool)
        .await
        .expect("migrations");

    let addr = spawn_server_with_db(Some(pool.clone())).await;

    let html = "<h1>Terms</h1><p>Persisted via DB.</p>";
    let payload = format!(
        r#"{{"key":"legal.terms.html","value":{}}}"#,
        serde_json::to_string(html).expect("html json")
    );
    let (status, body) = http(addr, &put_config(Some(&token("admin")), &payload)).await;
    assert_eq!(status, 200, "body: {body}");

    // "Restart" simülasyonu: taze bellek-içi store, DB'den yeniden yükle.
    let restarted = ServiceConfig::with_defaults();
    let rows = streamdubber_backend::config::load_from_db(&pool)
        .await
        .expect("load_from_db");
    restarted.apply_db_rows(rows);
    assert_eq!(restarted.get("legal.terms.html").as_deref(), Some(html));
}

#[tokio::test]
async fn write_requires_auth() {
    let addr = spawn_server().await;
    let (status, _) = http(
        addr,
        &put_config(None, r#"{"key":"stt.provider","value":"x"}"#),
    )
    .await;
    assert_eq!(status, 401);
}

#[tokio::test]
async fn broadcaster_cannot_write() {
    let addr = spawn_server().await;
    let (status, _) = http(
        addr,
        &put_config(
            Some(&token("broadcaster")),
            r#"{"key":"stt.provider","value":"x"}"#,
        ),
    )
    .await;
    assert_eq!(status, 403);
}

#[tokio::test]
async fn unknown_key_is_rejected() {
    let addr = spawn_server().await;
    let (status, _) = http(
        addr,
        &put_config(
            Some(&token("admin")),
            r#"{"key":"attacker.key","value":"x"}"#,
        ),
    )
    .await;
    assert_eq!(status, 404);
}

/// BH-02: `ConfigSpec` registry, bilinen anahtarın değerini tipine göre
/// doğrular — geçersiz değer 422, bellek hiç değişmez.
#[tokio::test]
async fn unknown_provider_value_is_rejected_with_422() {
    let addr = spawn_server().await;
    let (status, body) = http(
        addr,
        &put_config(
            Some(&token("admin")),
            r#"{"key":"stt.provider","value":"ghost"}"#,
        ),
    )
    .await;
    assert_eq!(status, 422, "body: {body}");

    // Bellek eski değerde kalır (deepgram seed default'u).
    let (_, admin_body) = http(addr, &get_auth("/admin/config", &token("admin"))).await;
    assert!(
        admin_body.contains("\"stt.provider\":\"deepgram\""),
        "body: {admin_body}"
    );
}

#[tokio::test]
async fn zero_rate_limit_is_rejected_with_422() {
    let addr = spawn_server().await;
    let (status, body) = http(
        addr,
        &put_config(
            Some(&token("admin")),
            r#"{"key":"auth.login.rate_limit_per_min","value":"0"}"#,
        ),
    )
    .await;
    assert_eq!(status, 422, "body: {body}");
}

#[tokio::test]
async fn wildcard_cors_origin_is_rejected_with_422() {
    let addr = spawn_server().await;
    let (status, body) = http(
        addr,
        &put_config(
            Some(&token("admin")),
            r#"{"key":"cors.allowed_origins","value":"*"}"#,
        ),
    )
    .await;
    assert_eq!(status, 422, "body: {body}");
}

#[tokio::test]
async fn malformed_url_is_rejected_with_422() {
    let addr = spawn_server().await;
    let (status, body) = http(
        addr,
        &put_config(
            Some(&token("admin")),
            r#"{"key":"auth.oauth_redirect_base","value":"not a url"}"#,
        ),
    )
    .await;
    assert_eq!(status, 422, "body: {body}");
}

/// BH-02: production'da http(s) sağlayıcı/redirect URL'lerinin şeması https
/// olmalı — dev'in `http://localhost` varsayılanı production'da reddedilir.
#[tokio::test]
async fn http_oauth_redirect_is_rejected_in_production_but_allowed_in_dev() {
    let dev_addr = spawn_server().await;
    let (status, body) = http(
        dev_addr,
        &put_config(
            Some(&token("admin")),
            r#"{"key":"auth.oauth_redirect_base","value":"http://localhost:8080/api/auth/oauth"}"#,
        ),
    )
    .await;
    assert_eq!(status, 200, "dev should allow http: {body}");

    let prod_addr = spawn_server_production().await;
    let (status, body) = http(
        prod_addr,
        &put_config(
            Some(&token("admin")),
            r#"{"key":"auth.oauth_redirect_base","value":"http://localhost:8080/api/auth/oauth"}"#,
        ),
    )
    .await;
    assert_eq!(status, 422, "production should reject http: {body}");

    let (status, body) = http(
        prod_addr,
        &put_config(
            Some(&token("admin")),
            r#"{"key":"auth.oauth_redirect_base","value":"https://api.strovak.com/api/auth/oauth"}"#,
        ),
    )
    .await;
    assert_eq!(status, 200, "production should allow https: {body}");
}

#[tokio::test]
async fn mock_fallback_cannot_be_enabled_in_production() {
    let addr = spawn_server_production().await;
    let (status, body) = http(
        addr,
        &put_config(
            Some(&token("admin")),
            r#"{"key":"providers.mock_fallback","value":"true"}"#,
        ),
    )
    .await;
    assert_eq!(status, 422, "body: {body}");
}

/// BH-02: mutasyon DB-first — `service_config` yazımı başarısız olursa bellek
/// hiç değişmemiş kalır (eski "yaz-sonra-geri-al" yerine). Gerçek Postgres
/// gerektirir; havuz `close()` ile kapatılıp yazım zorla başarısız kılınır.
/// `TEST_DATABASE_URL=postgres://... cargo test -- --ignored db_write_failure`
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL with a migrated database"]
async fn db_write_failure_leaves_memory_unchanged() {
    let url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL");
    let pool = streamdubber_backend::db::connect(&url)
        .await
        .expect("connect");
    streamdubber_backend::db::run_migrations(&pool)
        .await
        .expect("migrations");

    let addr = spawn_server_with_db(Some(pool.clone())).await;
    pool.close().await;

    let (status, _) = http(
        addr,
        &put_config(
            Some(&token("admin")),
            r#"{"key":"stt.provider","value":"assemblyai"}"#,
        ),
    )
    .await;
    assert_eq!(status, 500);

    let (_, admin_body) = http(addr, &get_auth("/admin/config", &token("admin"))).await;
    assert!(
        admin_body.contains("\"stt.provider\":\"deepgram\""),
        "memory should be untouched after DB failure: {admin_body}"
    );
}
