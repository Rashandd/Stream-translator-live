//! Backend STT control-plane client (phase 2.6b — docs/local-stt-plan.md §2/§5).
//!
//! The SDK is a headless tray agent; all user interaction happens in the web
//! panel. This module implements the SDK side of that contract:
//! - `GET /api/stt/policy` → policy + user mode + last benchmark (one call),
//! - `GET /api/sdk/commands` → one-shot command poll (panel queued them),
//! - `POST /api/sdk/benchmark` → hardware test report.
//!
//! [`handle_pending_commands`] glues them together: poll once, run the
//! benchmark for each queued command against the injected engine, report.

use serde::{Deserialize, Serialize};

use super::local_stt::{run_benchmark, LocalSttEngine};

/// `GET /api/stt/policy` yanıtı (backend `stt_policy::SttPolicyResponse`).
#[derive(Debug, Clone, Deserialize)]
pub struct SttPolicy {
    pub local_allowed: bool,
    pub usage_multiplier: f64,
    pub requirements: Requirements,
    pub watchdog: Watchdog,
    pub max_chars: u64,
    pub mode: SttMode,
    #[serde(default)]
    pub benchmark: Option<StoredBenchmark>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Requirements {
    pub min_vram_mb: u64,
    pub benchmark_rtf_max: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Watchdog {
    pub p95_ms: u64,
    pub retry_minutes: u64,
}

/// Panelde seçilen işleme modu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SttMode {
    Cloud,
    Local,
}

/// Sunucuda saklanan son benchmark kaydı.
#[derive(Debug, Clone, Deserialize)]
pub struct StoredBenchmark {
    pub passed: bool,
    pub rtf: f64,
    pub p95_ms: u64,
    #[serde(default)]
    pub gpu_name: Option<String>,
    #[serde(default)]
    pub vram_mb: Option<u64>,
    pub reported_at: i64,
}

/// `GET /api/sdk/commands` yanıtındaki tek komut.
#[derive(Debug, Clone, Deserialize)]
pub struct QueuedCommand {
    pub id: String,
    pub command: String,
}

#[derive(Debug, Deserialize)]
struct CommandsResponse {
    commands: Vec<QueuedCommand>,
}

/// `POST /api/sdk/benchmark` gövdesi.
#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkReport {
    pub passed: bool,
    pub rtf: f64,
    pub p95_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vram_mb: Option<u64>,
}

/// Donanım kimliği (platform adaptörü doldurur; çekirdek okumaz).
#[derive(Debug, Clone, Default)]
pub struct HardwareInfo {
    pub gpu_name: Option<String>,
    pub vram_mb: Option<u64>,
}

/// Benchmark örneği: VAD formatında bölünmüş PCM segmentleri.
pub struct BenchmarkSample {
    pub segments: Vec<Vec<i16>>,
    pub sample_rate_hz: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum SttControlError {
    #[error("http request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("server rejected the request: {0}")]
    Server(reqwest::StatusCode),
    #[error("local engine failed: {0}")]
    Engine(#[from] super::local_stt::LocalSttError),
}

fn url(base_url: &str, path: &str) -> String {
    format!("{}{path}", base_url.trim_end_matches('/'))
}

/// `GET /api/stt/policy` — cihaz JWT'siyle politika + mod + benchmark okur.
pub async fn fetch_policy(
    http: &reqwest::Client,
    base_url: &str,
    device_token: &str,
) -> Result<SttPolicy, SttControlError> {
    let response = http
        .get(url(base_url, "/api/stt/policy"))
        .bearer_auth(device_token)
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(SttControlError::Server(response.status()));
    }
    Ok(response.json().await?)
}

/// `GET /api/sdk/commands` — bekleyen komutları çeker (tek seferlik teslim).
pub async fn poll_commands(
    http: &reqwest::Client,
    base_url: &str,
    device_token: &str,
) -> Result<Vec<QueuedCommand>, SttControlError> {
    let response = http
        .get(url(base_url, "/api/sdk/commands"))
        .bearer_auth(device_token)
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(SttControlError::Server(response.status()));
    }
    let body: CommandsResponse = response.json().await?;
    Ok(body.commands)
}

/// `POST /api/sdk/benchmark` — donanım testi sonucunu raporlar.
pub async fn report_benchmark(
    http: &reqwest::Client,
    base_url: &str,
    device_token: &str,
    report: &BenchmarkReport,
) -> Result<(), SttControlError> {
    let response = http
        .post(url(base_url, "/api/sdk/benchmark"))
        .bearer_auth(device_token)
        .json(report)
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(SttControlError::Server(response.status()));
    }
    Ok(())
}

/// Tek yoklama turu: bekleyen komutları çeker, her `benchmark` komutu için
/// motoru policy eşiklerine karşı koşturur ve sonucu raporlar. İşlenen komut
/// sayısını döndürür (tray ajanının periyodik döngüsü bunu çağırır).
pub async fn handle_pending_commands(
    http: &reqwest::Client,
    base_url: &str,
    device_token: &str,
    engine: &mut dyn LocalSttEngine,
    sample: &BenchmarkSample,
    hardware: &HardwareInfo,
) -> Result<usize, SttControlError> {
    let commands = poll_commands(http, base_url, device_token).await?;
    if commands.is_empty() {
        return Ok(0);
    }

    let policy = fetch_policy(http, base_url, device_token).await?;
    let mut handled = 0;

    for command in commands {
        match command.command.as_str() {
            "benchmark" => {
                let outcome = run_benchmark(
                    engine,
                    &sample.segments,
                    sample.sample_rate_hz,
                    policy.requirements.benchmark_rtf_max,
                    policy.watchdog.p95_ms,
                )?;
                report_benchmark(
                    http,
                    base_url,
                    device_token,
                    &BenchmarkReport {
                        passed: outcome.passed,
                        rtf: outcome.rtf,
                        p95_ms: outcome.p95_ms,
                        gpu_name: hardware.gpu_name.clone(),
                        vram_mb: hardware.vram_mb,
                    },
                )
                .await?;
                handled += 1;
            }
            other => {
                // İleri sürüm komutu olabilir — düşürmek yeterli (backend
                // whitelist'i zaten bilinmeyeni panele 422 ile reddediyor).
                tracing::warn!(command = other, "unknown sdk command ignored");
            }
        }
    }
    Ok(handled)
}

/// Runs [`handle_pending_commands`] on a fixed interval until the task is
/// aborted/dropped — this is what the desktop shell spawns once a real
/// [`LocalSttEngine`] exists (whisper.cpp bind, roadmap 2.6b). A poll
/// failure (network hiccup, backend restart) is logged and swallowed: a
/// flaky poll must never take the tray agent down, it just retries next
/// tick. GPU/VRAM are re-probed every tick — `nvidia-smi` is a cheap
/// subprocess call and ticks are infrequent (15–30 s per the plan).
pub async fn run_polling_loop(
    http: reqwest::Client,
    base_url: String,
    device_token: String,
    mut engine: Box<dyn LocalSttEngine>,
    sample: BenchmarkSample,
    poll_interval: std::time::Duration,
) {
    let mut ticker = tokio::time::interval(poll_interval);
    loop {
        ticker.tick().await;
        let hardware = super::gpu_probe::probe();
        match handle_pending_commands(
            &http,
            &base_url,
            &device_token,
            engine.as_mut(),
            &sample,
            &hardware,
        )
        .await
        {
            Ok(0) => {}
            Ok(handled) => tracing::info!(handled, "processed sdk command(s)"),
            Err(err) => tracing::warn!(%err, "stt control poll failed, will retry next tick"),
        }
    }
}
