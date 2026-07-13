//! Typed config/plan validation (BH-02).
//!
//! `ServiceConfig`/`PlanRegistry`'nin alt katmanı (`set`/`set_existing`) kasıtlı
//! olarak validasyonsuzdur: `gateway::mod` testleri ve `ServiceConfig`'in kendi
//! testleri var olmayan sağlayıcı adlarıyla (`"ghost"`, `"whisper"`) doğrudan bu
//! metotları çağırıyor (fallback-tükenme / mekanik-doğrulama senaryoları).
//! Validasyon bu yüzden yalnız admin mutation giriş noktasında
//! (`app::put_config` / `app::put_plan`) uygulanır — depolama katmanına değil.
//! Bkz. `docs/backend-hardening-captions-plan.md` §13.

use crate::config::{Environment, PUBLIC_CONFIG_KEYS};
use crate::plans::PlanDefinition;

/// Bir config anahtarının kabul ettiği değer şekli.
#[derive(Debug, Clone, Copy)]
pub enum ConfigValueType {
    Bool,
    /// `[min, max]` (max yoksa üst sınırsız).
    PosInt {
        min: i64,
        max: Option<i64>,
    },
    PosFloat {
        min: f64,
        max: Option<f64>,
    },
    /// İzinli değerler — boş string izinliyse listeye açıkça `""` eklenir.
    Enum(&'static [&'static str]),
    /// Boşsa "yapılandırılmamış" kabul edilir (mevcut geri-uyumlu semantik
    /// korunur); doluysa mutlak URL olmalı ve production'da şema https/wss
    /// olmalı.
    Url {
        allow_empty: bool,
    },
    /// Virgülle ayrılmış origin listesi — `*` ve bozuk origin reddedilir.
    OriginList,
    /// Virgülle ayrılmış plan adı listesi — her ad `PlanRegistry`'de var olmalı.
    PlanList,
    /// Tek bir plan adı — `PlanRegistry`'de var olmalı, boş olamaz.
    PlanName,
    /// Virgülle ayrılmış DeepL hedef dil kodu listesi.
    LanguageCodeList,
    FreeText {
        max_len: usize,
    },
}

/// Merkezi config spec kaydı — `key` → değer tipi.
///
/// `visibility` ayrı tutulmadı: BH-01'in `PUBLIC_CONFIG_KEYS` allowlist'i zaten
/// tek doğru kaynak (bkz. [`is_public`]) — burada tekrarlanıp iki kaynağın
/// birbirinden sapması riske edilmedi.
pub struct ConfigSpec {
    pub key: &'static str,
    pub value_type: ConfigValueType,
}

/// Bir anahtarın public mi internal mi olduğunu [`PUBLIC_CONFIG_KEYS`]
/// allowlist'inden okur (BH-01 ile aynı tek kaynak).
pub fn is_public(key: &str) -> bool {
    PUBLIC_CONFIG_KEYS.contains(&key)
}

const STT_PROVIDERS: &[&str] = &["deepgram", "assemblyai", "selfhosted"];
const STT_FALLBACKS: &[&str] = &["deepgram", "assemblyai", "selfhosted", ""];
const TRANSLATE_PROVIDERS: &[&str] = &["deepl", "google"];
const TRANSLATE_FALLBACKS: &[&str] = &["deepl", "google", ""];
const TTS_PROVIDERS: &[&str] = &["disabled", "elevenlabs"];
const TTS_FALLBACKS: &[&str] = &["elevenlabs", ""];
const SMTP_TLS_MODES: &[&str] = &["starttls", "none"];
const EMAIL_2FA_MODES: &[&str] = &["auto", "required", "off"];
const LOG_LEVELS: &[&str] = &["debug", "info", "warn", "warning", "error"];

