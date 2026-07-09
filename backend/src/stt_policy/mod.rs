//! Yerel STT politikası (Faz 2.6b — docs/local-stt-plan.md §2-3).
//!
//! `GET /api/stt/policy`: SDK (pairing token'ı) veya panel, yerel STT'nin bu
//! kullanıcının planı için açık olup olmadığını, kota çarpanını, donanım
//! eşiklerini ve watchdog parametrelerini tek çağrıda öğrenir. Tüm değerler
//! `ServiceConfig`'ten okunur → admin panelden restart'sız ayarlanır ve SDK
//! sürümü yayınlamadan politika değiştirilebilir.

use std::sync::Arc;

use axum::{extract::State, Json};
use serde::Serialize;

use crate::auth::authz::AuthUser;
use crate::config::ServiceConfig;
use crate::integrations::oauth::plan_allowed;
use crate::sdk_link::{BenchmarkReport, SttMode, SttSettingsStore};

/// Donanım/etkinleştirme eşikleri (SDK "Donanımımı test et" bunlara bakar).
#[derive(Debug, Serialize)]
pub struct Requirements {
    pub min_vram_mb: u64,
    pub benchmark_rtf_max: f64,
}

/// Yayın sırasında bulut fallback watchdog parametreleri.
#[derive(Debug, Serialize)]
pub struct Watchdog {
    pub p95_ms: u64,
    pub retry_minutes: u64,
}

/// `GET /api/stt/policy` yanıtı.
#[derive(Debug, Serialize)]
pub struct SttPolicy {
    /// Kullanıcının planı `stt.local.plans` listesinde mi?
    pub local_allowed: bool,
    /// Transcript yolunun kota çarpanı (`stt.local.usage_multiplier`).
    pub usage_multiplier: f64,
    pub requirements: Requirements,
    pub watchdog: Watchdog,
    /// Tek transcript satırı karakter sınırı (`stt.local.max_chars`).
    pub max_chars: u64,
}

/// Config'ten sayısal değer okur; yok/bozuksa seed varsayılanına düşer —
/// admin yanlış değer girse bile SDK asla parametresiz kalmaz.
fn num<T: std::str::FromStr>(config: &ServiceConfig, key: &str, default: T) -> T {
    config
        .get(key)
        .and_then(|v| v.parse::<T>().ok())
        .unwrap_or(default)
}

/// `stt.local.usage_multiplier` değerini okur. Yok/bozuk/0..1 aralığı dışı
/// değerde indirimsiz `1.0`'a düşer — faturalama için muhafazakâr varsayılan.
/// (ws transcript yolu ve policy yanıtı aynı fonksiyonu kullanır.)
pub fn usage_multiplier(config: &ServiceConfig) -> f64 {
    config
        .get("stt.local.usage_multiplier")
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|m| (0.0..=1.0).contains(m))
        .unwrap_or(1.0)
}

/// `stt.cloud.credit_weight` değerini okur — backend'in kendi işlediği
/// (indirimsiz) STT saatinin kredi ağırlığı. Farklı self-hosted modeller
/// (ör. faster-whisper vs. Voxtral) farklı GPU $/saat maliyeti taşır; tek
/// global sabit `1.0` yerine bu, admin'in gerçek maliyete göre ayarladığı
/// çarpandır (roadmap "Fiyatlandırma — Kredi Sistemi"). Yok/bozuk/≤0
/// değerde eski sabit davranışa (`1.0`) düşer — varsayılan davranışı değiştirmez.
pub fn cloud_credit_weight(config: &ServiceConfig) -> f64 {
    config
        .get("stt.cloud.credit_weight")
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|w| *w > 0.0)
        .unwrap_or(1.0)
}

/// Politikayı config'ten kurar (handler + testlerin ortak çekirdeği).
pub fn policy_for_plan(config: &ServiceConfig, plan: &str) -> SttPolicy {
    let plans_csv = config.get("stt.local.plans").unwrap_or_default();
    SttPolicy {
        local_allowed: plan_allowed(&plans_csv, plan),
        usage_multiplier: usage_multiplier(config),
        requirements: Requirements {
            min_vram_mb: num(config, "stt.local.min_vram_mb", 6144),
            benchmark_rtf_max: num(config, "stt.local.benchmark_rtf_max", 0.5),
        },
        watchdog: Watchdog {
            p95_ms: num(config, "stt.local.watchdog_p95_ms", 1500),
            retry_minutes: num(config, "stt.local.retry_minutes", 5),
        },
        max_chars: num(config, "stt.local.max_chars", 500),
    }
}

