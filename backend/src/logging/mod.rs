//! Merkezi loglama (Faz 1.12) — tüm backend servisleri tek formatta yazar.
//!
//! - Seviye `log.level` config anahtarından okunur (admin panelden değişir,
//!   restart gerektirmez — bkz. `ServiceConfig`).
//! - Kayıtlar `LOG_FILE` verilmişse JSONL dosyasına kalıcı yazılır (restart'ta
//!   kaybolmaz, açılışta geri yüklenir); verilmemişse bellek içi çalışır (dev/CI).
//! - Hassas değerler (`api_key=...`, `token: ...`, `Bearer ...`) yazılmadan
//!   önce maskelenir — log deposuna secret sızmaz, sadece referans ID loglanır.
//! - Admin `GET /admin/logs` ile arar/filtreler (servis, seviye, tarih aralığı,
//!   kullanıcı/session ID) — sadece admin yetkili erişir (bkz. `app::router`).

use std::collections::VecDeque;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::config::ServiceConfig;

/// Bellekte tutulan (sorgulanabilir) en fazla kayıt sayısı. Dosya bundan
/// bağımsız büyür; sorgu penceresi son `CAPACITY` kayıttır.
const CAPACITY: usize = 10_000;

/// Sorgu başına varsayılan/azami kayıt sayısı.
const DEFAULT_QUERY_LIMIT: usize = 100;
const MAX_QUERY_LIMIT: usize = 1_000;

/// Log seviyesi — düşükten yükseğe sıralı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        };
        f.write_str(s)
    }
}

impl FromStr for LogLevel {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "debug" => Ok(LogLevel::Debug),
            "info" => Ok(LogLevel::Info),
            "warn" | "warning" => Ok(LogLevel::Warn),
            "error" => Ok(LogLevel::Error),
            _ => Err(()),
        }
    }
}

/// Tek bir log kaydı — tüm servisler bu formatta yazar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Unix epoch milisaniye (UTC) — `ws::protocol` ile aynı zaman temeli.
    pub ts_ms: i64,
    pub level: LogLevel,
    /// Kaydı yazan servis: `ws`, `pipeline`, `gateway`, `billing`,
    /// `integrations`, `overlay`, ...
    pub service: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// `GET /admin/logs` filtreleri. Tüm alanlar opsiyonel; verilenler AND'lenir.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct LogQuery {
    pub service: Option<String>,
    pub level: Option<LogLevel>,
    /// Aralık başlangıcı (epoch ms, dahil).
    pub from_ms: Option<i64>,
    /// Aralık sonu (epoch ms, dahil).
    pub to_ms: Option<i64>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub limit: Option<usize>,
}

/// Loglama hataları (dosya deposu açılışı).
#[derive(Debug, thiserror::Error)]
pub enum LogError {
    #[error("log file error: {0}")]
    Io(#[from] std::io::Error),
}

struct Inner {
    entries: VecDeque<LogEntry>,
    file: Option<File>,
}

/// Merkezi logger — `AppState` üzerinden tüm servislere enjekte edilir.
pub struct Logger {
    config: Arc<ServiceConfig>,
    inner: Mutex<Inner>,
}

impl Logger {
    /// Bellek içi logger (dev/CI/test) — restart'ta kaybolur, `LOG_FILE` ile
    /// kalıcı depo önerilir (bkz. `with_file`).
    pub fn in_memory(config: Arc<ServiceConfig>) -> Self {
        Logger {
            config,
            inner: Mutex::new(Inner {
                entries: VecDeque::new(),
                file: None,
            }),
        }
    }

