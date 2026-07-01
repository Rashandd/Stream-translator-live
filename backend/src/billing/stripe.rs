//! Stripe REST client (Faz 1.11) — checkout session oluşturma + webhook doğrulama.
//!
//! Resmi Stripe SDK'sı yerine ince bir REST/HMAC katmanı: checkout session
//! oluşturma (`POST /v1/checkout/sessions`, form-encoded, Basic Auth) ve
//! webhook imza doğrulama (`Stripe-Signature` header, zaman toleranslı
//! HMAC-SHA256). `secret_key`/`webhook_secret` `SecretStore`'dan gelir, koda
//! gömülmez.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_BASE_URL: &str = "https://api.stripe.com";
const DEFAULT_TOLERANCE_SECS: i64 = 300;

type HmacSha256 = Hmac<Sha256>;

/// Stripe API/webhook hataları.
#[derive(Debug, thiserror::Error)]
pub enum StripeError {
    #[error("stripe not configured")]
    NotConfigured,
    #[error("stripe request failed: {0}")]
    Request(String),
    #[error("missing or malformed Stripe-Signature header")]
    MalformedSignature,
    #[error("signature does not match payload")]
    SignatureMismatch,
    #[error("webhook timestamp outside tolerance")]
    TimestampOutOfRange,
    #[error("invalid event payload: {0}")]
    InvalidPayload(String),
}

/// Checkout session yanıtı — yayıncı `url`'e yönlendirilir.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CheckoutSession {
    pub id: String,
    pub url: Option<String>,
}

/// Doğrulanmış bir webhook payload'ından çözülen minimal Stripe event'i.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct StripeEvent {
    pub id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub data: StripeEventData,
}

/// Event gövdesi — nesnenin ham hali (checkout session, subscription, ...).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct StripeEventData {
    pub object: serde_json::Value,
}

/// Stripe REST client'ı.
pub struct StripeClient {
    secret_key: Option<String>,
    base_url: String,
    http: reqwest::Client,
}

impl StripeClient {
    /// `secret_key` yoksa client "yapılandırılmamış" kalır; her çağrı
    /// [`StripeError::NotConfigured`] döner (ağa hiç çıkmaz).
    pub fn new(secret_key: Option<String>) -> Self {
        StripeClient {
            secret_key,
            base_url: DEFAULT_BASE_URL.to_string(),
            http: reqwest::Client::new(),
        }
    }

    /// Test/self-host için özel API tabanı ile kurar.
    #[cfg(test)]
    fn with_base_url(secret_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        StripeClient {
            secret_key: Some(secret_key.into()),
            base_url: base_url.into(),
            http: reqwest::Client::new(),
        }
    }

    /// `STRIPE_SECRET_KEY` çözümlendi mi?
    pub fn is_configured(&self) -> bool {
        self.secret_key.is_some()
    }