/// `GET /api/stt/policy` yanıtı: global politika + kullanıcının mod tercihi
/// ve son benchmark sonucu — SDK tek çağrıda her şeyi öğrenir.
#[derive(Debug, Serialize)]
pub struct SttPolicyResponse {
    #[serde(flatten)]
    pub policy: SttPolicy,
    pub mode: SttMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub benchmark: Option<BenchmarkReport>,
}

/// `GET /api/stt/policy` — herhangi bir geçerli JWT (yayıncı paneli veya
/// pairing'li SDK) kendi planının politikasını ve mod/benchmark durumunu okur.
pub async fn get_policy(
    user: AuthUser,
    State(config): State<Arc<ServiceConfig>>,
    State(settings): State<Arc<SttSettingsStore>>,
) -> Json<SttPolicyResponse> {
    let user_settings = settings.get(&user.claims.sub);
    Json(SttPolicyResponse {
        policy: policy_for_plan(&config, &user.claims.plan),
        mode: user_settings.mode,
        benchmark: user_settings.benchmark,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_reflects_seed_defaults() {
        let config = ServiceConfig::with_defaults();
        let p = policy_for_plan(&config, "free");
        assert!(p.local_allowed);
        assert_eq!(p.usage_multiplier, 0.5);
        assert_eq!(p.requirements.min_vram_mb, 6144);
        assert_eq!(p.requirements.benchmark_rtf_max, 0.5);
        assert_eq!(p.watchdog.p95_ms, 1500);
        assert_eq!(p.watchdog.retry_minutes, 5);
        assert_eq!(p.max_chars, 500);
    }

    #[test]
    fn plan_gating_reflected_in_local_allowed() {
        let config = ServiceConfig::with_defaults();
        config.set("stt.local.plans", "pro,streamer");
        assert!(!policy_for_plan(&config, "free").local_allowed);
        assert!(policy_for_plan(&config, "pro").local_allowed);
    }

    #[test]
    fn broken_config_values_fall_back_to_defaults() {
        let config = ServiceConfig::with_defaults();
        config.set("stt.local.min_vram_mb", "çokça");
        config.set("stt.local.watchdog_p95_ms", "");
        let p = policy_for_plan(&config, "free");
        assert_eq!(p.requirements.min_vram_mb, 6144);
        assert_eq!(p.watchdog.p95_ms, 1500);
    }

    #[test]
    fn usage_multiplier_reads_config_with_conservative_fallback() {
        let config = ServiceConfig::with_defaults();
        // Seed varsayılanı: 0.5.
        assert_eq!(usage_multiplier(&config), 0.5);
        config.set("stt.local.usage_multiplier", "0.75");
        assert_eq!(usage_multiplier(&config), 0.75);
        // Bozuk / aralık dışı değerler indirimsiz 1.0'a düşer.
        config.set("stt.local.usage_multiplier", "abc");
        assert_eq!(usage_multiplier(&config), 1.0);
        config.set("stt.local.usage_multiplier", "1.5");
        assert_eq!(usage_multiplier(&config), 1.0);
        config.set("stt.local.usage_multiplier", "-0.2");
        assert_eq!(usage_multiplier(&config), 1.0);
    }

    #[test]
    fn cloud_credit_weight_defaults_to_one_and_reads_config() {
        let config = ServiceConfig::with_defaults();
        assert_eq!(cloud_credit_weight(&config), 1.0);
        config.set("stt.cloud.credit_weight", "2.5");
        assert_eq!(cloud_credit_weight(&config), 2.5);
        // Bozuk / <=0 değerler eski sabit davranışa (1.0) düşer.
        config.set("stt.cloud.credit_weight", "abc");
        assert_eq!(cloud_credit_weight(&config), 1.0);
        config.set("stt.cloud.credit_weight", "0");
        assert_eq!(cloud_credit_weight(&config), 1.0);
        config.set("stt.cloud.credit_weight", "-1.0");
        assert_eq!(cloud_credit_weight(&config), 1.0);
    }
}
