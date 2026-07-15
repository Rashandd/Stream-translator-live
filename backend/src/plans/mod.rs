//! Plan tanımları (Faz 1.11) — fiyatlandırma sayfasındaki 4 plan.
//!
//! `docs/schema.md`'deki `plans` tablosuyla aynı alanlar. Admin panelden
//! değiştirilebilir olması gerektiği için (CLAUDE.md: "hiçbir şey hard-coded
//! olmaz") bir registry olarak tutulur. PostgreSQL yapılandırıldığında
//! request-time kalıcı okumalar DB'den yapılır; bellek yalnız DB'siz
//! development/test adapter'ı ve WebSocket'in sıcak önbelleğidir.

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
    db: Option<sqlx::PgPool>,
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
                monthly_hours: 30,
                max_languages: 3,
                max_platforms: 3,
                has_watermark: false,
                has_voice_clone: false,
            },
            PlanDefinition {
                name: "pro".into(),
                price_cents: 2499,
                monthly_hours: 80,
                max_languages: 10,
                max_platforms: -1,
                has_watermark: false,
                has_voice_clone: true,
            },
            PlanDefinition {
                name: "streamer".into(),
                price_cents: 9999,
                monthly_hours: 200,
                max_languages: -1,
                max_platforms: -1,
                has_watermark: false,
                has_voice_clone: true,
            },
            // "Founding 5" kapalı beta (Faz 1 ilk test): admin onayladığı
            // kullanıcılara `ws::mod::effective_plan` üzerinden atanır —
            // `PlanRegistry`'de normal bir plan gibi durur (admin panelden
            // `PUT /admin/plans` ile ayarlanabilir), ama `PlanRegistry::get`
            // dışında hiçbir yerde manuel atanmaz. İlk test STT-only
            // olduğundan `has_voice_clone: false`.
            PlanDefinition {
                name: "founding".into(),
                price_cents: 0,
                monthly_hours: 50,
                max_languages: 3,
                max_platforms: 3,
                has_watermark: false,
                has_voice_clone: false,
            },
        ];
        let map = defaults.into_iter().map(|p| (p.name.clone(), p)).collect();
        PlanRegistry {
            plans: RwLock::new(map),
            db: None,
        }
    }

    /// Registry'yi PostgreSQL plan repository'sine bağlar. Varsayılanlar
    /// WebSocket gibi eşzamanlı sıcak yollar için önbellekte tutulur; HTTP
    /// mutation/read yolları `persistent_*` ile DB'nin kendisini kullanır.
    pub fn with_db(mut self, pool: sqlx::PgPool) -> Self {
        self.db = Some(pool);
        self
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

    /// Bir planı siler, önceki tanımı döner (DB yazımı başarısız olduğunda
    /// yeni eklenen planı geri almak için). Bilinmeyen plan adı `None`.
    pub fn remove(&self, name: &str) -> Option<PlanDefinition> {
        self.plans.write().ok()?.remove(name)
    }

    /// `plans` tablosundan okunan tanımları varsayılanların üzerine yazar
    /// (başlangıçta, admin'in önceki oturumlarda kaydettiği planları belleğe
    /// yüklemek için — `ServiceConfig::apply_db_rows` ile aynı desen).
    pub fn apply_db_rows(&self, rows: Vec<PlanDefinition>) {
        if let Ok(mut map) = self.plans.write() {
            for plan in rows {
                map.insert(plan.name.clone(), plan);
            }
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

    /// Production'da DB-authoritative plan kataloğunu getirir. DB yokken
    /// mevcut bellek adapter'ını kullanır.
    pub async fn persistent_snapshot(&self) -> Result<Vec<PlanDefinition>, sqlx::Error> {
        match &self.db {
            Some(pool) => load_from_db(pool).await,
            None => Ok(self.snapshot()),
        }
    }

    /// Production'da tek bir planı DB'den okur. `None`, hem DB hem adapter
    /// için bilinmeyen/inaktif plan anlamına gelir.
    pub async fn persistent_get(&self, name: &str) -> Result<Option<PlanDefinition>, sqlx::Error> {
        Ok(self
            .persistent_snapshot()
            .await?
            .into_iter()
            .find(|plan| plan.name == name))
    }

    /// DB-first mutation. Commit'ten sonra yerel sıcak önbelleği günceller;
    /// DB hatasında hiçbir instance-local değer değiştirilmez.
    pub async fn persistent_set(&self, plan: PlanDefinition) -> Result<(), sqlx::Error> {
        if let Some(pool) = &self.db {
            persist_to_db(pool, &plan).await?;
        }
        self.set(plan);
        Ok(())
    }
}

impl Default for PlanRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// `plans` tablosu INTEGER (i32) kolonlar kullanır; aralık dışı bir admin
/// girdisi sessizce kırpılmak yerine açık hata döner.
fn to_db_int(value: i64, field: &str) -> Result<i32, sqlx::Error> {
    i32::try_from(value).map_err(|_| {
        sqlx::Error::Protocol(format!(
            "plan alanı INTEGER aralığı dışında: {field}={value}"
        ))
    })
}

/// Aktif planları `plans` tablosundan okur (başlangıçta, admin'in önceki
/// oturumlarda kaydettiği tanımları belleğe yüklemek için).
pub async fn load_from_db(pool: &sqlx::PgPool) -> Result<Vec<PlanDefinition>, sqlx::Error> {
    use sqlx::Row;
    let rows = sqlx::query(
        "SELECT name, price_cents, monthly_hours, max_languages, max_platforms, \
                has_watermark, has_voice_clone \
         FROM plans WHERE is_active",
    )
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(PlanDefinition {
                name: row.try_get("name")?,
                price_cents: i64::from(row.try_get::<i32, _>("price_cents")?),
                monthly_hours: i64::from(row.try_get::<i32, _>("monthly_hours")?),
                max_languages: i64::from(row.try_get::<i32, _>("max_languages")?),
                max_platforms: i64::from(row.try_get::<i32, _>("max_platforms")?),
                has_watermark: row.try_get("has_watermark")?,
                has_voice_clone: row.try_get("has_voice_clone")?,
            })
        })
        .collect()
}