    /// `mode=subscription` bir Checkout Session oluşturur.
    ///
    /// `client_reference_id` StreamDubber user UUID'sidir — webhook geldiğinde
    /// hangi yayıncıya ait olduğunu eşlemek için kullanılır.
    pub async fn create_checkout_session(
        &self,
        price_id: &str,
        success_url: &str,
        cancel_url: &str,
        client_reference_id: &str,
        customer_email: Option<&str>,
    ) -> Result<CheckoutSession, StripeError> {
        let secret_key = self.secret_key.as_deref().ok_or(StripeError::NotConfigured)?;

        let mut params = vec![
            ("mode", "subscription".to_string()),
            ("line_items[0][price]", price_id.to_string()),
            ("line_items[0][quantity]", "1".to_string()),
            ("success_url", success_url.to_string()),
            ("cancel_url", cancel_url.to_string()),
            ("client_reference_id", client_reference_id.to_string()),
        ];
        if let Some(email) = customer_email {
            params.push(("customer_email", email.to_string()));
        }

        let resp = self
            .http
            .post(format!("{}/v1/checkout/sessions", self.base_url))
            .basic_auth(secret_key, Some(""))
            .form(&params)
            .send()
            .await
            .map_err(|e| StripeError::Request(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(StripeError::Request(format!(
                "stripe returned status {}",
                resp.status()
            )));
        }

        resp.json::<CheckoutSession>()
            .await
            .map_err(|e| StripeError::Request(e.to_string()))
    }
}

/// `Stripe-Signature` header'ını ayrıştırır: `t=<unix_ts>,v1=<hex>[,v1=<hex>...]`.
fn parse_signature_header(header: &str) -> Result<(i64, Vec<&str>), StripeError> {
    let mut timestamp = None;
    let mut v1_sigs = Vec::new();
    for part in header.split(',') {
        let mut kv = part.splitn(2, '=');
        match (kv.next(), kv.next()) {
            (Some("t"), Some(v)) => timestamp = v.parse::<i64>().ok(),
            (Some("v1"), Some(v)) => v1_sigs.push(v),
            _ => {}
        }
    }
    let timestamp = timestamp.ok_or(StripeError::MalformedSignature)?;
    if v1_sigs.is_empty() {
        return Err(StripeError::MalformedSignature);
    }
    Ok((timestamp, v1_sigs))
}

/// `t.payload` üzerinden HMAC-SHA256 hesaplar, hex döner.
fn compute_signature(secret: &str, timestamp: i64, payload: &[u8]) -> String {
    // Stripe webhook secret'ları değişken uzunlukta; HMAC her anahtar
    // uzunluğunu kabul eder.
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac accepts any key length");
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(payload);
    hex::encode(mac.finalize().into_bytes())
}

/// Zamanlama saldırılarına karşı sabit zamanlı karşılaştırma.
fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Webhook imzasını doğrular (Stripe'ın zaman toleranslı HMAC-SHA256 şeması).
///
/// Şimdiki zamanı kullanır; sabit zaman için [`verify_signature_at`].
pub fn verify_signature(payload: &[u8], sig_header: &str, secret: &str) -> Result<(), StripeError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_secs() as i64;
    verify_signature_at(payload, sig_header, secret, now, DEFAULT_TOLERANCE_SECS)
}

/// Test edilebilir çekirdek: `now`/`tolerance` enjekte edilir.
fn verify_signature_at(
    payload: &[u8],
    sig_header: &str,
    secret: &str,
    now: i64,
    tolerance_secs: i64,
) -> Result<(), StripeError> {
    let (timestamp, v1_sigs) = parse_signature_header(sig_header)?;
    if (now - timestamp).abs() > tolerance_secs {
        return Err(StripeError::TimestampOutOfRange);
    }
    let expected = compute_signature(secret, timestamp, payload);
    if v1_sigs.iter().any(|s| constant_time_eq(s, &expected)) {
        Ok(())
    } else {
        Err(StripeError::SignatureMismatch)
    }
}

/// Doğrulanmış bir webhook payload'ını [`StripeEvent`]'e çözer.
pub fn parse_event(payload: &[u8]) -> Result<StripeEvent, StripeError> {
    serde_json::from_slice(payload).map_err(|e| StripeError::InvalidPayload(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::Form;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::post;
    use axum::{Json, Router};
    use std::collections::HashMap;

    fn sign(secret: &str, timestamp: i64, payload: &[u8]) -> String {
        format!("t={timestamp},v1={}", compute_signature(secret, timestamp, payload))
    }

    #[test]
    fn verify_accepts_valid_signature() {
        let payload = br#"{"id":"evt_1","type":"checkout.session.completed"}"#;
        let header = sign("whsec_test", 1_000, payload);
        assert!(verify_signature_at(payload, &header, "whsec_test", 1_050, 300).is_ok());
    }

    #[test]
    fn verify_rejects_tampered_payload() {
        let payload = br#"{"id":"evt_1"}"#;
        let header = sign("whsec_test", 1_000, payload);
        let tampered = br#"{"id":"evt_2"}"#;
        assert!(matches!(
            verify_signature_at(tampered, &header, "whsec_test", 1_050, 300),
            Err(StripeError::SignatureMismatch)
        ));
    }

    #[test]
    fn verify_rejects_wrong_secret() {
        let payload = br#"{"id":"evt_1"}"#;
        let header = sign("whsec_test", 1_000, payload);
        assert!(matches!(
            verify_signature_at(payload, &header, "whsec_other", 1_050, 300),
            Err(StripeError::SignatureMismatch)
        ));
    }

    #[test]
    fn verify_rejects_expired_timestamp() {
        let payload = br#"{"id":"evt_1"}"#;
        let header = sign("whsec_test", 1_000, payload);
        // 1000 + 301 > 300 tolerans -> reddedilir.
        assert!(matches!(
            verify_signature_at(payload, &header, "whsec_test", 1_301, 300),
            Err(StripeError::TimestampOutOfRange)
        ));
    }

    #[test]
    fn verify_rejects_malformed_header() {
        let payload = b"{}";
        assert!(matches!(
            verify_signature_at(payload, "not-a-valid-header", "whsec_test", 0, 300),
            Err(StripeError::MalformedSignature)
        ));
    }

    #[test]
    fn parse_event_reads_type_and_object() {
        let payload = br#"{
            "id": "evt_1",
            "type": "checkout.session.completed",
            "data": { "object": { "client_reference_id": "user-1" } }
        }"#;
        let event = parse_event(payload).unwrap();
        assert_eq!(event.event_type, "checkout.session.completed");
        assert_eq!(
            event.data.object["client_reference_id"].as_str(),
            Some("user-1")
        );
    }

    #[tokio::test]
    async fn unconfigured_client_errors_without_network_call() {
        let client = StripeClient::new(None);
        let err = client
            .create_checkout_session("price_1", "https://s", "https://c", "user-1", None)
            .await
            .unwrap_err();
        assert!(matches!(err, StripeError::NotConfigured));
    }

    async fn spawn_checkout_mock() -> String {
        async fn checkout(
            headers: HeaderMap,
            Form(form): Form<HashMap<String, String>>,
        ) -> (StatusCode, Json<serde_json::Value>) {
            let auth = headers.get("Authorization").and_then(|v| v.to_str().ok());
            if auth != Some("Basic c2tfdGVzdDo=") {
                // base64("sk_test:") — yanlış secret key.
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({ "error": { "message": "invalid api key" } })),
                );
            }
            if form.get("line_items[0][price]").map(String::as_str) != Some("price_123") {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": { "message": "missing price" } })),
                );
            }
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "id": "cs_test_1",
                    "url": "https://checkout.stripe.com/c/pay/cs_test_1"
                })),
            )
        }
        let app = Router::new().route("/v1/checkout/sessions", post(checkout));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn creates_checkout_session_with_correct_auth_and_params() {
        let base_url = spawn_checkout_mock().await;
        let client = StripeClient::with_base_url("sk_test", base_url);
        let session = client
            .create_checkout_session(
                "price_123",
                "https://app/success",
                "https://app/cancel",
                "user-1",
                Some("user@example.com"),
            )
            .await
            .unwrap();
        assert_eq!(session.id, "cs_test_1");
        assert_eq!(
            session.url.as_deref(),
            Some("https://checkout.stripe.com/c/pay/cs_test_1")
        );
    }

    #[tokio::test]
    async fn wrong_secret_key_is_request_error() {
        let base_url = spawn_checkout_mock().await;
        let client = StripeClient::with_base_url("sk_wrong", base_url);
        let err = client
            .create_checkout_session("price_123", "https://s", "https://c", "user-1", None)
            .await
            .unwrap_err();
        assert!(matches!(err, StripeError::Request(_)));
    }
}