/// `ServiceConfig::with_defaults`'taki HER anahtarın burada bir karşılığı
/// olmalı — bkz. `config::tests::all_seeded_keys_have_a_spec` (BH-02 kabul
/// kriteri: "mevcut default key listesini spec registry'ye taşı").
pub const CONFIG_SPECS: &[ConfigSpec] = &[
    ConfigSpec {
        key: "stt.provider",
        value_type: ConfigValueType::Enum(STT_PROVIDERS),
    },
    ConfigSpec {
        key: "stt.fallback",
        value_type: ConfigValueType::Enum(STT_FALLBACKS),
    },
    ConfigSpec {
        key: "stt.local.plans",
        value_type: ConfigValueType::PlanList,
    },
    ConfigSpec {
        key: "stt.local.max_chars",
        value_type: ConfigValueType::PosInt {
            min: 1,
            max: Some(10_000),
        },
    },
    ConfigSpec {
        key: "stt.local.usage_multiplier",
        value_type: ConfigValueType::PosFloat {
            min: 0.0,
            max: Some(1.0),
        },
    },
    ConfigSpec {
        key: "stt.cloud.credit_weight",
        value_type: ConfigValueType::PosFloat {
            min: 0.0,
            max: None,
        },
    },
    ConfigSpec {
        key: "stt.local.min_vram_mb",
        value_type: ConfigValueType::PosInt { min: 1, max: None },
    },
    ConfigSpec {
        key: "stt.local.benchmark_rtf_max",
        value_type: ConfigValueType::PosFloat {
            min: 0.0,
            max: None,
        },
    },
    ConfigSpec {
        key: "stt.local.watchdog_p95_ms",
        value_type: ConfigValueType::PosInt {
            min: 1,
            max: Some(60_000),
        },
    },
    ConfigSpec {
        key: "stt.local.retry_minutes",
        value_type: ConfigValueType::PosInt {
            min: 1,
            max: Some(1_440),
        },
    },
    ConfigSpec {
        key: "translate.provider",
        value_type: ConfigValueType::Enum(TRANSLATE_PROVIDERS),
    },
    ConfigSpec {
        key: "translate.fallback",
        value_type: ConfigValueType::Enum(TRANSLATE_FALLBACKS),
    },
    ConfigSpec {
        key: "translate.deepl.enabled_targets",
        value_type: ConfigValueType::LanguageCodeList,
    },
    ConfigSpec {
        key: "tts.provider",
        value_type: ConfigValueType::Enum(TTS_PROVIDERS),
    },
    ConfigSpec {
        key: "tts.fallback",
        value_type: ConfigValueType::Enum(TTS_FALLBACKS),
    },
    ConfigSpec {
        key: "tts.local.plans",
        value_type: ConfigValueType::PlanList,
    },
    ConfigSpec {
        key: "tts.local.usage_multiplier",
        value_type: ConfigValueType::PosFloat {
            min: 0.0,
            max: Some(1.0),
        },
    },
    ConfigSpec {
        key: "tts.cloud.credit_weight",
        value_type: ConfigValueType::PosFloat {
            min: 0.0,
            max: None,
        },
    },
    ConfigSpec {
        key: "tts.local.min_vram_mb",
        value_type: ConfigValueType::PosInt { min: 1, max: None },
    },
    ConfigSpec {
        key: "tts.local.benchmark_rtf_max",
        value_type: ConfigValueType::PosFloat {
            min: 0.0,
            max: None,
        },
    },
    ConfigSpec {
        key: "tts.local.watchdog_p95_ms",
        value_type: ConfigValueType::PosInt {
            min: 1,
            max: Some(60_000),
        },
    },
    ConfigSpec {
        key: "tts.local.retry_minutes",
        value_type: ConfigValueType::PosInt {
            min: 1,
            max: Some(1_440),
        },
    },
    ConfigSpec {
        key: "stream.max_bitrate",
        value_type: ConfigValueType::PosInt {
            min: 1_000,
            max: Some(100_000_000),
        },
    },
    ConfigSpec {
        key: "vad.enabled",
        value_type: ConfigValueType::Bool,
    },
    ConfigSpec {
        key: "dist.service.plans",
        value_type: ConfigValueType::PlanList,
    },
    ConfigSpec {
        key: "dist.cloud.plans",
        value_type: ConfigValueType::PlanList,
    },
    ConfigSpec {
        key: "integration.redirect_base",
        value_type: ConfigValueType::Url { allow_empty: false },
    },
    ConfigSpec {
        key: "providers.streamlabs.test_url",
        value_type: ConfigValueType::Url { allow_empty: false },
    },
    ConfigSpec {
        key: "providers.youtube.test_url",
        value_type: ConfigValueType::Url { allow_empty: false },
    },
    ConfigSpec {
        key: "integration.frontend_base",
        value_type: ConfigValueType::Url { allow_empty: true },
    },
    ConfigSpec {
        key: "billing.plan.free.stripe_price_id",
        value_type: ConfigValueType::FreeText { max_len: 200 },
    },
    ConfigSpec {
        key: "billing.plan.starter.stripe_price_id",
        value_type: ConfigValueType::FreeText { max_len: 200 },
    },
    ConfigSpec {
        key: "billing.plan.pro.stripe_price_id",
        value_type: ConfigValueType::FreeText { max_len: 200 },
    },
    ConfigSpec {
        key: "billing.plan.streamer.stripe_price_id",
        value_type: ConfigValueType::FreeText { max_len: 200 },
    },
    ConfigSpec {
        key: "billing.success_url",
        value_type: ConfigValueType::Url { allow_empty: false },
    },
    ConfigSpec {
        key: "billing.cancel_url",
        value_type: ConfigValueType::Url { allow_empty: false },
    },
    ConfigSpec {
        key: "log.level",
        value_type: ConfigValueType::Enum(LOG_LEVELS),
    },
    ConfigSpec {
        key: "overlay.max_subscribers",
        value_type: ConfigValueType::PosInt {
            min: 1,
            max: Some(1_000_000),
        },
    },
    ConfigSpec {
        key: "overlay.snapshot_lines",
        value_type: ConfigValueType::PosInt {
            min: 0,
            max: Some(1_000),
        },
    },
    ConfigSpec {
        key: "ext.twitch.enabled",
        value_type: ConfigValueType::Bool,
    },
    ConfigSpec {
        key: "ext.twitch.default_lang",
        value_type: ConfigValueType::FreeText { max_len: 10 },
    },
    ConfigSpec {
        key: "ext.twitch.max_viewers_per_channel",
        value_type: ConfigValueType::PosInt {
            min: 1,
            max: Some(1_000_000),
        },
    },
    ConfigSpec {
        key: "ext.twitch.state_rate_per_min",
        value_type: ConfigValueType::PosInt {
            min: 1,
            max: Some(10_000),
        },
    },
    ConfigSpec {
        key: "providers.mock_fallback",
        value_type: ConfigValueType::Bool,
    },
    ConfigSpec {
        key: "mail.smtp.tls",
        value_type: ConfigValueType::Enum(SMTP_TLS_MODES),
    },
    ConfigSpec {
        key: "auth.email_2fa",
        value_type: ConfigValueType::Enum(EMAIL_2FA_MODES),
    },
    ConfigSpec {
        key: "auth.default_plan",
        value_type: ConfigValueType::PlanName,
    },
    ConfigSpec {
        key: "auth.default_locale",
        value_type: ConfigValueType::FreeText { max_len: 10 },
    },
    ConfigSpec {
        key: "auth.login.rate_limit_per_min",
        value_type: ConfigValueType::PosInt {
            min: 1,
            max: Some(10_000),
        },
    },
    ConfigSpec {
        key: "auth.signup.rate_limit_per_min",
        value_type: ConfigValueType::PosInt {
            min: 1,
            max: Some(10_000),
        },
    },
    ConfigSpec {
        key: "auth.oauth_redirect_base",
        value_type: ConfigValueType::Url { allow_empty: false },
    },
    ConfigSpec {
        key: "founding.gate.enabled",
        value_type: ConfigValueType::Bool,
    },
    ConfigSpec {
        key: "promo.founding.enabled",
        value_type: ConfigValueType::Bool,
    },
    ConfigSpec {
        key: "promo.founding.message",
        value_type: ConfigValueType::FreeText { max_len: 2_000 },
    },
    ConfigSpec {
        key: "cost.stt.usd_per_hour",
        value_type: ConfigValueType::PosFloat {
            min: 0.0,
            max: None,
        },
    },
    ConfigSpec {
        key: "cost.translate.usd_per_hour",
        value_type: ConfigValueType::PosFloat {
            min: 0.0,
            max: None,
        },
    },
    ConfigSpec {
        key: "legal.terms.html",
        value_type: ConfigValueType::FreeText { max_len: 200_000 },
    },
    ConfigSpec {
        key: "legal.terms.version",
        value_type: ConfigValueType::FreeText { max_len: 64 },
    },
    ConfigSpec {
        key: "legal.privacy.html",
        value_type: ConfigValueType::FreeText { max_len: 200_000 },
    },
    ConfigSpec {
        key: "legal.privacy.version",
        value_type: ConfigValueType::FreeText { max_len: 64 },
    },
    ConfigSpec {
        key: "legal.kvkk.html",
        value_type: ConfigValueType::FreeText { max_len: 200_000 },
    },
    ConfigSpec {
        key: "legal.kvkk.version",
        value_type: ConfigValueType::FreeText { max_len: 64 },
    },
    ConfigSpec {
        key: "legal.cookies.html",
        value_type: ConfigValueType::FreeText { max_len: 200_000 },
    },
    ConfigSpec {
        key: "legal.cookies.version",
        value_type: ConfigValueType::FreeText { max_len: 64 },
    },
    ConfigSpec {
        key: "legal.dpa.html",
        value_type: ConfigValueType::FreeText { max_len: 200_000 },
    },
    ConfigSpec {
        key: "legal.dpa.version",
        value_type: ConfigValueType::FreeText { max_len: 64 },
    },
    ConfigSpec {
        key: "legal.communications.html",
        value_type: ConfigValueType::FreeText { max_len: 200_000 },
    },
    ConfigSpec {
        key: "legal.communications.version",
        value_type: ConfigValueType::FreeText { max_len: 64 },
    },
    ConfigSpec {
        key: "company.legal_name",
        value_type: ConfigValueType::FreeText { max_len: 500 },
    },
    ConfigSpec {
        key: "company.registration_number",
        value_type: ConfigValueType::FreeText { max_len: 500 },
    },
    ConfigSpec {
        key: "company.registered_address",
        value_type: ConfigValueType::FreeText { max_len: 500 },
    },
    ConfigSpec {
        key: "cors.allowed_origins",
        value_type: ConfigValueType::OriginList,
    },
];

