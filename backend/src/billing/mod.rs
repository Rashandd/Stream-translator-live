//! Stripe altyapısı (Faz 1.11) — checkout başlatma + webhook işleme.
//!
//! Plan → Stripe Price ID eşlemesi `ServiceConfig`'ten (`billing.plan.<plan>.stripe_price_id`)
//! okunur; hard-coded değildir, admin panelden değiştirilebilir. `client_id`/`secret_key`
//! gibi burada da secret'lar (`STRIPE_SECRET_KEY`, `STRIPE_WEBHOOK_SECRET`) `SecretStore`'dan
//! (env/Vault) gelir.
//!
//! Webhook event'lerinin abonelik durumunu DB'ye (`subscriptions`, `users.stripe_customer_id`)
//! yazması follow-up'tır — bu modül event'i doğrulayıp ayrıştırır ve loglar.

pub mod stripe;

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::auth::authz::AuthUser;
use crate::config::ServiceConfig;
use crate::secrets::{names, SecretStore};
use stripe::{StripeClient, StripeError};

/// Stripe client'ı + webhook secret'ı taşıyan paylaşılan state.
pub struct Billing {
    client: StripeClient,
    webhook_secret: Option<String>,
}

impl Billing {
    /// Sağlayıcı kimliklerini `SecretStore`'dan çözümleyerek kurar. Secret
    /// eksikse ilgili işlem (checkout/webhook) "yapılandırılmamış" olarak
    /// reddedilir.
    pub async fn from_secrets(secrets: &dyn SecretStore) -> Self {
        let secret_key = secrets.get(names::STRIPE_SECRET_KEY).await.ok();
        let webhook_secret = secrets.get(names::STRIPE_WEBHOOK_SECRET).await.ok();
        Billing {
            client: StripeClient::new(secret_key),
            webhook_secret,
        }
    }

    /// Yapılandırılmamış billing — Stripe secret'ları olmayan ortamlar (ve
    /// testler) için. checkout/webhook 503 döner.
    pub fn empty() -> Self {
        Billing {
            client: StripeClient::new(None),
            webhook_secret: None,
        }
    }

    /// Test/manuel kurulum için doğrudan client ile.
    #[cfg(test)]
    fn from_parts(client: StripeClient, webhook_secret: Option<String>) -> Self {
        Billing { client, webhook_secret }
    }
}

/// `StripeError` → HTTP durum kodu.
fn stripe_status(err: &StripeError) -> StatusCode {
    match err {
        StripeError::NotConfigured => StatusCode::SERVICE_UNAVAILABLE,
        StripeError::Request(_) => StatusCode::BAD_GATEWAY,
        StripeError::MalformedSignature
        | StripeError::SignatureMismatch
        | StripeError::TimestampOutOfRange
        | StripeError::InvalidPayload(_) => StatusCode::BAD_REQUEST,
    }
}

/// `POST /api/billing/checkout` gövdesi.
#[derive(Debug, Deserialize)]
pub struct CheckoutRequest {
    pub plan: String,
}

/// Yayıncının yönlendirileceği Stripe Checkout URL'si.
#[derive(Debug, Serialize)]
pub struct CheckoutResponse {
    pub url: String,
}

/// `POST /api/billing/checkout` (yayıncı) — seçilen plan için bir Stripe
/// Checkout Session başlatır ve yönlendirme URL'sini döner.
pub async fn checkout(
    user: AuthUser,
    State(billing): State<Arc<Billing>>,
    State(cfg): State<Arc<ServiceConfig>>,
    Json(req): Json<CheckoutRequest>,
) -> Result<Json<CheckoutResponse>, (StatusCode, String)> {
    let price_id = cfg
        .get(&format!("billing.plan.{}.stripe_price_id", req.plan))
        .filter(|v| !v.is_empty())
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("plan '{}' has no configured Stripe price", req.plan),
            )
        })?;

    let success_url = cfg
        .get("billing.success_url")
        .unwrap_or_else(|| "http://localhost:3000/dashboard/billing?checkout=success".to_string());
    let cancel_url = cfg
        .get("billing.cancel_url")
        .unwrap_or_else(|| "http://localhost:3000/dashboard/billing?checkout=canceled".to_string());

    let session = billing
        .client
        .create_checkout_session(&price_id, &success_url, &cancel_url, &user.claims.sub, None)
        .await
        .map_err(|err| (stripe_status(&err), err.to_string()))?;

    let url = session
        .url
        .ok_or_else(|| (StatusCode::BAD_GATEWAY, "stripe returned no checkout url".to_string()))?;

    Ok(Json(CheckoutResponse { url }))
}

