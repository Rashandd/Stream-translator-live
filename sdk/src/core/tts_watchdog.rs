//! Runtime watchdog for local TTS (phase 2.2b — mirrors `stt_watchdog.rs`
//! for the synthesis direction; same deterministic decision state machine).
//!
//! The idle benchmark cannot measure game load, so the real protection runs
//! during the stream: if local synthesis falls behind, the SDK switches to
//! requesting cloud-synthesized audio (ElevenLabs) and retries local after a
//! hysteresis period.
//!
//! Pure decision state machine: `now` is injected, so every rule is
//! deterministic under test. Thresholds come from the server policy
//! (`GET /api/tts/policy` → `watchdog`), not from constants baked into the SDK.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use super::local_stt::p95_ms;

/// Watchdog thresholds (server policy → SDK).
#[derive(Debug, Clone)]
pub struct WatchdogConfig {
    /// p95 üstü sınır (ms) — pencere p95'i bunu aşarsa fallback.
    pub p95_ms: u64,
    /// p95 penceresi (plan: son 30 sn).
    pub window: Duration,
    /// "Tek başına yavaş" satır eşiği (plan: 2 sn).
    pub slow_segment_ms: u64,
    /// Bu kadar ardışık yavaş satır → fallback (plan: 3).
    pub consecutive_limit: u32,
    /// Fallback sonrası yerel tekrar deneme süresi (plan: 5 dk).
    pub retry: Duration,
    /// p95 kuralının tetiklenmesi için pencerede en az örnek sayısı.
    pub min_samples_for_p95: usize,
}

impl WatchdogConfig {
    /// Sunucu policy değerlerinden kurar; kalan alanlar plan sabitleri.
    pub fn from_policy(p95_ms: u64, retry_minutes: u64) -> Self {
        WatchdogConfig {
            p95_ms,
            window: Duration::from_secs(30),
            slow_segment_ms: 2_000,
            consecutive_limit: 3,
            retry: Duration::from_secs(retry_minutes * 60),
            min_samples_for_p95: 8,
        }
    }
}

/// Anlık TTS rotası.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TtsRoute {
    /// Yerel motor sesi kendi sentezliyor.
    Local,
    /// Metin buluta gidiyor (ElevenLabs sentezliyor).
    CloudFallback,
}

/// Yerel TTS sağlık watchdog'u.
pub struct TtsWatchdog {
    config: WatchdogConfig,
    /// Pencere içi (an, gecikme ms) örnekleri.
    samples: VecDeque<(Instant, u64)>,
    consecutive_slow: u32,
    /// Fallback'e geçiş anı (histerezis sayacı).
    fallback_since: Option<Instant>,
}

impl TtsWatchdog {
    pub fn new(config: WatchdogConfig) -> Self {
        TtsWatchdog {
            config,
            samples: VecDeque::new(),
            consecutive_slow: 0,
            fallback_since: None,
        }
    }

    /// Geçerli rota. Fallback'teyse ve retry süresi dolduysa yerel moda
    /// döner (pencere sıfırlanır — eski kötü ölçümler yeni denemeyi lekelemez).
    pub fn route(&mut self, now: Instant) -> TtsRoute {
        if let Some(since) = self.fallback_since {
            if now.duration_since(since) < self.config.retry {
                return TtsRoute::CloudFallback;
            }
            self.fallback_since = None;
            self.samples.clear();
            self.consecutive_slow = 0;
        }
        TtsRoute::Local
    }

