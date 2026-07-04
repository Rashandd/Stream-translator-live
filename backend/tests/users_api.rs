//! Yayıncı kullanıcı sistemi (1.4) uçtan-uca testleri.
//!
//! Sözleşme:
//! - `POST /api/auth/signup` → 201; aynı e-posta 409; bozuk e-posta/kısa şifre 422.
//! - `POST /api/auth/login` — SMTP yapılandırılmamışken (`auth.email_2fa=auto`)
//!   doğrudan `role=broadcaster` token döner; SMTP varken 6 haneli kod maillenir
//!   ve yanıt `two_factor_required` olur.
//! - `POST /api/auth/2fa` — maillenen kod tek kullanımlıktır; yanlış kod 401.
//! - `POST /api/auth/reset/request|confirm` — şifre sıfırlama; hesap varlığı
//!   sızdırılmaz (bilinmeyen e-posta da 200 alır).
//! - Şifreler ve kodlar hiçbir log çıktısında yer almaz.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use streamdubber_backend::{
    app,
    auth::bootstrap::BootstrapAdmin,
    auth::jwt::Jwt,
    config::ServiceConfig,
    gateway::{stt::MockStt, translate::MockTranslate, Gateway},
    secrets::{names, ManagedSecrets, MemorySecretStore},
    state::AppState,
};

const SECRET: &[u8] = b"users-api-test-secret";

/// Çok oturumlu mini SMTP sunucusu — her mailin DATA gövdesini kanala yollar.
async fn spawn_smtp_mock() -> (u16, mpsc::UnboundedReceiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("smtp bind");
    let port = listener.local_addr().expect("smtp addr").port();
    let (tx, rx) = mpsc::unbounded_channel();

    tokio::spawn(async move {
        loop {
            let Ok((socket, _)) = listener.accept().await else {
                break;
            };
            let tx = tx.clone();
            tokio::spawn(async move {
                let (read, mut write) = socket.into_split();
                let mut reader = BufReader::new(read);
                let mut line = String::new();
                let mut data = String::new();
                let mut in_data = false;

                let _ = write.write_all(b"220 mock ESMTP\r\n").await;
                loop {
                    line.clear();
                    match reader.read_line(&mut line).await {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {}
                    }
                    if in_data {
                        if line.trim_end() == "." {
                            in_data = false;
                            let _ = write.write_all(b"250 ok queued\r\n").await;
                        } else {
                            data.push_str(&line);
                        }
                        continue;
                    }
                    let upper = line.to_uppercase();
                    if upper.starts_with("EHLO") || upper.starts_with("HELO") {
                        let _ = write.write_all(b"250 mock greets you\r\n").await;
                    } else if upper.starts_with("DATA") {
                        in_data = true;
                        let _ = write.write_all(b"354 go ahead\r\n").await;
                    } else if upper.starts_with("QUIT") {
                        let _ = write.write_all(b"221 bye\r\n").await;
                        break;
                    } else {
                        let _ = write.write_all(b"250 ok\r\n").await;
                    }
                }
                let _ = tx.send(data);
            });
        }
    });

    (port, rx)
}

struct TestServer {
    addr: SocketAddr,
}

/// Sunucu kurar. `smtp_port` verilirse SMTP secret'ları memory store'a konur
/// (2FA maili gerçekten mock sunucuya gider).
async fn spawn_server(admin: Option<BootstrapAdmin>, smtp_port: Option<u16>) -> TestServer {
    let service_config = Arc::new(ServiceConfig::with_defaults());
    service_config.set("mail.smtp.tls", "none");
    let mut gateway = Gateway::new(Arc::clone(&service_config));
    gateway.register_stt(Arc::new(MockStt::new("deepgram")));
    gateway.register_translate(Arc::new(MockTranslate::new("deepl")));

    let mut state = AppState::skeleton(Jwt::new(SECRET), Arc::clone(&service_config), gateway);
    state.bootstrap_admin = Arc::new(admin);
    if let Some(port) = smtp_port {
        let store = MemorySecretStore::with([
            (names::SMTP_HOST, "127.0.0.1"),
            (names::SMTP_PORT, port.to_string().as_str()),
            (names::MAIL_FROM, "noreply@strovak.com"),
        ]);
        state.secrets = Arc::new(ManagedSecrets::new(Box::new(store), "env"));
    }
    let state = state;

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        axum::serve(listener, app::router(state)).await.expect("serve");
    });
    TestServer { addr }
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

