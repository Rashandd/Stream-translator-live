//! SDK komut kanalı + STT ayar senkronu (Faz 2.6b — docs/local-stt-plan.md §2/§5).
//!
//! SDK tamamen arka planda çalışan bir tray ajanıdır; tüm kumanda web
//! paneldedir. Panel komut bırakır (`POST /api/sdk/commands`), SDK yoklar
//! (`GET /api/sdk/commands` — tek seferlik teslim) ve benchmark sonucunu
//! raporlar (`POST /api/sdk/benchmark`). STT mod tercihi (`PUT /api/stt/mode`)
//! panelden yazılır, SDK policy yanıtından okur.
//!
//! Komut kuyruğu bellek içidir (komutlar nadir ve anlıktır — restart bekleyen
//! komutu düşürür, panel yeniden tetikler). Mod tercihi ve son benchmark
//! sonucu `STT_SETTINGS_FILE` verilirse düz JSON dosyada kalıcıdır (secret
//! içermez; stream key/token gibi şifreleme gerektiren veri yoktur).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::auth::authz::AuthUser;
use crate::config::ServiceConfig;
use crate::logging::{LogLevel, Logger};

/// Panelin SDK'ya bırakabileceği komutlar (whitelist).
const KNOWN_COMMANDS: &[&str] = &["benchmark"];

/// Kuyruğa alınmış tek bir komut.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueuedCommand {
    pub id: String,
    pub command: String,
}

/// Kullanıcı başına bellek içi komut kuyruğu (tek seferlik teslim).
#[derive(Default)]
pub struct SdkCommandQueue {
    inner: Mutex<HashMap<String, Vec<QueuedCommand>>>,
}

impl SdkCommandQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Komutu kuyruğa ekler ve kuyruklanan kaydı döndürür.
    pub fn push(&self, user_id: &str, command: &str) -> QueuedCommand {
        let cmd = QueuedCommand {
            id: uuid::Uuid::new_v4().simple().to_string(),
            command: command.to_string(),
        };
        self.inner
            .lock()
            .expect("sdk command queue lock poisoned")
            .entry(user_id.to_string())
            .or_default()
            .push(cmd.clone());
        cmd
    }

    /// Kullanıcının bekleyen komutlarını döndürür ve kuyruğu boşaltır
    /// (tek seferlik teslim — SDK aynı komutu iki kez almaz).
    pub fn drain(&self, user_id: &str) -> Vec<QueuedCommand> {
        self.inner
            .lock()
            .expect("sdk command queue lock poisoned")
            .remove(user_id)
            .unwrap_or_default()
    }
}

/// STT işleme modu — panel yazar, SDK okur. Varsayılan bulut.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SttMode {
    #[default]
    Cloud,
    Local,
}

/// SDK'nın raporladığı donanım testi sonucu.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BenchmarkReport {
    pub passed: bool,
    /// Real-time factor: işleme süresi / ses süresi (küçük iyi).
    pub rtf: f64,
    /// Segment başına p95 gecikme (ms).
    pub p95_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vram_mb: Option<u64>,
    /// Rapor zamanı (unix saniye) — sunucu tarafında damgalanır.
    #[serde(default)]
    pub reported_at: i64,
}

/// Kullanıcı başına STT ayarları.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserSttSettings {
    #[serde(default)]
    pub mode: SttMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark: Option<BenchmarkReport>,
}

/// STT ayar depolama hataları.
#[derive(Debug, thiserror::Error)]
pub enum SttSettingsError {
    #[error("settings file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("settings file parse error: {0}")]
    Parse(#[from] serde_json::Error),
}

/// Kullanıcı → STT ayarları deposu (opsiyonel düz-JSON kalıcılık).
pub struct SttSettingsStore {
    inner: Mutex<HashMap<String, UserSttSettings>>,
    path: Option<PathBuf>,
}

impl SttSettingsStore {
    /// Bellek içi depo (dev/CI — restart'ta varsayılana döner).
    pub fn in_memory() -> Self {
        SttSettingsStore {
            inner: Mutex::new(HashMap::new()),
            path: None,
        }
    }

