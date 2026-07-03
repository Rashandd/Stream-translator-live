//! Kullanım takibi (Faz 1.11) — plan başına aylık yayın süresi limiti.
//!
//! Süre, ses frame'lerinin backend'e ulaştığı duvar saati (wall-clock) aralığına
//! göre ölçülür: VAD zaten boş sesi göndermediği için bu, gerçek konuşma
//! süresine yakın bir yaklaşıklıktır (kesin ses süresi decode gerektirir).
//! Dönem `"YYYY-MM"` (UTC) ile anahtarlanır; ay değişince sayaç kendiliğinden
//! sıfırlanır.
//!
//! Kalıcılık (restart'ta sayaç sıfırlanmasın):
//! - `USAGE_FILE` → append-only JSONL snapshot satırları; açılışta geri
//!   yüklenir (aynı kullanıcı+dönem için SON satır kazanır) ve dosya
//!   kompakte edilir. `Logger::with_file` ile aynı desen.
//! - `DATABASE_URL` → her tam dakika sınırında `usage_records` upsert'i
//!   (fire-and-forget tokio task; kullanıcı `users` tablosunda yoksa FK
//!   ihlali warn loglanır — user sistemi 1.4'te kapanır).

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::Utc;
use serde::{Deserialize, Serialize};

/// Bir kullanıcının bir dönemdeki kullanım durumu.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UsageStatus {
    pub used_seconds: i64,
    /// `-1` = sınırsız (plan `monthly_hours = -1`).
    pub remaining_seconds: i64,
    pub limit_reached: bool,
}

/// Kalıcı dosyadaki bir snapshot satırı.
#[derive(Debug, Serialize, Deserialize)]
struct UsageLine {
    user_id: String,
    period: String,
    used_seconds: i64,
}

#[derive(Default)]
struct UserUsage {
    period: String,
    used_seconds: i64,
    /// DB'ye en son yazılan tam dakika (used_seconds / 60).
    flushed_minutes: i64,
}

/// Kullanım depolama hataları.
#[derive(Debug, thiserror::Error)]
pub enum UsageError {
    #[error("usage file error: {0}")]
    Io(#[from] std::io::Error),
}

/// Kullanıcı → dönem kullanım sayaçlarını tutan takipçi (+ opsiyonel kalıcılık).
pub struct UsageTracker {
    users: Mutex<HashMap<String, UserUsage>>,
    file: Option<Mutex<std::fs::File>>,
    file_path: Option<PathBuf>,
    db: Option<sqlx::PgPool>,
}

impl UsageTracker {
    /// Bellek içi takipçi (dev/CI — restart'ta sıfırlanır).
    pub fn new() -> Self {
        UsageTracker {
            users: Mutex::new(HashMap::new()),
            file: None,
            file_path: None,
            db: None,
        }
    }

    /// JSONL dosya kalıcılığıyla kurar: mevcut satırlar geri yüklenir
    /// (kullanıcı+dönem başına son satır kazanır) ve dosya kompakte edilir.
    pub fn with_file(path: impl AsRef<Path>) -> Result<Self, UsageError> {
        let path = path.as_ref().to_path_buf();
        let users = load_usage(&path)?;

        // Kompaksiyon: ay boyunca büyüyen dosya tek snapshot setine iner.
        let compacted: Vec<String> = users
            .iter()
            .map(|(user_id, u)| snapshot_line(user_id, u))
            .collect();
        std::fs::write(&path, compacted.join(""))?;

        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(UsageTracker {
            users: Mutex::new(users),
            file: Some(Mutex::new(file)),
            file_path: Some(path),
            db: None,
        })
    }

    /// DB write-behind'ı açar (tam dakika sınırlarında `usage_records` upsert).
    pub fn with_db(mut self, pool: Option<sqlx::PgPool>) -> Self {
        self.db = pool;
        self
    }