/// Bir anahtarın spec'ini bulur. Bulunamazsa (henüz registry'ye taşınmamış
/// veya `attacker.key` gibi hiç var olmayan bir anahtar) `None` — çağıran
/// bunu "ek validasyon yok" olarak yorumlar; anahtarın gerçekten var olup
/// olmadığı zaten `ServiceConfig::set_existing`'in kendi 404 kontrolüdür.
pub fn find_spec(key: &str) -> Option<&'static ConfigSpec> {
    CONFIG_SPECS.iter().find(|spec| spec.key == key)
}

/// Validasyon için gereken bağlam: ortam (production kuralları) + bilinen
/// plan adları (CSV/tekil plan referanslarını doğrulamak için).
pub struct ValidationContext<'a> {
    pub environment: Environment,
    pub known_plans: &'a [String],
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("expected true/false, got {0:?}")]
    InvalidBool(String),
    #[error("expected a whole number, got {0:?}")]
    NotAnInteger(String),
    #[error("expected a number, got {0:?}")]
    NotANumber(String),
    #[error("value {value} is out of range [{min}, {max}]")]
    OutOfRange {
        value: String,
        min: String,
        max: String,
    },
    #[error("unknown value {value:?}, expected one of {allowed:?}")]
    UnknownEnumValue {
        value: String,
        allowed: &'static [&'static str],
    },
    #[error("malformed URL: {0:?}")]
    MalformedUrl(String),
    #[error("insecure URL scheme not allowed in production: {0:?} (use https/wss)")]
    InsecureUrlInProduction(String),
    #[error("malformed origin: {0:?}")]
    MalformedOrigin(String),
    #[error("wildcard origin (\"*\") is not allowed")]
    WildcardOrigin,
    #[error("unknown plan: {0:?}")]
    UnknownPlan(String),
    #[error("value too long (max {max} chars, got {actual})")]
    TooLong { max: usize, actual: usize },
    #[error("unknown language code: {0:?}")]
    UnknownLanguageCode(String),
    #[error("providers.mock_fallback cannot be enabled in production")]
    MockFallbackInProduction,
    #[error("auth.email_2fa cannot be 'off' in production")]
    Weak2faInProduction,
    #[error("plan name must be a non-empty lowercase slug (letters, digits, '-'): {0:?}")]
    InvalidPlanSlug(String),
    #[error("plan price_cents must not be negative: {0}")]
    NegativePlanPrice(i64),
    #[error("plan field {field} must be -1 (unlimited) or a non-negative number, got {value}")]
    InvalidPlanLimit { field: &'static str, value: i64 },
}