    /// Yerel motor bir satırı sentezledikten sonra çağrılır; ölçümü
    /// kaydeder ve güncel rotayı döndürür.
    pub fn record(&mut self, now: Instant, latency_ms: u64) -> TtsRoute {
        if self.route(now) == TtsRoute::CloudFallback {
            return TtsRoute::CloudFallback;
        }

        if latency_ms > self.config.slow_segment_ms {
            self.consecutive_slow += 1;
        } else {
            self.consecutive_slow = 0;
        }

        self.samples.push_back((now, latency_ms));
        while let Some(&(at, _)) = self.samples.front() {
            if now.duration_since(at) > self.config.window {
                self.samples.pop_front();
            } else {
                break;
            }
        }

        let consecutive_tripped = self.consecutive_slow >= self.config.consecutive_limit;
        let p95_tripped = self.samples.len() >= self.config.min_samples_for_p95
            && p95_ms(self.samples.iter().map(|&(_, ms)| ms).collect()) > self.config.p95_ms;

        if consecutive_tripped || p95_tripped {
            self.fallback_since = Some(now);
            return TtsRoute::CloudFallback;
        }
        TtsRoute::Local
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> WatchdogConfig {
        // Not: gerçek seed varsayılanı (config.rs) 2000ms'dir; burada 1500
        // kullanılıyor çünkü `slow_segment_ms` sabiti 2000ms'de kilitli
        // (`from_policy`) — testler p95 kuralını "ardışık yavaş" kuralından
        // ayrıştırmak için 1500-2000 aralığındaki boşluğa ihtiyaç duyar
        // (stt_watchdog.rs'teki testlerle birebir aynı sayısal ilişki).
        WatchdogConfig::from_policy(1_500, 5)
    }

    #[test]
    fn healthy_latencies_stay_local() {
        let mut wd = TtsWatchdog::new(config());
        let t0 = Instant::now();
        for i in 0..50 {
            let now = t0 + Duration::from_millis(i * 500);
            assert_eq!(wd.record(now, 400), TtsRoute::Local);
        }
    }

    #[test]
    fn three_consecutive_slow_segments_trigger_fallback() {
        let mut wd = TtsWatchdog::new(config());
        let t0 = Instant::now();
        assert_eq!(wd.record(t0, 2_500), TtsRoute::Local);
        assert_eq!(wd.record(t0 + Duration::from_secs(1), 2_500), TtsRoute::Local);
        assert_eq!(
            wd.record(t0 + Duration::from_secs(2), 2_500),
            TtsRoute::CloudFallback
        );
        assert_eq!(
            wd.route(t0 + Duration::from_secs(3)),
            TtsRoute::CloudFallback
        );
    }

    #[test]
    fn fast_segment_resets_consecutive_counter() {
        let mut wd = TtsWatchdog::new(config());
        let t0 = Instant::now();
        wd.record(t0, 2_500);
        wd.record(t0 + Duration::from_secs(1), 2_500);
        wd.record(t0 + Duration::from_secs(2), 300);
        assert_eq!(
            wd.record(t0 + Duration::from_secs(3), 2_500),
            TtsRoute::Local
        );
    }

    #[test]
    fn window_p95_breach_triggers_fallback() {
        let mut wd = TtsWatchdog::new(config());
        let t0 = Instant::now();
        // 8+ örnek, hepsi 2 sn slow-segment sınırının altında ama p95 > 1500ms.
        let mut route = TtsRoute::Local;
        for i in 0..10 {
            let now = t0 + Duration::from_millis(i * 800);
            route = wd.record(now, 1_800);
        }
        assert_eq!(route, TtsRoute::CloudFallback);
    }

    #[test]
    fn p95_rule_needs_minimum_samples() {
        let mut wd = TtsWatchdog::new(config());
        let t0 = Instant::now();
        // 3 örnek 1800ms: p95 kuralı için az, ardışık kural için yavaş değil (<2000).
        for i in 0..3 {
            let now = t0 + Duration::from_secs(i);
            assert_eq!(wd.record(now, 1_800), TtsRoute::Local);
        }
    }

    #[test]
    fn old_samples_fall_out_of_the_window() {
        let mut wd = TtsWatchdog::new(config());
        let t0 = Instant::now();
        // 9 kötü örnek ama pencere (30 sn) dışına düşecek kadar seyrek.
        for i in 0..9 {
            let now = t0 + Duration::from_secs(i * 40);
            assert_eq!(wd.record(now, 1_800), TtsRoute::Local);
        }
    }

    #[test]
    fn retry_period_returns_to_local_with_clean_window() {
        let mut wd = TtsWatchdog::new(config());
        let t0 = Instant::now();
        for i in 0..3 {
            wd.record(t0 + Duration::from_secs(i), 2_500);
        }
        assert_eq!(wd.route(t0 + Duration::from_secs(10)), TtsRoute::CloudFallback);
        let after_retry = t0 + Duration::from_secs(2 + 5 * 60);
        assert_eq!(wd.route(after_retry), TtsRoute::Local);
        assert_eq!(wd.record(after_retry, 2_500), TtsRoute::Local);
    }
}
