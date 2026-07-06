//! Local TTS engine abstraction (phase 2.2b — mirrors `local_stt.rs` for the
//! synthesis direction).
//!
//! The real model binding (XTTS-v2/Coqui, decision per docs/local-stt-plan.md
//! §4's (a)/(b) fork, same choice applies here) plugs in behind
//! [`LocalTtsEngine`]; the core stays model-agnostic and fully testable with
//! [`MockTtsEngine`]. The benchmark runner measures the engine against
//! server-policy thresholds (`GET /api/tts/policy`) and produces the report
//! the SDK posts to `POST /api/tts/benchmark`.

use std::time::Instant;

use super::local_stt::p95_ms;

/// Local text-to-speech engine producing 16 kHz mono s16le PCM — the same
/// format the cloud path (ElevenLabs) effectively stands in for once
/// decoded, so downstream muxing doesn't need to know which path ran.
pub trait LocalTtsEngine: Send {
    /// Synthesizes one line of text. Blocking (runs on a worker thread).
    fn synthesize(&mut self, text: &str) -> Result<Vec<i16>, LocalTtsError>;
}

/// Local engine errors.
#[derive(Debug, thiserror::Error)]
pub enum LocalTtsError {
    #[error("engine failure: {0}")]
    Engine(String),
    #[error("model not available: {0}")]
    ModelUnavailable(String),
}

/// Benchmark measurement + verdict.
#[derive(Debug, Clone, PartialEq)]
pub struct BenchmarkOutcome {
    /// Real-time factor: processing time / synthesized audio duration
    /// (lower is better) — same definition as STT's RTF, just measured
    /// against the engine's OUTPUT length instead of its input length.
    pub rtf: f64,
    /// p95 processing latency per line (ms).
    pub p95_ms: u64,
    pub passed: bool,
}

/// Runs the engine over sample lines of text and scores it against the
/// server policy: pass = `rtf <= rtf_max` AND `p95 <= p95_max_ms`.
pub fn run_benchmark(
    engine: &mut dyn LocalTtsEngine,
    texts: &[String],
    sample_rate_hz: u32,
    rtf_max: f64,
    p95_max_ms: u64,
) -> Result<BenchmarkOutcome, LocalTtsError> {
    if texts.is_empty() || sample_rate_hz == 0 {
        return Err(LocalTtsError::Engine("empty benchmark sample".into()));
    }

    let mut latencies = Vec::with_capacity(texts.len());
    let mut total_audio_secs = 0.0_f64;
    let started = Instant::now();

    for text in texts {
        let line_started = Instant::now();
        let pcm = engine.synthesize(text)?;
        latencies.push(line_started.elapsed().as_millis() as u64);
        total_audio_secs += pcm.len() as f64 / sample_rate_hz as f64;
    }

    if total_audio_secs == 0.0 {
        return Err(LocalTtsError::Engine("engine produced no audio".into()));
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

/// Test/geliştirme motoru: her metin için sabit uzunlukta sessizlik döner,
/// istenirse yapay gecikme ekler.
pub struct MockTtsEngine {
    /// Üretilen örnek sayısı (16 kHz'te ~1 sn için 16_000).
    pub samples_per_call: usize,
    pub delay_ms: u64,
    pub fail: bool,
}

impl MockTtsEngine {
    pub fn instant(samples_per_call: usize) -> Self {
        MockTtsEngine {
            samples_per_call,
            delay_ms: 0,
            fail: false,
        }
    }
}

impl LocalTtsEngine for MockTtsEngine {
    fn synthesize(&mut self, _text: &str) -> Result<Vec<i16>, LocalTtsError> {
        if self.fail {
            return Err(LocalTtsError::Engine("mock failure".into()));
        }
        if self.delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(self.delay_ms));
        }
        Ok(vec![0; self.samples_per_call])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_second_lines(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("line {i}")).collect()
    }

    #[test]
    fn fast_engine_passes_benchmark() {
        let mut engine = MockTtsEngine::instant(16_000); // 1 sn ses üretir
        let texts = one_second_lines(2);
        let out = run_benchmark(&mut engine, &texts, 16_000, 0.5, 1500).expect("benchmark");
        assert!(out.passed, "hızlı motor geçmeli: {out:?}");
        assert!(out.rtf < 0.5);
    }

    #[test]
    fn slow_engine_fails_rtf_bound() {
        let mut engine = MockTtsEngine {
            samples_per_call: 16_000,
            delay_ms: 60,
            fail: false,
        };
        let texts = one_second_lines(1);
        let out = run_benchmark(&mut engine, &texts, 16_000, 0.01, 10_000).expect("benchmark");
        assert!(!out.passed);
    }

    #[test]
    fn engine_error_propagates() {
        let mut engine = MockTtsEngine {
            samples_per_call: 16_000,
            delay_ms: 0,
            fail: true,
        };
        assert!(run_benchmark(&mut engine, &one_second_lines(1), 16_000, 0.5, 1500).is_err());
    }

    #[test]
    fn empty_sample_rejected() {
        let mut engine = MockTtsEngine::instant(16_000);
        assert!(run_benchmark(&mut engine, &[], 16_000, 0.5, 1500).is_err());
    }

    #[test]
    fn engine_producing_no_audio_is_rejected() {
        let mut engine = MockTtsEngine::instant(0);
        assert!(run_benchmark(&mut engine, &one_second_lines(1), 16_000, 0.5, 1500).is_err());
    }
}