fn parse_bool(value: &str) -> Result<(), ValidationError> {
    match value {
        "true" | "false" => Ok(()),
        other => Err(ValidationError::InvalidBool(other.to_string())),
    }
}

fn parse_pos_int(value: &str, min: i64, max: Option<i64>) -> Result<(), ValidationError> {
    let parsed: i64 = value
        .parse()
        .map_err(|_| ValidationError::NotAnInteger(value.to_string()))?;
    let upper = max.unwrap_or(i64::MAX);
    if parsed < min || parsed > upper {
        return Err(ValidationError::OutOfRange {
            value: value.to_string(),
            min: min.to_string(),
            max: max
                .map(|m| m.to_string())
                .unwrap_or_else(|| "∞".to_string()),
        });
    }
    Ok(())
}

fn parse_pos_float(value: &str, min: f64, max: Option<f64>) -> Result<(), ValidationError> {
    let parsed: f64 = value
        .parse()
        .map_err(|_| ValidationError::NotANumber(value.to_string()))?;
    if !parsed.is_finite() {
        return Err(ValidationError::NotANumber(value.to_string()));
    }
    let upper = max.unwrap_or(f64::INFINITY);
    if parsed < min || parsed > upper {
        return Err(ValidationError::OutOfRange {
            value: value.to_string(),
            min: min.to_string(),
            max: max
                .map(|m| m.to_string())
                .unwrap_or_else(|| "∞".to_string()),
        });
    }
    Ok(())
}

