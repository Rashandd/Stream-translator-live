//! Local STT engine abstraction (phase 2.6b — docs/local-stt-plan.md §2/§4).
//!
//! The real model binding (whisper.cpp GGML large-v3-turbo, decision per plan
//! §4) plugs in behind [`LocalSttEngine`]; the core stays model-agnostic and
//! fully testable with [`MockEngine`]. The benchmark runner measures the
//! engine against server-policy thresholds and produces the report the SDK
//! posts to `POST /api/sdk/benchmark`.

use std::time::Instant;

/// Local speech-to-text engine over VAD-finished PCM segments
/// (16 kHz mono s16le — same format the cloud path streams).
pub trait LocalSttEngine: Send {
    /// Transcribes one finished segment. Blocking (runs on a worker thread).
    fn transcribe(&mut self, pcm: &[i16]) -> Result<String, LocalSttError>;
}

/// Local engine errors.
#[derive(Debug, thiserror::Error)]
pub enum LocalSttError {
    #[error("engine failure: {0}")]
    Engine(String),
    #[error("model not available: {0}")]
    ModelUnavailable(String),
}

/// Benchmark measurement + verdict.
#[derive(Debug, Clone, PartialEq)]
pub struct BenchmarkOutcome {
    /// Real-time factor: total processing time / total audio time (lower is better).
    pub rtf: f64,
    /// p95 processing latency per segment (ms).
    pub p95_ms: u64,
    pub passed: bool,
}

/// p95 of a latency sample set (ms). Empty input → 0.
pub fn p95_ms(mut samples: Vec<u64>) -> u64 {
    if samples.is_empty() {
        return 0;
    }
    samples.sort_unstable();
    // Nearest-rank yöntemi: ceil(0.95 * n) - 1 indeksi.
    let rank = ((samples.len() as f64) * 0.95).ceil() as usize;
    samples[rank.saturating_sub(1).min(samples.len() - 1)]
}

/// Runs the engine over the sample segments and scores it against the server
/// policy: pass = `rtf <= rtf_max` AND `p95 <= p95_max_ms`. The p95 bound
/// reuses the runtime watchdog threshold — an engine that cannot beat the
/// watchdog at idle would only ever stream through the cloud fallback.
pub fn run_benchmark(
    engine: &mut dyn LocalSttEngine,
    segments: &[Vec<i16>],
    sample_rate_hz: u32,
    rtf_max: f64,
    p95_max_ms: u64,
) -> Result<BenchmarkOutcome, LocalSttError> {
    if segments.is_empty() || sample_rate_hz == 0 {
        return Err(LocalSttError::Engine("empty benchmark sample".into()));
    }

    let mut latencies = Vec::with_capacity(segments.len());
    let mut total_audio_secs = 0.0_f64;
    let started = Instant::now();

    for segment in segments {
        let seg_started = Instant::now();
        engine.transcribe(segment)?;
        latencies.push(seg_started.elapsed().as_millis() as u64);
        total_audio_secs += segment.len() as f64 / sample_rate_hz as f64;
    }

    let total_processing_secs = started.elapsed().as_secs_f64();
    let rtf = total_processing_secs / total_audio_secs;
    let p95 = p95_ms(latencies);

    Ok(BenchmarkOutcome {
        rtf,
        p95_ms: p95,
        passed: rtf <= rtf_max && p95 <= p95_max_ms,
    })
}

/// Test/geliştirme motoru: sabit metin döner, istenirse yapay gecikme ekler.
pub struct MockEngine {
    pub text: String,
    pub delay_ms: u64,
    pub fail: bool,
}

impl MockEngine {
    pub fn instant(text: impl Into<String>) -> Self {
        MockEngine {
            text: text.into(),
            delay_ms: 0,
            fail: false,
        }
    }
}

impl LocalSttEngine for MockEngine {
    fn transcribe(&mut self, _pcm: &[i16]) -> Result<String, LocalSttError> {
        if self.fail {
            return Err(LocalSttError::Engine("mock failure".into()));
        }
        if self.delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(self.delay_ms));
        }
        Ok(self.text.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1 saniyelik sessiz PCM segmenti (16 kHz mono).
    fn one_second_segment() -> Vec<i16> {
        vec![0; 16_000]
    }

    #[test]
    fn p95_nearest_rank() {
        assert_eq!(p95_ms(vec![]), 0);
        assert_eq!(p95_ms(vec![100]), 100);
        // 20 örnek: p95 = 19. sıradaki (küçükten büyüğe).
        let samples: Vec<u64> = (1..=20).collect();
        assert_eq!(p95_ms(samples), 19);
    }

    #[test]
    fn fast_engine_passes_benchmark() {
        let mut engine = MockEngine::instant("merhaba");
        let segments = vec![one_second_segment(), one_second_segment()];
        let out = run_benchmark(&mut engine, &segments, 16_000, 0.5, 1500).expect("benchmark");
        assert!(out.passed, "hızlı motor geçmeli: {out:?}");
        assert!(out.rtf < 0.5);
    }

    #[test]
    fn slow_engine_fails_rtf_bound() {
        // 1 saniyelik ses başına 60 ms işlem, ama rtf sınırı 0.01 → kalır.
        let mut engine = MockEngine {
            text: "yavaş".into(),
            delay_ms: 60,
            fail: false,
        };
        let segments = vec![one_second_segment()];
        let out = run_benchmark(&mut engine, &segments, 16_000, 0.01, 10_000).expect("benchmark");
        assert!(!out.passed);
    }

    #[test]
    fn engine_error_propagates() {
        let mut engine = MockEngine {
            text: String::new(),
            delay_ms: 0,
            fail: true,
        };
        let segments = vec![one_second_segment()];
        assert!(run_benchmark(&mut engine, &segments, 16_000, 0.5, 1500).is_err());
    }

    #[test]
    fn empty_sample_rejected() {
        let mut engine = MockEngine::instant("x");
        assert!(run_benchmark(&mut engine, &[], 16_000, 0.5, 1500).is_err());
    }
}
