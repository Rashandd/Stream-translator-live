//! SMTP mail gönderimi — 2FA/işlem mailleri altyapısı + canlı test ucu.
//!
//! Stub SMTP'nin gerçek karşılığı (doğrulama listesi: "mail gönderimi").
//! SMTP kimlikleri (`SMTP_HOST/PORT/USERNAME/PASSWORD`, `MAIL_FROM`)
//! [`SecretStore`]'dan **çağrı anında** çözülür — admin panelden girilir,
//! restart gerekmez. TLS kipi `mail.smtp.tls` config anahtarından:
//! `starttls` (üretim varsayılanı) | `none` (test/dahili relay).
//!
//! 2FA/e-posta doğrulama akışları kullanıcı sistemiyle (1.4) bu Mailer'ı
//! kullanacak; `POST /admin/mail/test` şimdiden canlı doğrulama sağlar.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use serde::{Deserialize, Serialize};

use crate::auth::authz::AdminUser;
use crate::config::ServiceConfig;
use crate::logging::{LogLevel, Logger};
use crate::secrets::{names, ManagedSecrets, SecretStore};

/// Mail hataları.
#[derive(Debug, thiserror::Error)]
pub enum MailError {
    #[error("smtp not configured — set SMTP_HOST and MAIL_FROM under Admin > Secrets")]
    NotConfigured,
    #[error("invalid address: {0}")]
    Address(String),
    #[error("smtp send failed: {0}")]
    Send(String),
}

/// Çözülmüş SMTP ayarları (çağrı anı görüntüsü).
struct SmtpSettings {
    host: String,
    port: u16,
    username: Option<String>,
    password: Option<String>,
    from: String,
    starttls: bool,
}

async fn resolve_settings(
    secrets: &dyn SecretStore,
    cfg: &ServiceConfig,
) -> Result<SmtpSettings, MailError> {
    let get = |name: &'static str| async move {
        secrets.get(name).await.ok().filter(|v| !v.is_empty())
    };
    let (Some(host), Some(from)) = (get(names::SMTP_HOST).await, get(names::MAIL_FROM).await)
    else {
        return Err(MailError::NotConfigured);
    };
    let port = get(names::SMTP_PORT)
        .await
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(587);
    let starttls = cfg.get("mail.smtp.tls").as_deref() != Some("none");
    Ok(SmtpSettings {
        host,
        port,
        username: get(names::SMTP_USERNAME).await,
        password: get(names::SMTP_PASSWORD).await,
        from,
        starttls,
    })
}

/// Bir maili gönderir. Ayarlar her çağrıda store'dan çözülür.
pub async fn send(
    secrets: &dyn SecretStore,
    cfg: &ServiceConfig,
    to: &str,
    subject: &str,
    body: &str,
) -> Result<(), MailError> {
    let settings = resolve_settings(secrets, cfg).await?;

    let from: Mailbox = settings
        .from
        .parse()
        .map_err(|_| MailError::Address(format!("MAIL_FROM invalid: {}", settings.from)))?;
    let to: Mailbox = to
        .parse()
        .map_err(|_| MailError::Address(format!("recipient invalid: {to}")))?;

    let message = Message::builder()
        .from(from)
        .to(to)
        .subject(subject)
        .body(body.to_string())
        .map_err(|e| MailError::Send(e.to_string()))?;

    let mut builder = if settings.starttls {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&settings.host)
            .map_err(|e| MailError::Send(e.to_string()))?
    } else {
        // Test/dahili relay: TLS'siz düz SMTP.
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&settings.host)
    };
    builder = builder.port(settings.port);
    if let (Some(user), Some(pass)) = (settings.username, settings.password) {
        builder = builder.credentials(Credentials::new(user, pass));
    }
    let transport = builder.build();

    transport
        .send(message)
        .await
        .map_err(|e| MailError::Send(e.to_string()))?;
    Ok(())
}

/// `POST /admin/mail/test` gövdesi.
#[derive(Debug, Deserialize)]
pub struct MailTestRequest {
    pub to: String,
}

/// Test sonucu — provider testleriyle aynı sözleşme.
#[derive(Debug, Serialize)]
pub struct MailTestOutcome {
    pub ok: bool,
    pub detail: String,
}