    /// Dosya kalıcılığıyla kurar; mevcut içerik geri yüklenir.
    pub fn with_file(path: impl AsRef<Path>) -> Result<Self, SttSettingsError> {
        let path = path.as_ref().to_path_buf();
        let inner = match std::fs::read_to_string(&path) {
            Ok(content) if !content.trim().is_empty() => serde_json::from_str(&content)?,
            Ok(_) => HashMap::new(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
            Err(e) => return Err(SttSettingsError::Io(e)),
        };
        Ok(SttSettingsStore {
            inner: Mutex::new(inner),
            path: Some(path),
        })
    }

    /// Kullanıcının ayarları (kayıt yoksa varsayılan: cloud, benchmark yok).
    pub fn get(&self, user_id: &str) -> UserSttSettings {
        self.inner
            .lock()
            .expect("stt settings lock poisoned")
            .get(user_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Mod tercihini yazar ve kalıcılaştırır.
    pub fn set_mode(&self, user_id: &str, mode: SttMode) {
        self.update(user_id, |s| s.mode = mode);
    }

    /// Benchmark sonucunu yazar ve kalıcılaştırır.
    pub fn set_benchmark(&self, user_id: &str, report: BenchmarkReport) {
        self.update(user_id, |s| s.benchmark = Some(report));
    }

    fn update(&self, user_id: &str, apply: impl FnOnce(&mut UserSttSettings)) {
        let snapshot = {
            let mut map = self.inner.lock().expect("stt settings lock poisoned");
            apply(map.entry(user_id.to_string()).or_default());
            self.path.as_ref().map(|_| {
                // Persist için tam haritanın anlık kopyası (küçük veri).
                serde_json::to_string_pretty(&*map).unwrap_or_default()
            })
        };
        if let (Some(json), Some(path)) = (snapshot, &self.path) {
            if let Err(err) = std::fs::write(path, json) {
                tracing::warn!(error = %err, "stt settings persist failed");
            }
        }
    }
}

// ── HTTP handler'ları ────────────────────────────────────────────────────

/// `POST /api/sdk/commands` gövdesi.
#[derive(Debug, Deserialize)]
pub struct PushCommandRequest {
    pub command: String,
}

/// `POST /api/sdk/commands` (panel) — SDK'ya komut bırakır.
pub async fn push_command(
    user: AuthUser,
    State(queue): State<Arc<SdkCommandQueue>>,
    State(logger): State<Arc<Logger>>,
    Json(req): Json<PushCommandRequest>,
) -> Result<(StatusCode, Json<QueuedCommand>), (StatusCode, String)> {
    if !KNOWN_COMMANDS.contains(&req.command.as_str()) {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("unknown command: {}", req.command),
        ));
    }
    let cmd = queue.push(&user.claims.sub, &req.command);
    logger.log(
        LogLevel::Info,
        "sdk_link",
        &format!("sdk command queued: {}", cmd.command),
        Some(&user.claims.sub),
        None,
    );
    Ok((StatusCode::ACCEPTED, Json(cmd)))
}

/// `GET /api/sdk/commands` yanıtı.
#[derive(Debug, Serialize)]
pub struct CommandsResponse {
    pub commands: Vec<QueuedCommand>,
}

/// `GET /api/sdk/commands` (SDK yoklaması) — bekleyenleri döndürür ve boşaltır.
pub async fn poll_commands(
    user: AuthUser,
    State(queue): State<Arc<SdkCommandQueue>>,
) -> Json<CommandsResponse> {
    Json(CommandsResponse {
        commands: queue.drain(&user.claims.sub),
    })
}

/// `POST /api/sdk/benchmark` gövdesi (rapor zamanı sunucuda damgalanır).
#[derive(Debug, Deserialize)]
pub struct BenchmarkRequest {
    pub passed: bool,
    pub rtf: f64,
    pub p95_ms: u64,
    #[serde(default)]
    pub gpu_name: Option<String>,
    #[serde(default)]
    pub vram_mb: Option<u64>,
}

/// `POST /api/sdk/benchmark` (SDK) — donanım testi sonucunu kaydeder.
pub async fn report_benchmark(
    user: AuthUser,
    State(settings): State<Arc<SttSettingsStore>>,
    State(logger): State<Arc<Logger>>,
    Json(req): Json<BenchmarkRequest>,
) -> Json<BenchmarkReport> {
    let report = BenchmarkReport {
        passed: req.passed,
        rtf: req.rtf,
        p95_ms: req.p95_ms,
        gpu_name: req.gpu_name,
        vram_mb: req.vram_mb,
        reported_at: chrono::Utc::now().timestamp(),
    };
    settings.set_benchmark(&user.claims.sub, report.clone());
    logger.log(
        LogLevel::Info,
        "sdk_link",
        &format!(
            "benchmark reported: passed={} rtf={:.2} p95={}ms",
            report.passed, report.rtf, report.p95_ms
        ),
        Some(&user.claims.sub),
        None,
    );
    Json(report)
}

/// `PUT /api/stt/mode` gövdesi.
#[derive(Debug, Deserialize)]
pub struct SetModeRequest {
    pub mode: SttMode,
}

/// `PUT /api/stt/mode` yanıtı.
#[derive(Debug, Serialize)]
pub struct SetModeResponse {
    pub mode: SttMode,
}

/// `PUT /api/stt/mode` (panel) — STT mod tercihini yazar. `local` için plan
/// izni (`stt.local.plans`) ve geçilmiş bir donanım testi şarttır; asıl yayın
/// anı gating'i ayrıca `stream_start`'ta enforce edilir (4007).
pub async fn set_mode(
    user: AuthUser,
    State(settings): State<Arc<SttSettingsStore>>,
    State(config): State<Arc<ServiceConfig>>,
    State(logger): State<Arc<Logger>>,
    Json(req): Json<SetModeRequest>,
) -> Result<Json<SetModeResponse>, (StatusCode, String)> {
    if req.mode == SttMode::Local {
        let plans_csv = config.get("stt.local.plans").unwrap_or_default();
        if !crate::integrations::oauth::plan_allowed(&plans_csv, &user.claims.plan) {
            return Err((
                StatusCode::FORBIDDEN,
                "local_stt_not_allowed_for_plan".to_string(),
            ));
        }
        let passed = settings
            .get(&user.claims.sub)
            .benchmark
            .map(|b| b.passed)
            .unwrap_or(false);
        if !passed {
            return Err((StatusCode::CONFLICT, "benchmark_required".to_string()));
        }
    }
    settings.set_mode(&user.claims.sub, req.mode);
    logger.log(
        LogLevel::Info,
        "sdk_link",
        &format!("stt mode set: {:?}", req.mode),
        Some(&user.claims.sub),
        None,
    );
    Ok(Json(SetModeResponse { mode: req.mode }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_delivers_once_per_user() {
        let q = SdkCommandQueue::new();
        q.push("u1", "benchmark");
        q.push("u1", "benchmark");
        q.push("u2", "benchmark");

        let u1 = q.drain("u1");
        assert_eq!(u1.len(), 2);
        assert!(u1.iter().all(|c| c.command == "benchmark"));
        // İkinci yoklama boş — tek seferlik teslim.
        assert!(q.drain("u1").is_empty());
        // Kullanıcı izolasyonu.
        assert_eq!(q.drain("u2").len(), 1);
    }

    #[test]
    fn settings_default_to_cloud_without_benchmark() {
        let s = SttSettingsStore::in_memory();
        let got = s.get("u1");
        assert_eq!(got.mode, SttMode::Cloud);
        assert!(got.benchmark.is_none());
    }

    #[test]
    fn settings_persist_and_reload() {
        let path = std::env::temp_dir().join(format!("sd-stt-settings-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);

        {
            let s = SttSettingsStore::with_file(&path).expect("create");
            s.set_benchmark(
                "u1",
                BenchmarkReport {
                    passed: true,
                    rtf: 0.31,
                    p95_ms: 640,
                    gpu_name: Some("RTX 3060".into()),
                    vram_mb: Some(12288),
                    reported_at: 1_720_000_000,
                },
            );
            s.set_mode("u1", SttMode::Local);
        }

        // Yeni process simülasyonu: aynı dosyadan geri yükleme.
        let s = SttSettingsStore::with_file(&path).expect("reload");
        let got = s.get("u1");
        assert_eq!(got.mode, SttMode::Local);
        let b = got.benchmark.expect("benchmark");
        assert!(b.passed);
        assert_eq!(b.gpu_name.as_deref(), Some("RTX 3060"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn corrupt_settings_file_is_an_error_not_a_panic() {
        let path = std::env::temp_dir().join(format!("sd-stt-corrupt-{}.json", std::process::id()));
        std::fs::write(&path, "{bozuk json").expect("write");
        assert!(SttSettingsStore::with_file(&path).is_err());
        let _ = std::fs::remove_file(&path);
    }
}
