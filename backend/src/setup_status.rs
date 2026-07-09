//! Production Setup sihirbazı — `GET /admin/setup/status` (Faz 1, V11).
//!
//! Operatörün production'a çıkmadan önce nelerin eksik olduğunu tek bakışta
//! görmesi için mevcut alt sistemlerin (secrets, provider testleri, Stripe
//! price'ları, OAuth entegrasyonları, config, persist dosyaları) durumunu tek
//! bir yanıtta toplar. Hiçbir yeni "gerçek" state üretmez — sadece var olan
//! [`ManagedSecrets`], [`ServiceConfig`], [`Integrations`], [`PlanRegistry`]
//! ve [`ProviderTestStore`]'u okur.

use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::auth::authz::AdminUser;
use crate::auth::bootstrap::BootstrapAdmin;
use crate::config::ServiceConfig;
use crate::integrations::Integrations;
use crate::logging::Logger;
use crate::plans::PlanRegistry;
use crate::provider_check::{self, ProviderTestStore};
use crate::secrets::{names, ManagedSecrets};

/// OAuth ile bağlanan sağlayıcılar (secret çiftleri + `Integrations` adı).
/// `defs()` gibi `provider_check` içindeki API-key sağlayıcılarından ayrı —
/// bunlar test edilmez, sadece "configured" ve "connect" aksiyonu vardır.
const OAUTH_PROVIDERS: &[(&str, &str, &str)] = &[
    (
        "streamlabs",
        names::STREAMLABS_CLIENT_ID,
        names::STREAMLABS_CLIENT_SECRET,
    ),
    (
        "youtube",
        names::YOUTUBE_CLIENT_ID,
        names::YOUTUBE_CLIENT_SECRET,
    ),
];

/// Bir kontrolün durumu — frontend bunu yeşil/sarı/kırmızı pill olarak çizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tone {
    Ok,
    Warn,
    Bad,
}

/// Tek bir readiness kontrolü.
#[derive(Debug, Serialize)]
pub struct SetupCheck {
    pub id: &'static str,
    pub label: String,
    pub tone: Tone,
    pub detail: String,
}