    /// Kalıcı dosyanın yolu (tanılama/log için).
    pub fn file_path(&self) -> Option<&Path> {
        self.file_path.as_deref()
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
        let (status, snapshot, db_flush) = {
            let mut map = self.users.lock().expect("usage tracker lock poisoned");
            let entry = map.entry(user_id.to_string()).or_default();
            if entry.period != period {
                entry.period = period.to_string();
                entry.used_seconds = 0;
                entry.flushed_minutes = 0;
            }
            entry.used_seconds += seconds.round() as i64;

            let status = if monthly_hours < 0 {
                UsageStatus {
                    used_seconds: entry.used_seconds,
                    remaining_seconds: -1,
                    limit_reached: false,
                }
            } else {
                let limit = monthly_hours * 3600;
                UsageStatus {
                    used_seconds: entry.used_seconds,
                    remaining_seconds: (limit - entry.used_seconds).max(0),
                    limit_reached: entry.used_seconds >= limit,
                }
            };

            // Sadece gerçek artışlarda persist et (status() sorguları hariç).
            let snapshot = (seconds > 0.0).then(|| snapshot_line(user_id, entry));
            let minutes = entry.used_seconds / 60;
            let db_flush = (seconds > 0.0 && minutes > entry.flushed_minutes).then(|| {
                entry.flushed_minutes = minutes;
                (user_id.to_string(), period.to_string(), entry.used_seconds)
            });
            (status, snapshot, db_flush)
        };

        if let (Some(line), Some(file)) = (&snapshot, &self.file) {
            if let Ok(mut f) = file.lock() {
                if let Err(err) = f.write_all(line.as_bytes()) {
                    tracing::warn!(error = %err, "usage file append failed");
                }
            }
        }

        if let (Some((user, period, used)), Some(pool)) = (db_flush, &self.db) {
            spawn_db_upsert(pool.clone(), user, period, used);
        }

        status
    }
}

/// Bir kullanıcının snapshot JSONL satırı (newline dahil).
fn snapshot_line(user_id: &str, usage: &UserUsage) -> String {
    let line = UsageLine {
        user_id: user_id.to_string(),
        period: usage.period.clone(),
        used_seconds: usage.used_seconds,
    };
    // UsageLine alanları String/i64 — serialization hata üretemez.
    let mut s = serde_json::to_string(&line).unwrap_or_default();
    s.push('\n');
    s
}

/// Dosyadan kullanım haritasını yükler; bozuk satırlar atlanır (warn).
fn load_usage(path: &Path) -> Result<HashMap<String, UserUsage>, UsageError> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(e) => return Err(UsageError::Io(e)),
    };
    let mut map: HashMap<String, UserUsage> = HashMap::new();
    for line in content.lines().filter(|l| !l.trim().is_empty()) {
        match serde_json::from_str::<UsageLine>(line) {
            Ok(parsed) => {
                map.insert(
                    parsed.user_id,
                    UserUsage {
                        period: parsed.period,
                        used_seconds: parsed.used_seconds,
                        flushed_minutes: parsed.used_seconds / 60,
                    },
                );
            }
            Err(err) => tracing::warn!(error = %err, "skipping malformed usage line"),
        }
    }
    Ok(map)
}

/// `usage_records` upsert'ini arkaplanda koşar; FK ihlali (users kaydı yok)
/// dahil hatalar warn loglanır, pipeline'ı asla düşürmez.
fn spawn_db_upsert(pool: sqlx::PgPool, user_id: String, period: String, used_seconds: i64) {
    tokio::spawn(async move {
        let Ok(uuid) = uuid::Uuid::parse_str(&user_id) else {
            tracing::warn!(%user_id, "usage db flush skipped: user id is not a UUID");
            return;
        };
        let result = sqlx::query(
            "INSERT INTO usage_records (user_id, period, seconds_used) \
             VALUES ($1, $2, $3) \
             ON CONFLICT (user_id, period) DO UPDATE SET \
                 seconds_used = EXCLUDED.seconds_used, updated_at = NOW()",
        )
        .bind(uuid)
        .bind(&period)
        .bind(used_seconds)
        .execute(&pool)
        .await;
        if let Err(err) = result {
            tracing::warn!(error = %err, %user_id, "usage db flush failed (users row missing? — 1.4)");
        }
    });
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

    // --- kalıcılık ---

    fn temp_usage_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("sd-usage-{tag}-{}.jsonl", std::process::id()))
    }

    #[test]
    fn usage_survives_restart_via_file() {
        let path = temp_usage_path("restart");
        let _ = std::fs::remove_file(&path);

        {
            let tracker = UsageTracker::with_file(&path).expect("open");
            tracker.record_for_period("u1", 120.0, 5, "2026-07");
            tracker.record_for_period("u1", 60.0, 5, "2026-07");
            tracker.record_for_period("u2", 30.0, 5, "2026-07");
        }

        // "Restart": aynı dosyadan yeni tracker.
        let reopened = UsageTracker::with_file(&path).expect("reopen");
        let u1 = reopened.record_for_period("u1", 0.0, 5, "2026-07");
        assert_eq!(u1.used_seconds, 180);
        let u2 = reopened.record_for_period("u2", 0.0, 5, "2026-07");
        assert_eq!(u2.used_seconds, 30);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn reload_compacts_file_to_one_line_per_user() {
        let path = temp_usage_path("compact");
        let _ = std::fs::remove_file(&path);

        {
            let tracker = UsageTracker::with_file(&path).expect("open");
            for _ in 0..10 {
                tracker.record_for_period("u1", 10.0, 5, "2026-07");
            }
        }
        // 10 append satırı → reload sonrası 1 kompakt satır.
        let _ = UsageTracker::with_file(&path).expect("reopen");
        let content = std::fs::read_to_string(&path).expect("read");
        assert_eq!(content.lines().count(), 1, "content: {content}");
        assert!(content.contains("\"used_seconds\":100"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn malformed_lines_are_skipped_on_load() {
        let path = temp_usage_path("malformed");
        std::fs::write(
            &path,
            "{\"user_id\":\"u1\",\"period\":\"2026-07\",\"used_seconds\":42}\nnot-json\n",
        )
        .expect("seed");

        let tracker = UsageTracker::with_file(&path).expect("open despite junk");
        let status = tracker.record_for_period("u1", 0.0, 5, "2026-07");
        assert_eq!(status.used_seconds, 42);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn period_change_resets_even_after_reload() {
        let path = temp_usage_path("period");
        let _ = std::fs::remove_file(&path);
        {
            let tracker = UsageTracker::with_file(&path).expect("open");
            tracker.record_for_period("u1", 500.0, 5, "2026-06");
        }
        let reopened = UsageTracker::with_file(&path).expect("reopen");
        // Yeni ay → eski dönem sayacı taşınmaz.
        let status = reopened.record_for_period("u1", 10.0, 5, "2026-07");
        assert_eq!(status.used_seconds, 10);

        let _ = std::fs::remove_file(&path);
    }
}