fn post_json(path: &str, json: &str) -> String {
    format!(
        "POST {path} HTTP/1.1\r\nHost: t\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
        json.len()
    )
}

fn post_with_token(path: &str, token: &str) -> String {
    format!(
        "POST {path} HTTP/1.1\r\nHost: t\r\nAuthorization: Bearer {token}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )
}

fn get_with_token(path: &str, token: &str) -> String {
    format!(
        "GET {path} HTTP/1.1\r\nHost: t\r\nAuthorization: Bearer {token}\r\nConnection: close\r\n\r\n"
    )
}

fn json(body: &str) -> serde_json::Value {
    serde_json::from_str(body).unwrap_or_else(|_| panic!("json body: {body}"))
}

/// Mail gövdesinden 6 haneli kodu çıkarır.
fn extract_code(mail: &str) -> String {
    let digits: Vec<String> = mail
        .split(|c: char| !c.is_ascii_digit())
        .filter(|run| run.len() == 6)
        .map(str::to_string)
        .collect();
    digits.last().cloned().unwrap_or_else(|| panic!("no 6-digit code in mail: {mail}"))
}

#[tokio::test]
async fn signup_then_login_without_smtp_issues_broadcaster_token() {
    let server = spawn_server(None, None).await;

    // Signup 201.
    let (status, _) = http(
        server.addr,
        &post_json(
            "/api/auth/signup",
            r#"{"email":"caster@example.com","password":"strong-pass-1"}"#,
        ),
    )
    .await;
    assert_eq!(status, 201);

    // Aynı e-posta 409; bozuk e-posta ve kısa şifre 422.
    let (status, _) = http(
        server.addr,
        &post_json(
            "/api/auth/signup",
            r#"{"email":"CASTER@example.com","password":"another-pass-1"}"#,
        ),
    )
    .await;
    assert_eq!(status, 409);
    let (status, _) = http(
        server.addr,
        &post_json("/api/auth/signup", r#"{"email":"not-an-email","password":"strong-pass-1"}"#),
    )
    .await;
    assert_eq!(status, 422);
    let (status, _) = http(
        server.addr,
        &post_json("/api/auth/signup", r#"{"email":"b@c.co","password":"short"}"#),
    )
    .await;
    assert_eq!(status, 422);

    // SMTP yok + auth.email_2fa=auto → doğrudan broadcaster token.
    let (status, body) = http(
        server.addr,
        &post_json(
            "/api/auth/login",
            r#"{"email":"caster@example.com","password":"strong-pass-1"}"#,
        ),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    let parsed = json(&body);
    assert_eq!(parsed["role"], "broadcaster");
    let token = parsed["token"].as_str().expect("token");

    let claims = Jwt::new(SECRET).verify(token).expect("valid token");
    assert_eq!(claims.role, "broadcaster");
    assert_eq!(claims.plan, "free");
    assert_ne!(claims.sub, "admin-bootstrap");

    // Token gerçek bir yayıncı ucunu açıyor (pairing kodu üretimi).
    let (status, body) = http(server.addr, &post_with_token("/api/pairing/code", token)).await;
    assert_eq!(status, 200, "body: {body}");
    assert_eq!(json(&body)["code"].as_str().expect("code").len(), 6);

    // Yayıncı token'ı admin uçlarını AÇMAZ.
    let (status, _) = http(server.addr, &get_with_token("/admin/ping", token)).await;
    assert_eq!(status, 403);
}

#[tokio::test]
async fn wrong_password_is_401_and_secrets_never_logged() {
    let server = spawn_server(Some(BootstrapAdmin::with_plain("ops@sd.io", "admin-pass")), None).await;

    let (status, _) = http(
        server.addr,
        &post_json(
            "/api/auth/signup",
            r#"{"email":"leak@example.com","password":"super-secret-pw-1"}"#,
        ),
    )
    .await;
    assert_eq!(status, 201);

    let (status, _) = http(
        server.addr,
        &post_json(
            "/api/auth/login",
            r#"{"email":"leak@example.com","password":"wrong-pass-xyz"}"#,
        ),
    )
    .await;
    assert_eq!(status, 401);

    // Bilinmeyen e-posta da aynı 401'i alır (enumeration yok).
    let (status, _) = http(
        server.addr,
        &post_json(
            "/api/auth/login",
            r#"{"email":"ghost@example.com","password":"whatever-pass"}"#,
        ),
    )
    .await;
    assert_eq!(status, 401);

    // Loglarda deneme var, şifreler yok.
    let (_, body) = http(
        server.addr,
        &post_json("/api/auth/login", r#"{"email":"ops@sd.io","password":"admin-pass"}"#),
    )
    .await;
    let admin_token = json(&body)["token"].as_str().expect("admin token").to_string();
    let (_, logs) = http(server.addr, &get_with_token("/admin/logs?service=auth", &admin_token)).await;
    assert!(logs.contains("login failed"), "logs: {logs}");
    assert!(!logs.contains("wrong-pass-xyz"), "password leaked: {logs}");
    assert!(!logs.contains("super-secret-pw-1"), "password leaked: {logs}");
    assert!(!logs.contains("admin-pass"), "password leaked: {logs}");
}

#[tokio::test]
async fn admin_login_still_works_and_reports_role() {
    let server = spawn_server(Some(BootstrapAdmin::with_plain("ops@sd.io", "dev-pass")), None).await;
    let (status, body) = http(
        server.addr,
        &post_json("/api/auth/login", r#"{"email":"ops@sd.io","password":"dev-pass"}"#),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    let parsed = json(&body);
    assert_eq!(parsed["role"], "admin");
    let token = parsed["token"].as_str().expect("token");
    let (status, _) = http(server.addr, &get_with_token("/admin/ping", token)).await;
    assert_eq!(status, 200);
}

#[tokio::test]
async fn smtp_configured_login_requires_mailed_code() {
    let (smtp_port, mut mails) = spawn_smtp_mock().await;
    let server = spawn_server(None, Some(smtp_port)).await;

    let (status, _) = http(
        server.addr,
        &post_json(
            "/api/auth/signup",
            r#"{"email":"twofa@example.com","password":"strong-pass-1"}"#,
        ),
    )
    .await;
    assert_eq!(status, 201);

    // Login → token DEĞİL, 2FA beklemesi.
    let (status, body) = http(
        server.addr,
        &post_json(
            "/api/auth/login",
            r#"{"email":"twofa@example.com","password":"strong-pass-1"}"#,
        ),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    let parsed = json(&body);
    assert_eq!(parsed["two_factor_required"], true);
    assert!(parsed.get("token").is_none(), "token must not leak before 2fa: {body}");

    // Kod maille geldi.
    let mail = mails.recv().await.expect("2fa mail");
    assert!(mail.contains("verification code"), "mail: {mail}");
    let code = extract_code(&mail);

    // Yanlış kod 401.
    let (status, _) = http(
        server.addr,
        &post_json(
            "/api/auth/2fa",
            r#"{"email":"twofa@example.com","code":"000000"}"#,
        ),
    )
    .await;
    assert_eq!(status, 401);

    // Doğru kod → broadcaster token; e-posta doğrulanmış sayılır.
    let (status, body) = http(
        server.addr,
        &post_json(
            "/api/auth/2fa",
            &format!(r#"{{"email":"twofa@example.com","code":"{code}"}}"#),
        ),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");
    let token = json(&body)["token"].as_str().expect("token").to_string();
    let claims = Jwt::new(SECRET).verify(&token).expect("valid");
    assert_eq!(claims.role, "broadcaster");

    // Kod tek kullanımlık.
    let (status, _) = http(
        server.addr,
        &post_json(
            "/api/auth/2fa",
            &format!(r#"{{"email":"twofa@example.com","code":"{code}"}}"#),
        ),
    )
    .await;
    assert_eq!(status, 401);
}

#[tokio::test]
async fn password_reset_flow_rotates_password() {
    let (smtp_port, mut mails) = spawn_smtp_mock().await;
    let server = spawn_server(None, Some(smtp_port)).await;

    let (status, _) = http(
        server.addr,
        &post_json(
            "/api/auth/signup",
            r#"{"email":"reset@example.com","password":"old-password-1"}"#,
        ),
    )
    .await;
    assert_eq!(status, 201);

    // Bilinmeyen e-posta da 200 alır ve mail GİTMEZ (enumeration yok).
    let (status, _) = http(
        server.addr,
        &post_json("/api/auth/reset/request", r#"{"email":"ghost@example.com"}"#),
    )
    .await;
    assert_eq!(status, 200);

    // Gerçek hesap → kod maillenir.
    let (status, _) = http(
        server.addr,
        &post_json("/api/auth/reset/request", r#"{"email":"reset@example.com"}"#),
    )
    .await;
    assert_eq!(status, 200);
    let mail = mails.recv().await.expect("reset mail");
    assert!(mail.contains("reset code"), "mail: {mail}");
    let code = extract_code(&mail);

    // Kısa yeni şifre 422; yanlış kod 401.
    let (status, _) = http(
        server.addr,
        &post_json(
            "/api/auth/reset/confirm",
            &format!(r#"{{"email":"reset@example.com","code":"{code}","new_password":"short"}}"#),
        ),
    )
    .await;
    assert_eq!(status, 422);
    let (status, _) = http(
        server.addr,
        &post_json(
            "/api/auth/reset/confirm",
            r#"{"email":"reset@example.com","code":"000000","new_password":"new-password-2"}"#,
        ),
    )
    .await;
    assert_eq!(status, 401);

    // 422 kod tüketmedi — aynı kod geçerli yeni şifreyle çalışır.
    let (status, body) = http(
        server.addr,
        &post_json(
            "/api/auth/reset/confirm",
            &format!(
                r#"{{"email":"reset@example.com","code":"{code}","new_password":"new-password-2"}}"#
            ),
        ),
    )
    .await;
    assert_eq!(status, 200, "body: {body}");

    // Eski şifre 401, yenisi girer (SMTP açık → 2FA beklemesi döner, bu da girişin
    // kimlik doğrulamayı geçtiğini kanıtlar).
    let (status, _) = http(
        server.addr,
        &post_json(
            "/api/auth/login",
            r#"{"email":"reset@example.com","password":"old-password-1"}"#,
        ),
    )
    .await;
    assert_eq!(status, 401);
    let (status, body) = http(
        server.addr,
        &post_json(
            "/api/auth/login",
            r#"{"email":"reset@example.com","password":"new-password-2"}"#,
        ),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(json(&body)["two_factor_required"], true);
}

/// Gerçek Postgres'e karşı users write-behind doğrulaması.
/// Koşmak için: `TEST_DATABASE_URL=postgres://... cargo test -- --ignored users_persist`
#[tokio::test]
#[ignore]
async fn users_persist_roundtrip_against_real_db() {
    use streamdubber_backend::users::UserStore;

    let url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL");
    let pool = streamdubber_backend::db::connect(&url).await.expect("connect");
    streamdubber_backend::db::run_migrations(&pool).await.expect("migrations");

    let email = format!("db-roundtrip-{}@example.com", uuid::Uuid::new_v4());
    let store = UserStore::in_memory().with_db(Some(pool.clone()));
    let record = store.signup(&email, "db-roundtrip-pass1", "free").expect("signup");

    // Write-behind async — satır görünene dek bekle (üst sınır 5 sn).
    let mut found = false;
    for _ in 0..50 {
        let row: Option<(String, bool)> = sqlx::query_as(
            "SELECT u.id::text, u.email_verified FROM users u WHERE u.email = $1",
        )
        .bind(email.to_lowercase())
        .fetch_optional(&pool)
        .await
        .expect("query");
        if let Some((id, verified)) = row {
            assert_eq!(id, record.id);
            assert!(!verified);
            found = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(found, "user row not persisted");

    // mark_verified yansır; plan FK gerçek plans satırına çözülür.
    store.mark_verified(&email).expect("mark verified");
    let mut verified = false;
    for _ in 0..50 {
        let row: (bool, String) = sqlx::query_as(
            "SELECT u.email_verified, p.name FROM users u JOIN plans p ON p.id = u.plan_id WHERE u.email = $1",
        )
        .bind(email.to_lowercase())
        .fetch_one(&pool)
        .await
        .expect("query");
        if row.0 {
            assert_eq!(row.1, "free");
            verified = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(verified, "email_verified not persisted");

    // Temizlik.
    sqlx::query("DELETE FROM users WHERE email = $1")
        .bind(email.to_lowercase())
        .execute(&pool)
        .await
        .expect("cleanup");
}