/// Bir sağlayıcının admin panelde görünen profili — hem API-key hem OAuth
/// sağlayıcılar için tek model (V11 "provider profile UI").
#[derive(Debug, Serialize)]
pub struct ProviderProfile {
    pub name: String,
    pub kind: &'static str,
    pub secret_names: Vec<&'static str>,
    pub configured: bool,
    pub last_tested_at_ms: Option<i64>,
    pub last_test_ok: Option<bool>,
    pub last_test_detail: Option<String>,
    /// API-key sağlayıcılar için `None`; OAuth sağlayıcılar için admin'in
    /// authorize URL'ini alacağı yol (`GET` ile çağrılır).
    pub connect_path: Option<String>,
    /// OAuth sağlayıcılar için Google/Streamlabs konsoluna girilecek callback URL.
    pub callback_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SetupStatus {
    pub checks: Vec<SetupCheck>,
    pub providers: Vec<ProviderProfile>,
}

fn persist_env_configured(var: &str) -> bool {
    std::env::var(var).ok().filter(|v| !v.is_empty()).is_some()
}

/// `GET /admin/setup/status` (sadece admin).
///
/// 7 alt sistemi (secrets/config/plans/integrations/provider testleri/
/// bootstrap admin/logger) tek yanıtta topladığı için extractor sayısı
/// axum'un olağan handler'larından fazla — bilinçli, bölünmesi okunurluğu
/// azaltır.
#[allow(clippy::too_many_arguments)]
pub async fn get_setup_status(
    _admin: AdminUser,
    State(secrets): State<Arc<ManagedSecrets>>,
    State(cfg): State<Arc<ServiceConfig>>,
    State(plans): State<Arc<PlanRegistry>>,
    State(integ): State<Arc<Integrations>>,
    State(test_store): State<Arc<ProviderTestStore>>,
    State(bootstrap_admin): State<Arc<Option<BootstrapAdmin>>>,
    State(logger): State<Arc<Logger>>,
) -> Json<SetupStatus> {
    let mut checks = Vec::new();

    checks.push(if bootstrap_admin.is_some() {
        SetupCheck {
            id: "admin_login",
            label: "Admin login".to_string(),
            tone: Tone::Ok,
            detail: "ADMIN_EMAIL + ADMIN_PASSWORD_HASH yapılandırılmış.".to_string(),
        }
    } else {
        SetupCheck {
            id: "admin_login",
            label: "Admin login".to_string(),
            tone: Tone::Bad,
            detail: "ADMIN_EMAIL/ADMIN_PASSWORD_HASH env değişkenleri eksik — admin panel girişi 503 döner.".to_string(),
        }
    });

    let secret_statuses = secrets.status().await;
    let configured_count = secret_statuses.iter().filter(|s| s.configured).count();
    let total = names::MANAGED.len();
    checks.push(SetupCheck {
        id: "secrets",
        label: "Sağlayıcı secret'ları".to_string(),
        tone: if configured_count == total {
            Tone::Ok
        } else if configured_count > 0 {
            Tone::Warn
        } else {
            Tone::Bad
        },
        detail: format!("{configured_count}/{total} secret admin panelden yapılandırılmış."),
    });

    let mock_fallback = cfg.get("providers.mock_fallback").unwrap_or_default();
    checks.push(SetupCheck {
        id: "mock_fallback",
        label: "Mock fallback".to_string(),
        tone: if mock_fallback == "true" { Tone::Warn } else { Tone::Ok },
        detail: if mock_fallback == "true" {
            "providers.mock_fallback=true — key eksikse sessizce mock cevap döner. Production'da false yapılmalı.".to_string()
        } else {
            "providers.mock_fallback=false — key eksikse açık hata döner.".to_string()
        },
    });

    let tls_configured = persist_env_configured("TLS_CERT_PATH") && persist_env_configured("TLS_KEY_PATH");
    let mtls_configured = persist_env_configured("TLS_CLIENT_CA_PATH");
    checks.push(if !tls_configured {
        SetupCheck {
            id: "tls",
            label: "TLS".to_string(),
            tone: Tone::Bad,
            detail: "TLS_CERT_PATH/TLS_KEY_PATH yapılandırılmamış — backend düz HTTP dinliyor. Bir reverse proxy TLS 1.3 terminasyonu yapmıyorsa ayarlayın.".to_string(),
        }
    } else if !mtls_configured {
        SetupCheck {
            id: "tls",
            label: "TLS".to_string(),
            tone: Tone::Warn,
            detail: "TLS aktif ama TLS_CLIENT_CA_PATH (mTLS) yok — SDK istemci sertifikası doğrulanmıyor, sadece sunucu kimliği.".to_string(),
        }
    } else {
        SetupCheck {
            id: "tls",
            label: "TLS".to_string(),
            tone: Tone::Ok,
            detail: "TLS 1.3 + mTLS (istemci sertifika doğrulaması) aktif.".to_string(),
        }
    });

    let redirect_base = cfg.get("integration.redirect_base").unwrap_or_default();
    checks.push(SetupCheck {
        id: "redirect_base",
        label: "OAuth redirect base".to_string(),
        tone: if redirect_base.contains("localhost") { Tone::Warn } else { Tone::Ok },
        detail: format!("integration.redirect_base = {redirect_base}"),
    });

    let plan_defs = plans.snapshot();
    let missing_price: Vec<String> = plan_defs
        .iter()
        .filter(|p| p.price_cents > 0)
        .filter(|p| {
            cfg.get(&format!("billing.plan.{}.stripe_price_id", p.name))
                .unwrap_or_default()
                .is_empty()
        })
        .map(|p| p.name.clone())
        .collect();
    checks.push(SetupCheck {
        id: "stripe_prices",
        label: "Stripe plan price ID".to_string(),
        tone: if missing_price.is_empty() { Tone::Ok } else { Tone::Warn },
        detail: if missing_price.is_empty() {
            "Tüm ücretli planlarda stripe_price_id ayarlı.".to_string()
        } else {
            format!("Eksik price ID: {}", missing_price.join(", "))
        },
    });

    let recent_errors = logger.recent_errors(50).len();
    checks.push(SetupCheck {
        id: "logs",
        label: "Son log hataları".to_string(),
        tone: if recent_errors == 0 { Tone::Ok } else { Tone::Warn },
        detail: format!("Son 50 kayıtta {recent_errors} hata seviyeli log."),
    });

    let persist_vars = ["SECRETS_FILE", "TOKENS_FILE", "RELAY_FILE", "USAGE_FILE", "LOG_FILE"];
    let persisted_count = persist_vars.iter().filter(|v| persist_env_configured(v)).count();
    checks.push(SetupCheck {
        id: "persistence",
        label: "Kalıcılık (restart-safe)".to_string(),
        tone: if persisted_count == persist_vars.len() {
            Tone::Ok
        } else if persisted_count > 0 {
            Tone::Warn
        } else {
            Tone::Bad
        },
        detail: format!(
            "{persisted_count}/{} kalıcılık dosyası (SECRETS_FILE/TOKENS_FILE/RELAY_FILE/USAGE_FILE/LOG_FILE) yapılandırılmış.",
            persist_vars.len()
        ),
    });

    let mut providers = Vec::new();
    for (name, secret_name) in provider_check::provider_defs() {
        let record = test_store.get(name);
        providers.push(ProviderProfile {
            name: name.to_string(),
            kind: "api_key",
            secret_names: vec![secret_name],
            configured: secret_statuses
                .iter()
                .any(|s| s.name == secret_name && s.configured),
            last_tested_at_ms: record.as_ref().map(|r| r.tested_at_ms),
            last_test_ok: record.as_ref().map(|r| r.ok),
            last_test_detail: record.map(|r| r.detail),
            connect_path: None,
            callback_url: None,
        });
    }
    for (name, id_secret, secret_secret) in OAUTH_PROVIDERS {
        providers.push(ProviderProfile {
            name: name.to_string(),
            kind: "oauth",
            secret_names: vec![id_secret, secret_secret],
            configured: integ.is_configured(name).await,
            last_tested_at_ms: None,
            last_test_ok: None,
            last_test_detail: None,
            connect_path: Some(format!("/admin/integrations/{name}/authorize")),
            callback_url: Some(format!("{redirect_base}/{name}/callback")),
        });
    }
    providers.sort_by(|a, b| a.name.cmp(&b.name));

    Json(SetupStatus { checks, providers })
}