/// Bir planı `plans` tablosuna yazar (upsert) — admin panelden
/// `PUT /admin/plans` çağrıldığında bellek içi değerle birlikte kalıcı hale
/// gelir, restart'ta kaybolmaz (`config::persist_to_db` ile aynı desen).
pub async fn persist_to_db(pool: &sqlx::PgPool, plan: &PlanDefinition) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO plans (name, price_cents, monthly_hours, max_languages, max_platforms, \
                            has_watermark, has_voice_clone) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (name) DO UPDATE SET \
            price_cents = EXCLUDED.price_cents, \
            monthly_hours = EXCLUDED.monthly_hours, \
            max_languages = EXCLUDED.max_languages, \
            max_platforms = EXCLUDED.max_platforms, \
            has_watermark = EXCLUDED.has_watermark, \
            has_voice_clone = EXCLUDED.has_voice_clone, \
            updated_at = NOW()",
    )
    .bind(&plan.name)
    .bind(to_db_int(plan.price_cents, "price_cents")?)
    .bind(to_db_int(plan.monthly_hours, "monthly_hours")?)
    .bind(to_db_int(plan.max_languages, "max_languages")?)
    .bind(to_db_int(plan.max_platforms, "max_platforms")?)
    .bind(plan.has_watermark)
    .bind(plan.has_voice_clone)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_five_plans() {
        let reg = PlanRegistry::with_defaults();
        assert_eq!(reg.snapshot().len(), 5);
        let free = reg.get("free").unwrap();
        assert_eq!(free.monthly_hours, 5);
        assert!(free.has_watermark);
        let starter = reg.get("starter").unwrap();
        assert_eq!(starter.price_cents, 999);
        assert_eq!(starter.monthly_hours, 30);
        let pro = reg.get("pro").unwrap();
        assert!(pro.has_voice_clone);
        assert_eq!(pro.max_platforms, -1);
        assert_eq!(pro.monthly_hours, 80);
        let streamer = reg.get("streamer").unwrap();
        assert_eq!(streamer.price_cents, 9999);
        assert_eq!(streamer.monthly_hours, 200);
        let founding = reg.get("founding").unwrap();
        assert_eq!(founding.price_cents, 0);
        assert_eq!(founding.monthly_hours, 50);
        assert!(!founding.has_watermark);
        assert!(!founding.has_voice_clone);
    }

    #[test]
    fn apply_db_rows_overrides_defaults() {
        let reg = PlanRegistry::with_defaults();
        reg.apply_db_rows(vec![PlanDefinition {
            name: "starter".into(),
            price_cents: 1299,
            monthly_hours: 35,
            max_languages: 3,
            max_platforms: 3,
            has_watermark: false,
            has_voice_clone: false,
        }]);
        let starter = reg.get("starter").unwrap();
        assert_eq!(starter.price_cents, 1299);
        assert_eq!(starter.monthly_hours, 35);
        // Diğer planlar varsayılanda kalır.
        assert_eq!(reg.get("free").unwrap().monthly_hours, 5);
        assert_eq!(reg.snapshot().len(), 5);
    }

    #[test]
    fn remove_returns_previous_and_deletes() {
        let reg = PlanRegistry::with_defaults();
        let removed = reg.remove("free").unwrap();
        assert_eq!(removed.monthly_hours, 5);
        assert!(reg.get("free").is_none());
        assert!(reg.remove("ghost").is_none());
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