fn parse_enum(value: &str, allowed: &'static [&'static str]) -> Result<(), ValidationError> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(ValidationError::UnknownEnumValue {
            value: value.to_string(),
            allowed,
        })
    }
}

/// `http(s)`/`ws(s)` mutlak URL doğrular; production'da güvensiz şema
/// (`http`/`ws`) reddedilir.
fn parse_url(value: &str, allow_empty: bool, env: Environment) -> Result<(), ValidationError> {
    if value.is_empty() {
        return if allow_empty {
            Ok(())
        } else {
            Err(ValidationError::MalformedUrl(value.to_string()))
        };
    }
    let parsed =
        reqwest::Url::parse(value).map_err(|_| ValidationError::MalformedUrl(value.to_string()))?;
    match parsed.scheme() {
        "https" | "wss" => Ok(()),
        "http" | "ws" if env == Environment::Development => Ok(()),
        "http" | "ws" => Err(ValidationError::InsecureUrlInProduction(value.to_string())),
        _ => Err(ValidationError::MalformedUrl(value.to_string())),
    }
}

/// Virgülle ayrılmış origin listesi — her origin `scheme://host[:port]`
/// biçiminde olmalı, path/query/fragment taşımamalı, `*` yasak.
fn parse_origin_list(value: &str) -> Result<(), ValidationError> {
    for raw in value.split(',') {
        let origin = raw.trim();
        if origin.is_empty() {
            continue;
        }
        if origin == "*" {
            return Err(ValidationError::WildcardOrigin);
        }
        let parsed = reqwest::Url::parse(origin)
            .map_err(|_| ValidationError::MalformedOrigin(origin.to_string()))?;
        let is_bare_origin = matches!(parsed.scheme(), "http" | "https")
            && parsed.host().is_some()
            && (parsed.path() == "/" || parsed.path().is_empty())
            && parsed.query().is_none()
            && parsed.fragment().is_none();
        if !is_bare_origin {
            return Err(ValidationError::MalformedOrigin(origin.to_string()));
        }
    }
    Ok(())
}

fn parse_plan_list(value: &str, known_plans: &[String]) -> Result<(), ValidationError> {
    for raw in value.split(',') {
        let name = raw.trim();
        if name.is_empty() {
            continue;
        }
        if !known_plans.iter().any(|p| p == name) {
            return Err(ValidationError::UnknownPlan(name.to_string()));
        }
    }
    Ok(())
}

fn parse_plan_name(value: &str, known_plans: &[String]) -> Result<(), ValidationError> {
    if value.trim().is_empty() || !known_plans.iter().any(|p| p == value) {
        return Err(ValidationError::UnknownPlan(value.to_string()));
    }
    Ok(())
}

fn parse_language_code_list(value: &str) -> Result<(), ValidationError> {
    for raw in value.split(',') {
        let code = raw.trim();
        if code.is_empty() {
            continue;
        }
        let known = crate::translate_languages::DEEPL_TARGET_LANGUAGES
            .iter()
            .any(|lang| lang.code.eq_ignore_ascii_case(code));
        if !known {
            return Err(ValidationError::UnknownLanguageCode(code.to_string()));
        }
    }
    Ok(())
}

fn parse_free_text(value: &str, max_len: usize) -> Result<(), ValidationError> {
    if value.chars().count() > max_len {
        return Err(ValidationError::TooLong {
            max: max_len,
            actual: value.chars().count(),
        });
    }
    Ok(())
}

