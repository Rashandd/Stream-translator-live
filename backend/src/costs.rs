//! Admin maliyet özeti (Faz 1.14) — `GET /admin/costs/summary`.
//!
//! Gerçek servis-bazlı $ harcaması yerine (henüz per-servis kullanım
//! ayrıştırılmıyor — `usage::UsageTracker` STT/çeviri/TTS'i tek havuzda
//! sayar) toplam kullanım saatini admin-ayarlanabilir $/saat oranlarıyla
//! çarpan bir tahmin döner. Aynı yanıtta plan gelirini de gösterir — admin
//! "maliyet kurtarıyor mu" sorusunu tek bakışta görür.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

use crate::auth::authz::AdminUser;
use crate::config::ServiceConfig;
use crate::plans::PlanRegistry;
use crate::usage::UsageTracker;
use crate::users::UserStore;

#[derive(Debug, Serialize, PartialEq)]
pub struct AdminCostSummary {
    pub period: String,
    pub user_count: usize,
    pub total_usage_hours: f64,
    pub stt_usd_per_hour: f64,
    pub translate_usd_per_hour: f64,
    /// Bu dönem tahmini toplam servis maliyeti (cent).
    pub estimated_cost_cents: i64,
    /// Şu an kayıtlı kullanıcıların planlarına göre tahmini aylık gelir
    /// (cent) — dönemsel değil, "bugün fatura kesilse ne olurdu" anlık görüntüsü.
    pub estimated_monthly_revenue_cents: i64,
}

fn cfg_f64(cfg: &ServiceConfig, key: &str, default: f64) -> f64 {
    cfg.get(key).and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// `GET /admin/costs/summary` (sadece admin).
pub async fn get_summary(
    _admin: AdminUser,
    State(usage): State<Arc<UsageTracker>>,
    State(users): State<Arc<UserStore>>,
    State(plans): State<Arc<PlanRegistry>>,
    State(cfg): State<Arc<ServiceConfig>>,
) -> Result<Json<AdminCostSummary>, (StatusCode, String)> {
    let stt_rate = cfg_f64(&cfg, "cost.stt.usd_per_hour", 0.05);
    let translate_rate = cfg_f64(&cfg, "cost.translate.usd_per_hour", 0.04);

    let total_hours = usage.total_seconds_this_period() as f64 / 3600.0;
    let estimated_cost_cents = (total_hours * (stt_rate + translate_rate) * 100.0).round() as i64;

    let records = users.persistent_list().await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "user service unavailable".to_string(),
        )
    })?;
    let revenue_cents: i64 = records
        .iter()
        .filter_map(|u| plans.get(&u.plan).map(|p| p.price_cents))
        .sum();

    Ok(Json(AdminCostSummary {
        period: UsageTracker::current_period(),
        user_count: records.len(),
        total_usage_hours: total_hours,
        stt_usd_per_hour: stt_rate,
        translate_usd_per_hour: translate_rate,
        estimated_cost_cents,
        estimated_monthly_revenue_cents: revenue_cents,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cfg_f64_falls_back_to_default_when_missing_or_unparseable() {
        let cfg = ServiceConfig::with_defaults();
        assert_eq!(cfg_f64(&cfg, "cost.stt.usd_per_hour", 0.05), 0.05);
        cfg.set("cost.stt.usd_per_hour", "not-a-number");
        assert_eq!(cfg_f64(&cfg, "cost.stt.usd_per_hour", 0.05), 0.05);
        cfg.set("cost.stt.usd_per_hour", "0.12");
        assert_eq!(cfg_f64(&cfg, "cost.stt.usd_per_hour", 0.05), 0.12);
    }
}
