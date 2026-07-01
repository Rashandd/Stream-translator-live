//! Plan tanımları (Faz 1.11) — fiyatlandırma sayfasındaki 4 plan.
//!
//! `docs/schema.md`'deki `plans` tablosuyla aynı alanlar. Admin panelden
//! değiştirilebilir olması gerektiği için (CLAUDE.md: "hiçbir şey hard-coded
//! olmaz") bellek içi bir registry olarak tutulur — Faz 1 iskeleti
//! `ServiceConfig` ile aynı desende; ileride `plans` tablosundan okunur/yazılır.

use std::collections::HashMap;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

/// Bir aboneli planın limitleri. `-1`, ilgili alanda sınırsız anlamına gelir.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanDefinition {
    pub name: String,
    pub price_cents: i64,
    pub monthly_hours: i64,
    pub max_languages: i64,
    pub max_platforms: i64,
    pub has_watermark: bool,
    pub has_voice_clone: bool,
}

/// Plan adı → tanım eşlemesini tutan registry.
pub struct PlanRegistry {
    plans: RwLock<HashMap<String, PlanDefinition>>,
}

impl PlanRegistry {
    /// Fiyatlandırma tablosundaki (`streamdubber_roadmap_final.md`) 4 planla kurar.
    pub fn with_defaults() -> Self {
        let defaults = vec![
            PlanDefinition {
                name: "free".into(),
                price_cents: 0,
                monthly_hours: 5,
                max_languages: 1,
                max_platforms: 1,
                has_watermark: true,
                has_voice_clone: false,
            },
            PlanDefinition {
                name: "starter".into(),
                price_cents: 999,
                monthly_hours: 20,
                max_languages: 3,
                max_platforms: 3,
                has_watermark: false,
                has_voice_clone: false,
            },
            PlanDefinition {
                name: "pro".into(),
                price_cents: 2499,
                monthly_hours: 40,
                max_languages: 10,
                max_platforms: -1,
                has_watermark: false,
                has_voice_clone: true,
            },
            PlanDefinition {
                name: "streamer".into(),
                price_cents: 4999,
                monthly_hours: 100,
                max_languages: -1,
                max_platforms: -1,
                has_watermark: false,
                has_voice_clone: true,
            },
        ];
        let map = defaults.into_iter().map(|p| (p.name.clone(), p)).collect();
        PlanRegistry {
            plans: RwLock::new(map),
        }
    }

    /// Bir planın tanımını okur. Bilinmeyen plan adı `None`.
    pub fn get(&self, name: &str) -> Option<PlanDefinition> {
        self.plans.read().ok()?.get(name).cloned()
    }

    /// Bir planı ekler/günceller (admin toggle). Restart gerekmez.
    pub fn set(&self, plan: PlanDefinition) {
        if let Ok(mut map) = self.plans.write() {
            map.insert(plan.name.clone(), plan);
        }
    }

    /// Tüm planların sıralı bir kopyası (admin panel listesi için).
    pub fn snapshot(&self) -> Vec<PlanDefinition> {
        let mut plans: Vec<PlanDefinition> = self
            .plans
            .read()
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default();
        plans.sort_by(|a, b| a.name.cmp(&b.name));
        plans
    }
}

impl Default for PlanRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_four_plans() {
        let reg = PlanRegistry::with_defaults();
        assert_eq!(reg.snapshot().len(), 4);
        let free = reg.get("free").unwrap();
        assert_eq!(free.monthly_hours, 5);
        assert!(free.has_watermark);
        let pro = reg.get("pro").unwrap();
        assert!(pro.has_voice_clone);
        assert_eq!(pro.max_platforms, -1);
    }

    #[test]
    fn unknown_plan_is_none() {
        let reg = PlanRegistry::with_defaults();
        assert!(reg.get("ghost").is_none());
    }

    #[test]
    fn set_overrides_without_restart() {
        let reg = PlanRegistry::with_defaults();
        reg.set(PlanDefinition {
            name: "free".into(),
            price_cents: 0,
            monthly_hours: 10,
            max_languages: 1,
            max_platforms: 1,
            has_watermark: true,
            has_voice_clone: false,
        });
        assert_eq!(reg.get("free").unwrap().monthly_hours, 10);
    }

    #[test]
    fn snapshot_is_sorted_by_name() {
        let reg = PlanRegistry::with_defaults();
        let names: Vec<String> = reg.snapshot().into_iter().map(|p| p.name).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }
}