/// Bilinen bir anahtarın yeni değerini, tipine ve production kısıtlarına göre
/// doğrular. Anahtarın registry'de spec'i yoksa ek validasyon uygulanmaz
/// (bkz. [`find_spec`] dokümantasyonu).
pub fn validate_config_value(
    key: &str,
    value: &str,
    ctx: &ValidationContext,
) -> Result<(), ValidationError> {
    let Some(spec) = find_spec(key) else {
        return Ok(());
    };

    match spec.value_type {
        ConfigValueType::Bool => parse_bool(value)?,
        ConfigValueType::PosInt { min, max } => parse_pos_int(value, min, max)?,
        ConfigValueType::PosFloat { min, max } => parse_pos_float(value, min, max)?,
        ConfigValueType::Enum(allowed) => parse_enum(value, allowed)?,
        ConfigValueType::Url { allow_empty } => parse_url(value, allow_empty, ctx.environment)?,
        ConfigValueType::OriginList => parse_origin_list(value)?,
        ConfigValueType::PlanList => parse_plan_list(value, ctx.known_plans)?,
        ConfigValueType::PlanName => parse_plan_name(value, ctx.known_plans)?,
        ConfigValueType::LanguageCodeList => parse_language_code_list(value)?,
        ConfigValueType::FreeText { max_len } => parse_free_text(value, max_len)?,
    }

    // Değer tipinden bağımsız, anahtara özel production kısıtları.
    if ctx.environment == Environment::Production {
        if key == "providers.mock_fallback" && value == "true" {
            return Err(ValidationError::MockFallbackInProduction);
        }
        if key == "auth.email_2fa" && value == "off" {
            return Err(ValidationError::Weak2faInProduction);
        }
    }

    Ok(())
}

fn is_valid_plan_slug(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !name.starts_with('-')
        && !name.ends_with('-')
}

/// Plan §13: "limitler yalnız `-1` veya pozitif" — `-1` (sınırsız sentinel)
/// veya kesin pozitif (`>0`) kabul edilir; `0` dahil diğer her değer
/// reddedilir (bir planın "0 saat/0 dil/0 platform" olması anlamsızdır).
fn validate_plan_limit(field: &'static str, value: i64) -> Result<(), ValidationError> {
    if value == -1 || value > 0 {
        Ok(())
    } else {
        Err(ValidationError::InvalidPlanLimit { field, value })
    }
}