/// `POST /admin/mail/test` (sadece admin) — panelden girilen SMTP
/// ayarlarıyla gerçek bir test maili gönderir.
pub async fn mail_test(
    admin: AdminUser,
    State(secrets): State<Arc<ManagedSecrets>>,
    State(cfg): State<Arc<ServiceConfig>>,
    State(logger): State<Arc<Logger>>,
    Json(req): Json<MailTestRequest>,
) -> Result<Json<MailTestOutcome>, (StatusCode, String)> {
    let result = send(
        secrets.as_ref(),
        &cfg,
        &req.to,
        "StreamDubber SMTP test",
        "This is a StreamDubber SMTP configuration test. If you can read this, outgoing mail works.",
    )
    .await;

    let outcome = match result {
        Ok(()) => MailTestOutcome {
            ok: true,
            detail: format!("test mail sent to {}", req.to),
        },
        Err(MailError::NotConfigured) => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                MailError::NotConfigured.to_string(),
            ));
        }
        Err(err) => MailTestOutcome {
            ok: false,
            detail: err.to_string(),
        },
    };
    logger.log(
        LogLevel::Info,
        "mail",
        &format!("smtp test → {}", if outcome.ok { "ok" } else { "failed" }),
        Some(&admin.claims.sub),
        None,
    );
    Ok(Json(outcome))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::MemorySecretStore;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;

    /// Mini SMTP sunucusu: tek oturum kabul eder, konuşmayı yürütür ve
    /// DATA gövdesini geri bildirir.
    async fn spawn_smtp_mock() -> (String, u16, oneshot::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = oneshot::channel();

        tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let (read, mut write) = socket.into_split();
            let mut reader = BufReader::new(read);
            let mut line = String::new();

            write.write_all(b"220 mock ESMTP\r\n").await.unwrap();
            let mut data = String::new();
            let mut in_data = false;
            loop {
                line.clear();
                if reader.read_line(&mut line).await.unwrap() == 0 {
                    break;
                }
                if in_data {
                    if line.trim_end() == "." {
                        in_data = false;
                        write.write_all(b"250 ok queued\r\n").await.unwrap();
                    } else {
                        data.push_str(&line);
                    }
                    continue;
                }
                let upper = line.to_uppercase();
                if upper.starts_with("EHLO") || upper.starts_with("HELO") {
                    write.write_all(b"250 mock greets you\r\n").await.unwrap();
                } else if upper.starts_with("MAIL FROM") || upper.starts_with("RCPT TO") {
                    data.push_str(&line); // zarf bilgisini de yakala
                    write.write_all(b"250 ok\r\n").await.unwrap();
                } else if upper.starts_with("DATA") {
                    in_data = true;
                    write.write_all(b"354 go ahead\r\n").await.unwrap();
                } else if upper.starts_with("QUIT") {
                    write.write_all(b"221 bye\r\n").await.unwrap();
                    break;
                } else {
                    write.write_all(b"250 ok\r\n").await.unwrap();
                }
            }
            let _ = tx.send(data);
        });

        ("127.0.0.1".to_string(), addr.port(), rx)
    }

    fn cfg_plain() -> ServiceConfig {
        let cfg = ServiceConfig::with_defaults();
        cfg.set("mail.smtp.tls", "none");
        cfg
    }

    #[tokio::test]
    async fn sends_mail_through_smtp_conversation() {
        let (host, port, received) = spawn_smtp_mock().await;
        let store = MemorySecretStore::with([
            (names::SMTP_HOST, host.as_str()),
            (names::SMTP_PORT, port.to_string().as_str()),
            (names::MAIL_FROM, "noreply@streamdubber.app"),
        ]);
        let cfg = cfg_plain();

        send(&store, &cfg, "ops@example.com", "Test subject", "Hello body")
            .await
            .expect("send");

        let data = received.await.expect("smtp session data");
        assert!(data.contains("noreply@streamdubber.app"), "data: {data}");
        assert!(data.contains("ops@example.com"));
        assert!(data.contains("Test subject"));
        assert!(data.contains("Hello body"));
    }

    #[tokio::test]
    async fn missing_host_or_from_is_not_configured() {
        let cfg = cfg_plain();
        let empty = MemorySecretStore::new();
        assert!(matches!(
            send(&empty, &cfg, "a@b.c", "s", "b").await,
            Err(MailError::NotConfigured)
        ));

        let only_host = MemorySecretStore::with([(names::SMTP_HOST, "smtp.example")]);
        assert!(matches!(
            send(&only_host, &cfg, "a@b.c", "s", "b").await,
            Err(MailError::NotConfigured)
        ));
    }

    #[tokio::test]
    async fn invalid_recipient_is_address_error() {
        let store = MemorySecretStore::with([
            (names::SMTP_HOST, "127.0.0.1"),
            (names::MAIL_FROM, "noreply@streamdubber.app"),
        ]);
        let cfg = cfg_plain();
        assert!(matches!(
            send(&store, &cfg, "not-an-address", "s", "b").await,
            Err(MailError::Address(_))
        ));
    }

    #[tokio::test]
    async fn unreachable_smtp_is_send_error() {
        let store = MemorySecretStore::with([
            (names::SMTP_HOST, "127.0.0.1"),
            (names::SMTP_PORT, "1"),
            (names::MAIL_FROM, "noreply@streamdubber.app"),
        ]);
        let cfg = cfg_plain();
        assert!(matches!(
            send(&store, &cfg, "a@b.c", "s", "b").await,
            Err(MailError::Send(_))
        ));
    }
}