    /// Kalıcı JSONL dosya deposu — mevcut kayıtlar açılışta geri yüklenir
    /// (son `CAPACITY` kayıt sorgu penceresine alınır), yenileri sona eklenir.
    pub fn with_file(config: Arc<ServiceConfig>, path: impl AsRef<Path>) -> Result<Self, LogError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let mut entries = VecDeque::new();
        if path.exists() {
            let reader = BufReader::new(File::open(path)?);
            for line in reader.lines() {
                let line = line?;
                if let Ok(entry) = serde_json::from_str::<LogEntry>(&line) {
                    entries.push_back(entry);
                    if entries.len() > CAPACITY {
                        entries.pop_front();
                    }
                }
            }
        }

        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Logger {
            config,
            inner: Mutex::new(Inner {
                entries,
                file: Some(file),
            }),
        })
    }

    /// Config'teki asgari seviye (`log.level`) — bilinmeyen/eksik değerde `info`.
    fn min_level(&self) -> LogLevel {
        self.config
            .get("log.level")
            .and_then(|v| v.parse().ok())
            .unwrap_or(LogLevel::Info)
    }

    /// Bir kayıt yazar. Seviye `log.level`'ın altındaysa düşürülür; mesajdaki
    /// hassas değerler maskelenir. Dosya deposu varsa satır olarak eklenir.
    pub fn log(
        &self,
        level: LogLevel,
        service: &str,
        message: &str,
        user_id: Option<&str>,
        session_id: Option<&str>,
    ) {
        if level < self.min_level() {
            return;
        }

        let entry = LogEntry {
            ts_ms: chrono::Utc::now().timestamp_millis(),
            level,
            service: service.to_string(),
            message: redact(message),
            user_id: user_id.map(str::to_string),
            session_id: session_id.map(str::to_string),
        };

        let Ok(mut inner) = self.inner.lock() else {
            return;
        };
        if let Some(file) = inner.file.as_mut() {
            match serde_json::to_string(&entry) {
                Ok(json) => {
                    if let Err(err) = writeln!(file, "{json}") {
                        tracing::error!(error = %err, "failed to persist log entry");
                    }
                }
                Err(err) => tracing::error!(error = %err, "failed to serialize log entry"),
            }
        }
        inner.entries.push_back(entry);
        if inner.entries.len() > CAPACITY {
            inner.entries.pop_front();
        }
    }

    /// Filtrelere uyan kayıtları yeniden-eskiye sıralı döner.
    pub fn query(&self, q: &LogQuery) -> Vec<LogEntry> {
        let limit = q
            .limit
            .unwrap_or(DEFAULT_QUERY_LIMIT)
            .min(MAX_QUERY_LIMIT);

        let Ok(inner) = self.inner.lock() else {
            return Vec::new();
        };
        inner
            .entries
            .iter()
            .rev()
            .filter(|e| q.service.as_deref().is_none_or(|s| e.service == s))
            .filter(|e| q.level.is_none_or(|l| e.level == l))
            .filter(|e| q.from_ms.is_none_or(|from| e.ts_ms >= from))
            .filter(|e| q.to_ms.is_none_or(|to| e.ts_ms <= to))
            .filter(|e| q.user_id.as_deref().is_none_or(|u| e.user_id.as_deref() == Some(u)))
            .filter(|e| {
                q.session_id
                    .as_deref()
                    .is_none_or(|s| e.session_id.as_deref() == Some(s))
            })
            .take(limit)
            .cloned()
            .collect()
    }

    /// Son `n` hata kaydı (Admin Dashboard "Log ve Hata İzleme").
    pub fn recent_errors(&self, n: usize) -> Vec<LogEntry> {
        self.query(&LogQuery {
            level: Some(LogLevel::Error),
            limit: Some(n),
            ..LogQuery::default()
        })
    }
}

/// Maskelenen anahtar parçaları — `key` hem `api_key` hem `stream_key`'i kapsar.
const SENSITIVE_KEYS: &[&str] = &["key", "token", "secret", "password", "authorization"];