/// Bir `PlanDefinition` mutasyonunu kalıcılaştırmadan önce doğrular (BH-02:
/// "plan validation ekle"). "Kullanımda olan planın silinmesini engelle"
/// kuralı burada YOK — bu backend'de plan silme HTTP uç noktası hiç
/// yok (`PUT /admin/plans` yalnız upsert'tir, `PlanRegistry::remove` yalnız
/// internal rollback'te kullanılır); silme uç noktası eklenene kadar
/// uygulanacak bir mutasyon yolu bulunmuyor.
pub fn validate_plan(plan: &PlanDefinition) -> Result<(), ValidationError> {
    if !is_valid_plan_slug(&plan.name) {
        return Err(ValidationError::InvalidPlanSlug(plan.name.clone()));
    }
    if plan.price_cents < 0 {
        return Err(ValidationError::NegativePlanPrice(plan.price_cents));
    }
    validate_plan_limit("monthly_hours", plan.monthly_hours)?;
    validate_plan_limit("max_languages", plan.max_languages)?;
    validate_plan_limit("max_platforms", plan.max_platforms)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(env: Environment) -> ValidationContext<'static> {
        // 'static known_plans: testler gerçek PlanRegistry seed adlarını kullanır.
        static PLANS: &[&str] = &["free", "starter", "pro", "streamer", "founding"];
        ValidationContext {
            environment: env,
            known_plans: Box::leak(
                PLANS
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            ),
        }
    }

    #[test]
    fn unknown_provider_is_rejected() {
        let err = validate_config_value("stt.provider", "ghost", &ctx(Environment::Development))
            .unwrap_err();
        assert!(matches!(err, ValidationError::UnknownEnumValue { .. }));
    }

    #[test]
    fn known_provider_is_accepted() {
        assert!(validate_config_value(
            "stt.provider",
            "assemblyai",
            &ctx(Environment::Development)
        )
        .is_ok());
    }

    #[test]
    fn negative_and_zero_limit_is_rejected() {
        for bad in ["-1", "0"] {
            let err = validate_config_value(
                "auth.login.rate_limit_per_min",
                bad,
                &ctx(Environment::Development),
            )
            .unwrap_err();
            assert!(
                matches!(err, ValidationError::OutOfRange { .. }),
                "value {bad} should be rejected, got {err:?}"
            );
        }
    }

    #[test]
    fn malformed_url_is_rejected() {
        let err = validate_config_value(
            "auth.oauth_redirect_base",
            "not a url",
            &ctx(Environment::Development),
        )
        .unwrap_err();
        assert!(matches!(err, ValidationError::MalformedUrl(_)));
    }

    #[test]
    fn malformed_origin_is_rejected() {
        let err = validate_config_value(
            "cors.allowed_origins",
            "http://ok.example, not-an-origin",
            &ctx(Environment::Development),
        )
        .unwrap_err();
        assert!(matches!(err, ValidationError::MalformedOrigin(_)));
    }

    #[test]
    fn wildcard_origin_is_rejected() {
        let err =
            validate_config_value("cors.allowed_origins", "*", &ctx(Environment::Development))
                .unwrap_err();
        assert_eq!(err, ValidationError::WildcardOrigin);
    }

    #[test]
    fn http_provider_url_is_rejected_in_production_but_allowed_in_dev() {
        assert!(validate_config_value(
            "auth.oauth_redirect_base",
            "http://localhost:8080/api/auth/oauth",
            &ctx(Environment::Development),
        )
        .is_ok());

        let err = validate_config_value(
            "auth.oauth_redirect_base",
            "http://localhost:8080/api/auth/oauth",
            &ctx(Environment::Production),
        )
        .unwrap_err();
        assert!(matches!(err, ValidationError::InsecureUrlInProduction(_)));

        assert!(validate_config_value(
            "auth.oauth_redirect_base",
            "https://api.strovak.com/api/auth/oauth",
            &ctx(Environment::Production),
        )
        .is_ok());
    }

    #[test]
    fn mock_fallback_is_rejected_in_production_only() {
        assert!(validate_config_value(
            "providers.mock_fallback",
            "true",
            &ctx(Environment::Development)
        )
        .is_ok());
        let err = validate_config_value(
            "providers.mock_fallback",
            "true",
            &ctx(Environment::Production),
        )
        .unwrap_err();
        assert_eq!(err, ValidationError::MockFallbackInProduction);
        assert!(validate_config_value(
            "providers.mock_fallback",
            "false",
            &ctx(Environment::Production)
        )
        .is_ok());
    }

    #[test]
    fn weak_2fa_is_rejected_in_production_only() {
        assert!(
            validate_config_value("auth.email_2fa", "off", &ctx(Environment::Development)).is_ok()
        );
        let err = validate_config_value("auth.email_2fa", "off", &ctx(Environment::Production))
            .unwrap_err();
        assert_eq!(err, ValidationError::Weak2faInProduction);
        assert!(
            validate_config_value("auth.email_2fa", "required", &ctx(Environment::Production))
                .is_ok()
        );
    }

    #[test]
    fn unknown_plan_in_plan_list_is_rejected() {
        let err = validate_config_value(
            "dist.service.plans",
            "free,ghost-plan",
            &ctx(Environment::Development),
        )
        .unwrap_err();
        assert_eq!(err, ValidationError::UnknownPlan("ghost-plan".to_string()));
    }

    #[test]
    fn known_plan_list_is_accepted() {
        assert!(validate_config_value(
            "dist.service.plans",
            "free, pro",
            &ctx(Environment::Development),
        )
        .is_ok());
    }

    #[test]
    fn empty_plan_list_is_accepted() {
        // Boş liste = "hiçbir plan bu dağıtım yöntemine erişemiyor" — geçerli
        // bir admin kararı, config validasyonunun engellemesi gereken bir şey
        // değil.
        assert!(
            validate_config_value("dist.service.plans", "", &ctx(Environment::Development)).is_ok()
        );
    }

    #[test]
    fn unknown_default_plan_is_rejected() {
        let err = validate_config_value(
            "auth.default_plan",
            "ghost-plan",
            &ctx(Environment::Development),
        )
        .unwrap_err();
        assert_eq!(err, ValidationError::UnknownPlan("ghost-plan".to_string()));
    }

    #[test]
    fn unknown_language_code_is_rejected() {
        let err = validate_config_value(
            "translate.deepl.enabled_targets",
            "en,xx",
            &ctx(Environment::Development),
        )
        .unwrap_err();
        assert_eq!(err, ValidationError::UnknownLanguageCode("xx".to_string()));
    }

    #[test]
    fn unregistered_key_has_no_extra_validation() {
        // `attacker.key` registry'de yok — set_existing zaten 404 döner,
        // validate_config_value burada sessizce Ok döner (double-gate değil).
        assert!(
            validate_config_value("attacker.key", "anything", &ctx(Environment::Development))
                .is_ok()
        );
    }

    #[test]
    fn bool_field_rejects_non_bool_value() {
        let err = validate_config_value("vad.enabled", "yes", &ctx(Environment::Development))
            .unwrap_err();
        assert!(matches!(err, ValidationError::InvalidBool(_)));
    }

    #[test]
    fn all_seeded_keys_have_a_spec() {
        let cfg = crate::config::ServiceConfig::with_defaults();
        for key in cfg.snapshot().keys() {
            assert!(
                find_spec(key).is_some(),
                "seeded config key missing a ConfigSpec entry: {key}"
            );
        }
    }

    #[test]
    fn all_spec_keys_are_seeded() {
        let cfg = crate::config::ServiceConfig::with_defaults();
        for spec in CONFIG_SPECS {
            assert!(
                cfg.get(spec.key).is_some(),
                "ConfigSpec references a key that isn't seeded: {}",
                spec.key
            );
        }
    }

    #[test]
    fn valid_plan_is_accepted() {
        assert!(validate_plan(&PlanDefinition {
            name: "starter".into(),
            price_cents: 999,
            monthly_hours: 30,
            max_languages: -1,
            max_platforms: 3,
            has_watermark: false,
            has_voice_clone: false,
        })
        .is_ok());
    }

    #[test]
    fn negative_plan_price_is_rejected() {
        let err = validate_plan(&PlanDefinition {
            name: "starter".into(),
            price_cents: -100,
            monthly_hours: 30,
            max_languages: 3,
            max_platforms: 3,
            has_watermark: false,
            has_voice_clone: false,
        })
        .unwrap_err();
        assert_eq!(err, ValidationError::NegativePlanPrice(-100));
    }

    #[test]
    fn invalid_plan_limit_is_rejected_but_unlimited_sentinel_is_ok() {
        let err = validate_plan(&PlanDefinition {
            name: "starter".into(),
            price_cents: 0,
            monthly_hours: -2,
            max_languages: 3,
            max_platforms: 3,
            has_watermark: false,
            has_voice_clone: false,
        })
        .unwrap_err();
        assert_eq!(
            err,
            ValidationError::InvalidPlanLimit {
                field: "monthly_hours",
                value: -2
            }
        );

        // -1 (sınırsız) her zaman geçerli.
        assert!(validate_plan(&PlanDefinition {
            name: "streamer".into(),
            price_cents: 9999,
            monthly_hours: -1,
            max_languages: -1,
            max_platforms: -1,
            has_watermark: false,
            has_voice_clone: true,
        })
        .is_ok());
    }

    #[test]
    fn zero_plan_limit_is_rejected() {
        // "limitler yalnız -1 veya pozitif" — 0 bir planın "0 saat/0 dil"
        // olması anlamına gelir, bu anlamsızdır ve reddedilmelidir.
        let err = validate_plan(&PlanDefinition {
            name: "starter".into(),
            price_cents: 0,
            monthly_hours: 10,
            max_languages: 0,
            max_platforms: 3,
            has_watermark: false,
            has_voice_clone: false,
        })
        .unwrap_err();
        assert_eq!(
            err,
            ValidationError::InvalidPlanLimit {
                field: "max_languages",
                value: 0
            }
        );
    }

    #[test]
    fn empty_or_malformed_plan_slug_is_rejected() {
        for bad in ["", "Free", "free plan", "free_plan", "-free", "free-"] {
            let err = validate_plan(&PlanDefinition {
                name: bad.into(),
                price_cents: 0,
                monthly_hours: 1,
                max_languages: 1,
                max_platforms: 1,
                has_watermark: false,
                has_voice_clone: false,
            })
            .unwrap_err();
            assert!(
                matches!(err, ValidationError::InvalidPlanSlug(_)),
                "slug {bad:?} should be rejected"
            );
        }
    }
}