/// `POST /webhooks/stripe` — Stripe'tan gelen abonelik olayları.
///
/// İmza `Stripe-Signature` header'ından, ham (Json-parse edilmemiş) body
/// üzerinden doğrulanır — Stripe HMAC'i JSON'un ayrıştırılmadan önceki
/// byte'ları üzerinden hesaplar.
pub async fn webhook(
    State(billing): State<Arc<Billing>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, (StatusCode, String)> {
    let secret = billing
        .webhook_secret
        .as_deref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "stripe webhook not configured".to_string()))?;

    let sig_header = headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or((StatusCode::BAD_REQUEST, "missing Stripe-Signature header".to_string()))?;

    stripe::verify_signature(&body, sig_header, secret)
        .map_err(|err| (stripe_status(&err), err.to_string()))?;

    let event = stripe::parse_event(&body).map_err(|err| (stripe_status(&err), err.to_string()))?;

    // TODO(follow-up): event türüne göre `subscriptions`/`users.stripe_customer_id`
    // güncelle (checkout.session.completed, customer.subscription.updated/deleted).
    // Şimdilik sadece doğrulanmış event loglanır.
    tracing::info!(event_id = %event.id, event_type = %event.event_type, "stripe webhook received");

    Ok(StatusCode::OK)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::FromRef;
    use axum::routing::post;
    use axum::Router;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    #[derive(Clone)]
    struct TestState {
        billing: Arc<Billing>,
        cfg: Arc<ServiceConfig>,
    }

    impl FromRef<TestState> for Arc<Billing> {
        fn from_ref(s: &TestState) -> Self {
            s.billing.clone()
        }
    }
    impl FromRef<TestState> for Arc<ServiceConfig> {
        fn from_ref(s: &TestState) -> Self {
            s.cfg.clone()
        }
    }

    /// Webhook router'ını gerçek bir TCP dinleyicide ayağa kaldırır.
    async fn spawn_webhook_server(billing: Billing) -> std::net::SocketAddr {
        let state = TestState {
            billing: Arc::new(billing),
            cfg: Arc::new(ServiceConfig::with_defaults()),
        };
        let app = Router::new()
            .route("/webhooks/stripe", post(webhook))
            .with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        addr
    }

    fn sign(secret: &str, timestamp: u64, payload: &[u8]) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(timestamp.to_string().as_bytes());
        mac.update(b".");
        mac.update(payload);
        format!("t={timestamp},v1={}", hex::encode(mac.finalize().into_bytes()))
    }

    #[tokio::test]
    async fn webhook_rejects_missing_signature() {
        let addr = spawn_webhook_server(Billing::from_parts(StripeClient::new(None), Some("whsec".into()))).await;
        let resp = reqwest::Client::new()
            .post(format!("http://{addr}/webhooks/stripe"))
            .body("{}")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn webhook_rejects_unconfigured_secret() {
        let addr = spawn_webhook_server(Billing::from_parts(StripeClient::new(None), None)).await;
        let resp = reqwest::Client::new()
            .post(format!("http://{addr}/webhooks/stripe"))
            .header("stripe-signature", "t=1,v1=deadbeef")
            .body("{}")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn webhook_accepts_valid_signature() {
        let secret = "whsec_test";
        let addr = spawn_webhook_server(Billing::from_parts(StripeClient::new(None), Some(secret.into()))).await;

        let payload = br#"{"id":"evt_1","type":"checkout.session.completed","data":{"object":{}}}"#;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let header = sign(secret, now, payload);

        let resp = reqwest::Client::new()
            .post(format!("http://{addr}/webhooks/stripe"))
            .header("stripe-signature", header)
            .body(payload.to_vec())
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn webhook_rejects_tampered_body() {
        let secret = "whsec_test";
        let addr = spawn_webhook_server(Billing::from_parts(StripeClient::new(None), Some(secret.into()))).await;

        let signed_payload = br#"{"id":"evt_1","type":"x"}"#;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let header = sign(secret, now, signed_payload);

        let resp = reqwest::Client::new()
            .post(format!("http://{addr}/webhooks/stripe"))
            .header("stripe-signature", header)
            .body(r#"{"id":"evt_2","type":"y"}"#)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