/// Mesajdaki hassas değerleri maskeler.
///
/// İki desen tanınır:
/// - `anahtar=değer` / `anahtar: değer` — anahtar hassas kelime içeriyorsa
///   değer `[REDACTED]` olur,
/// - `Bearer <değer>` — takip eden kelime maskelenir.
///
/// Boşluklar tek boşluğa normalize edilir (kelime bazlı tarama).
fn redact(message: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut mask_next = false;

    for word in message.split_whitespace() {
        if mask_next {
            // `Authorization: Bearer <token>` — değer "Bearer"dan sonraki kelime.
            if word.eq_ignore_ascii_case("bearer") {
                out.push(word.to_string());
                continue; // mask_next açık kalır
            }
            out.push("[REDACTED]".to_string());
            mask_next = false;
            continue;
        }

        if word.eq_ignore_ascii_case("bearer") {
            out.push(word.to_string());
            mask_next = true;
            continue;
        }

        if let Some(pos) = word.find(['=', ':']) {
            let (key, rest) = word.split_at(pos);
            let key_lower = key.to_ascii_lowercase();
            let is_sensitive = SENSITIVE_KEYS.iter().any(|s| key_lower.contains(s));
            if is_sensitive {
                if rest.len() > 1 {
                    // `key=value` — değer bitişik.
                    let sep = &rest[..1];
                    out.push(format!("{key}{sep}[REDACTED]"));
                } else {
                    // `key:` — değer bir sonraki kelime.
                    out.push(word.to_string());
                    mask_next = true;
                }
                continue;
            }
        }

        out.push(word.to_string());
    }

    out.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn logger() -> Logger {
        Logger::in_memory(Arc::new(ServiceConfig::with_defaults()))
    }

    #[test]
    fn level_parses_and_orders() {
        assert_eq!("error".parse::<LogLevel>(), Ok(LogLevel::Error));
        assert_eq!("WARN".parse::<LogLevel>(), Ok(LogLevel::Warn));
        assert!("gibberish".parse::<LogLevel>().is_err());
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Warn < LogLevel::Error);
    }

    #[test]
    fn default_level_drops_debug_keeps_info() {
        let log = logger();
        log.log(LogLevel::Debug, "ws", "debug detayı", None, None);
        log.log(LogLevel::Info, "ws", "bağlantı açıldı", None, None);
        let entries = log.query(&LogQuery::default());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].level, LogLevel::Info);
    }

    #[test]
    fn level_change_applies_without_restart() {
        let config = Arc::new(ServiceConfig::with_defaults());
        let log = Logger::in_memory(Arc::clone(&config));

        log.log(LogLevel::Debug, "ws", "önce", None, None);
        assert!(log.query(&LogQuery::default()).is_empty());

        config.set("log.level", "debug");
        log.log(LogLevel::Debug, "ws", "sonra", None, None);
        assert_eq!(log.query(&LogQuery::default()).len(), 1);
    }

    #[test]
    fn query_filters_by_service_level_user_session() {
        let log = logger();
        log.log(LogLevel::Info, "ws", "a", Some("u1"), Some("s1"));
        log.log(LogLevel::Error, "billing", "b", Some("u2"), None);
        log.log(LogLevel::Warn, "ws", "c", Some("u1"), Some("s2"));

        let by_service = log.query(&LogQuery {
            service: Some("billing".into()),
            ..LogQuery::default()
        });
        assert_eq!(by_service.len(), 1);
        assert_eq!(by_service[0].message, "b");

        let by_level = log.query(&LogQuery {
            level: Some(LogLevel::Warn),
            ..LogQuery::default()
        });
        assert_eq!(by_level.len(), 1);

        let by_user = log.query(&LogQuery {
            user_id: Some("u1".into()),
            ..LogQuery::default()
        });
        assert_eq!(by_user.len(), 2);

        let by_session = log.query(&LogQuery {
            session_id: Some("s2".into()),
            ..LogQuery::default()
        });
        assert_eq!(by_session.len(), 1);
        assert_eq!(by_session[0].message, "c");
    }

    #[test]
    fn query_is_newest_first_and_limited() {
        let log = logger();
        for i in 0..5 {
            log.log(LogLevel::Info, "ws", &format!("m{i}"), None, None);
        }
        let entries = log.query(&LogQuery {
            limit: Some(2),
            ..LogQuery::default()
        });
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].message, "m4");
        assert_eq!(entries[1].message, "m3");
    }

    #[test]
    fn query_filters_by_time_range() {
        let log = logger();
        log.log(LogLevel::Info, "ws", "içeride", None, None);
        let ts = log.query(&LogQuery::default())[0].ts_ms;

        let inside = log.query(&LogQuery {
            from_ms: Some(ts - 1),
            to_ms: Some(ts + 1),
            ..LogQuery::default()
        });
        assert_eq!(inside.len(), 1);

        let outside = log.query(&LogQuery {
            to_ms: Some(ts - 1000),
            ..LogQuery::default()
        });
        assert!(outside.is_empty());
    }

    #[test]
    fn recent_errors_returns_only_errors() {
        let log = logger();
        log.log(LogLevel::Info, "ws", "olağan", None, None);
        log.log(LogLevel::Error, "gateway", "stt patladı", None, None);
        log.log(LogLevel::Error, "billing", "webhook imzası geçersiz", None, None);

        let errors = log.recent_errors(10);
        assert_eq!(errors.len(), 2);
        assert!(errors.iter().all(|e| e.level == LogLevel::Error));
        assert_eq!(errors[0].message, "webhook imzası geçersiz");
    }

    #[test]
    fn redacts_sensitive_key_value_pairs() {
        assert_eq!(
            redact("deepgram api_key=dg_12345 ile bağlanıldı"),
            "deepgram api_key=[REDACTED] ile bağlanıldı"
        );
        assert_eq!(
            redact("stream_key: live_abc123"),
            "stream_key: [REDACTED]"
        );
        assert_eq!(
            redact("Authorization: Bearer eyJhbGciOi"),
            "Authorization: Bearer [REDACTED]"
        );
        assert_eq!(redact("Bearer eyJhbGciOi"), "Bearer [REDACTED]");
        // Hassas olmayan çiftlere dokunulmaz.
        assert_eq!(redact("plan=free user=u1"), "plan=free user=u1");
    }

    #[test]
    fn log_messages_are_redacted_before_storage() {
        let log = logger();
        log.log(
            LogLevel::Warn,
            "gateway",
            "provider hatası token=sk_live_999",
            None,
            None,
        );
        let entries = log.query(&LogQuery::default());
        assert_eq!(entries[0].message, "provider hatası token=[REDACTED]");
        assert!(!entries[0].message.contains("sk_live_999"));
    }

    #[test]
    fn file_store_survives_reopen() {
        let path = std::env::temp_dir().join(format!(
            "streamdubber-log-test-{}.jsonl",
            uuid::Uuid::new_v4().simple()
        ));
        let config = Arc::new(ServiceConfig::with_defaults());

        {
            let log = Logger::with_file(Arc::clone(&config), &path).expect("open log file");
            log.log(LogLevel::Error, "billing", "kalıcı kayıt", Some("u1"), None);
        } // drop → dosya kapanır (restart simülasyonu)

        let reopened = Logger::with_file(config, &path).expect("reopen log file");
        let entries = reopened.query(&LogQuery::default());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message, "kalıcı kayıt");
        assert_eq!(entries[0].user_id.as_deref(), Some("u1"));

        let _ = std::fs::remove_file(&path);
    }
}
