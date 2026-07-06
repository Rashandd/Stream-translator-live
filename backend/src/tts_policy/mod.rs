//! Yerel TTS politikası (Faz 2.2b) — `stt_policy` ile birebir aynı desen.
//!
//! `GET /api/tts/policy`: SDK (pairing token'ı) veya panel, yerel TTS'in
//! (XTTS-v2) bu kullanıcının planı için açık olup olmadığını, kota
//! çarpanını, donanım eşiklerini ve watchdog parametrelerini tek çağrıda
//! öğrenir. Tüm değerler `ServiceConfig`'ten okunur → admin panelden
//! restart'sız ayarlanır ve SDK sürümü yayınlamadan politika değiştirilebilir.

use std::sync::Arc;

use axum::{extract::State, Json};
use serde::Serialize;

use crate::auth::authz::AuthUser;
use crate::config::ServiceConfig;
use crate::integrations::oauth::plan_allowed;
use crate::sdk_link::{BenchmarkReport, TtsMode, TtsSettingsStore};

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

/// `GET /api/tts/policy` yanıtı.
#[derive(Debug, Serialize)]
pub struct TtsPolicy {
    /// Kullanıcının planı `tts.local.plans` listesinde mi?
    pub local_allowed: bool,
    /// Yerel sentez yolunun kota çarpanı (`tts.local.usage_multiplier`).
    pub usage_multiplier: f64,
    pub requirements: Requirements,
    pub watchdog: Watchdog,
}

/// Config'ten sayısal değer okur; yok/bozuksa seed varsayılanına düşer.
fn num<T: std::str::FromStr>(config: &ServiceConfig, key: &str, default: T) -> T {
    config
        .get(key)
        .and_then(|v| v.parse::<T>().ok())
        .unwrap_or(default)
}

/// `tts.local.usage_multiplier` değerini okur — bozuk/aralık-dışı değerde
/// indirimsiz `1.0`'a düşer (`stt_policy::usage_multiplier` ile aynı mantık).
pub fn usage_multiplier(config: &ServiceConfig) -> f64 {
    config
        .get("tts.local.usage_multiplier")
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|m| (0.0..=1.0).contains(m))
        .unwrap_or(1.0)
}

/// Politikayı config'ten kurar (handler + testlerin ortak çekirdeği).
pub fn policy_for_plan(config: &ServiceConfig, plan: &str) -> TtsPolicy {
    let plans_csv = config.get("tts.local.plans").unwrap_or_default();
    TtsPolicy {
        local_allowed: plan_allowed(&plans_csv, plan),
        usage_multiplier: usage_multiplier(config),
        requirements: Requirements {
            min_vram_mb: num(config, "tts.local.min_vram_mb", 6144),
            benchmark_rtf_max: num(config, "tts.local.benchmark_rtf_max", 0.5),
        },
        watchdog: Watchdog {
            p95_ms: num(config, "tts.local.watchdog_p95_ms", 2000),
            retry_minutes: num(config, "tts.local.retry_minutes", 5),
        },
    }
}

/// `GET /api/tts/policy` yanıtı: global politika + kullanıcının mod tercihi
/// ve son benchmark sonucu — SDK tek çağrıda her şeyi öğrenir.
#[derive(Debug, Serialize)]
pub struct TtsPolicyResponse {
    #[serde(flatten)]
    pub policy: TtsPolicy,
    pub mode: TtsMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub benchmark: Option<BenchmarkReport>,
}

/// `GET /api/tts/policy` — herhangi bir geçerli JWT (yayıncı paneli veya
/// pairing'li SDK) kendi planının politikasını ve mod/benchmark durumunu okur.
pub async fn get_policy(
    user: AuthUser,
    State(config): State<Arc<ServiceConfig>>,
    State(settings): State<Arc<TtsSettingsStore>>,
) -> Json<TtsPolicyResponse> {
    let user_settings = settings.get(&user.claims.sub);
    Json(TtsPolicyResponse {
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
        let p = policy_for_plan(&config, "pro");
        assert!(p.local_allowed);
        assert_eq!(p.usage_multiplier, 0.5);
        assert_eq!(p.requirements.min_vram_mb, 6144);
        assert_eq!(p.requirements.benchmark_rtf_max, 0.5);
        assert_eq!(p.watchdog.p95_ms, 2000);
        assert_eq!(p.watchdog.retry_minutes, 5);
    }

    #[test]
    fn free_plan_is_not_allowed_by_default() {
        // tts.local.plans seed'i "pro,streamer" — free/starter dışarıda
        // (has_voice_clone zaten aynı ikiliyi kilitliyor).
        let config = ServiceConfig::with_defaults();
        assert!(!policy_for_plan(&config, "free").local_allowed);
        assert!(!policy_for_plan(&config, "starter").local_allowed);
        assert!(policy_for_plan(&config, "streamer").local_allowed);
    }

    #[test]
    fn plan_gating_reflected_in_local_allowed() {
        let config = ServiceConfig::with_defaults();
        config.set("tts.local.plans", "streamer");
        assert!(!policy_for_plan(&config, "pro").local_allowed);
        assert!(policy_for_plan(&config, "streamer").local_allowed);
    }

    #[test]
    fn broken_config_values_fall_back_to_defaults() {
        let config = ServiceConfig::with_defaults();
        config.set("tts.local.min_vram_mb", "çokça");
        config.set("tts.local.watchdog_p95_ms", "");
        let p = policy_for_plan(&config, "pro");
        assert_eq!(p.requirements.min_vram_mb, 6144);
        assert_eq!(p.watchdog.p95_ms, 2000);
    }

    #[test]
    fn usage_multiplier_reads_config_with_conservative_fallback() {
        let config = ServiceConfig::with_defaults();
        assert_eq!(usage_multiplier(&config), 0.5);
        config.set("tts.local.usage_multiplier", "0.75");
        assert_eq!(usage_multiplier(&config), 0.75);
        config.set("tts.local.usage_multiplier", "abc");
        assert_eq!(usage_multiplier(&config), 1.0);
        config.set("tts.local.usage_multiplier", "1.5");
        assert_eq!(usage_multiplier(&config), 1.0);
    }
}
