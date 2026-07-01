//! Kullanım takibi (Faz 1.11) — plan başına aylık yayın süresi limiti.
//!
//! Süre, ses frame'lerinin backend'e ulaştığı duvar saati (wall-clock) aralığına
//! göre ölçülür: VAD zaten boş sesi göndermediği için bu, gerçek konuşma
//! süresine yakın bir yaklaşıklıktır (kesin ses süresi decode gerektirir).
//! Dönem `"YYYY-MM"` (UTC) ile anahtarlanır; ay değişince sayaç kendiliğinden
//! sıfırlanır. Faz 1 iskeleti bellek içi; ileride `usage_records` tablosundan
//! okunur/yazılır (restart sonrası kalıcılık).

use std::collections::HashMap;
use std::sync::Mutex;

use chrono::Utc;

/// Bir kullanıcının bir dönemdeki kullanım durumu.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UsageStatus {
    pub used_seconds: i64,
    /// `-1` = sınırsız (plan `monthly_hours = -1`).
    pub remaining_seconds: i64,
    pub limit_reached: bool,
}

#[derive(Default)]
struct UserUsage {
    period: String,
    used_seconds: i64,
}

/// Kullanıcı → dönem kullanım sayaçlarını tutan in-memory takipçi.
pub struct UsageTracker {
    users: Mutex<HashMap<String, UserUsage>>,
}

impl UsageTracker {
    /// Boş bir takipçi kurar.
    pub fn new() -> Self {
        UsageTracker {
            users: Mutex::new(HashMap::new()),
        }
    }

    fn current_period() -> String {
        Utc::now().format("%Y-%m").to_string()
    }

    /// `seconds` kadar kullanım ekler (dönem değiştiyse sıfırdan başlar) ve
    /// `monthly_hours` limitine göre durumu döner. `monthly_hours < 0` sınırsız.
    pub fn record(&self, user_id: &str, seconds: f64, monthly_hours: i64) -> UsageStatus {
        self.record_for_period(user_id, seconds, monthly_hours, &Self::current_period())
    }

    /// Geçerli kullanım (kayıt eklemeden) — `session_ready` anında gösterim için.
    pub fn status(&self, user_id: &str, monthly_hours: i64) -> UsageStatus {
        self.record(user_id, 0.0, monthly_hours)
    }

    /// Test edilebilir çekirdek: `period` enjekte edilir.
    fn record_for_period(&self, user_id: &str, seconds: f64, monthly_hours: i64, period: &str) -> UsageStatus {
        let mut map = self.users.lock().expect("usage tracker lock poisoned");
        let entry = map.entry(user_id.to_string()).or_default();
        if entry.period != period {
            entry.period = period.to_string();
            entry.used_seconds = 0;
        }
        entry.used_seconds += seconds.round() as i64;

        if monthly_hours < 0 {
            return UsageStatus {
                used_seconds: entry.used_seconds,
                remaining_seconds: -1,
                limit_reached: false,
            };
        }
        let limit = monthly_hours * 3600;
        UsageStatus {
            used_seconds: entry.used_seconds,
            remaining_seconds: (limit - entry.used_seconds).max(0),
            limit_reached: entry.used_seconds >= limit,
        }
    }
}

impl Default for UsageTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_user_has_full_allowance() {
        let tracker = UsageTracker::new();
        let status = tracker.status("u1", 5);
        assert_eq!(status.used_seconds, 0);
        assert_eq!(status.remaining_seconds, 5 * 3600);
        assert!(!status.limit_reached);
    }

    #[test]
    fn recording_accumulates_within_period() {
        let tracker = UsageTracker::new();
        tracker.record_for_period("u1", 100.0, 5, "2026-07");
        let status = tracker.record_for_period("u1", 50.0, 5, "2026-07");
        assert_eq!(status.used_seconds, 150);
        assert_eq!(status.remaining_seconds, 5 * 3600 - 150);
    }

    #[test]
    fn limit_reached_when_used_meets_cap() {
        let tracker = UsageTracker::new();
        let status = tracker.record_for_period("u1", 3600.0, 1, "2026-07");
        assert_eq!(status.remaining_seconds, 0);
        assert!(status.limit_reached);
    }

    #[test]
    fn unlimited_plan_never_reaches_limit() {
        let tracker = UsageTracker::new();
        let status = tracker.record_for_period("u1", 1_000_000.0, -1, "2026-07");
        assert_eq!(status.remaining_seconds, -1);
        assert!(!status.limit_reached);
    }

    #[test]
    fn new_period_resets_counter() {
        let tracker = UsageTracker::new();
        tracker.record_for_period("u1", 3600.0, 5, "2026-06");
        let status = tracker.record_for_period("u1", 10.0, 5, "2026-07");
        assert_eq!(status.used_seconds, 10);
    }

    #[test]
    fn users_are_isolated() {
        let tracker = UsageTracker::new();
        tracker.record_for_period("u1", 100.0, 5, "2026-07");
        let status = tracker.record_for_period("u2", 5.0, 5, "2026-07");
        assert_eq!(status.used_seconds, 5);
    }
}
