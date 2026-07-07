//! macOS + Linux capture adapter — cpal-backed (1.3 follow-up).
//!
//! cpal is a mature, widely-used cross-platform audio I/O crate covering
//! CoreAudio (macOS) and ALSA/PulseAudio/JACK (Linux) behind one API —
//! deliberately chosen over hand-rolled per-platform FFI (as `wasapi.rs`
//! does for Windows) because the macOS path cannot be compile-checked at
//! all in this repo's environment today (no darwin target/SDK available);
//! resting on a battle-tested crate is safer for code nobody here can build
//! or run. See `docs/audio-pipeline.md` for the full tradeoff writeup.
//!
//! Thread model mirrors `wasapi.rs`: cpal's `Stream` handle is not `Send` on
//! every backend, so it's built and lives entirely inside a dedicated
//! capture thread; only an mpsc `Receiver` crosses back out, which is what
//! makes [`CpalCapture`] itself `Send`.
//!
//! cpal does not resample or downmix for you — it only lets you request a
//! config the device actually reports as supported, which is rarely exactly
//! 16 kHz mono. So capture opens the device's own default input config
//! (whatever native rate/channels/sample format it reports) and a small
//! linear-interpolation resampler + channel-average downmix converts that
//! to the 16 kHz mono i16 the core pipeline expects (see `core::capture`).
//! Linear interpolation is not audiophile-grade, but it's more than adequate
//! for STT and matches this codebase's existing preference for small
//! hand-rolled DSP over a heavy dependency (see `core::preprocess`).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};

use super::{AudioAdapterInfo, AudioBackend};
use crate::core::capture::{CaptureConfig, CaptureError, CaptureSource};

/// Kuyrukta bekleyebilecek azami kare (20 ms/kare → ~1.3 sn tampon) — wasapi.rs ile aynı.
const CHANNEL_CAPACITY: usize = 64;
/// Cihaz durumu denetim aralığı (stop bayrağı / async hata).
const POLL_INTERVAL: Duration = Duration::from_millis(100);

#[cfg(target_os = "macos")]
pub fn adapter_info() -> AudioAdapterInfo {
    AudioAdapterInfo {
        backend: AudioBackend::CoreAudio,
        enabled: true,
    }
}

#[cfg(target_os = "linux")]
pub fn adapter_info() -> AudioAdapterInfo {
    AudioAdapterInfo {
        backend: AudioBackend::PipeWire,
        enabled: true,
    }
}

/// Capture thread'inden gelen kareleri aktaran kaynak.
struct CpalCapture {
    config: CaptureConfig,
    frames: Receiver<Vec<i16>>,
    stop: Arc<AtomicBool>,
    /// Thread (ya da async cpal hata callback'i) ölümcül hatayla bittiyse
    /// mesajı buraya bırakır.
    error: Arc<Mutex<Option<String>>>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl CaptureSource for CpalCapture {
    fn config(&self) -> CaptureConfig {
        self.config
    }

    fn next_frame(&mut self) -> Result<Option<Vec<i16>>, CaptureError> {
        loop {
            match self.frames.recv_timeout(Duration::from_secs(5)) {
                Ok(frame) => return Ok(Some(frame)),
                Err(RecvTimeoutError::Timeout) => {
                    tracing::warn!("cpal: no frames for 5s, still waiting");
                }
                Err(RecvTimeoutError::Disconnected) => {
                    let err = self.error.lock().ok().and_then(|mut e| e.take());
                    return match err {
                        Some(message) => Err(CaptureError::Device(message)),
                        None => Ok(None), // temiz kapanış
                    };
                }
            }
        }
    }
}

impl Drop for CpalCapture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Varsayılan giriş cihazında bir cpal capture akışı açar.
///
/// Cihaz açılışı senkron doğrulanır: thread ilk kurulum hatasını hemen
/// raporlar, `open()` bu durumda `Err(Device)` döner (wasapi.rs ile aynı
/// sözleşme — çağıran "açıldı ama hiç ses gelmiyor" belirsizliğine düşmez).
pub fn open(config: CaptureConfig) -> Result<Box<dyn CaptureSource>, CaptureError> {
    let (frame_tx, frame_rx) = std::sync::mpsc::sync_channel::<Vec<i16>>(CHANNEL_CAPACITY);
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);
    let stop = Arc::new(AtomicBool::new(false));
    let error = Arc::new(Mutex::new(None::<String>));

    let worker = {
        let stop = Arc::clone(&stop);
        let error = Arc::clone(&error);
        std::thread::Builder::new()
            .name("cpal-capture".into())
            .spawn(move || {
                if let Err(message) = capture_thread(config, frame_tx, ready_tx, Arc::clone(&stop), Arc::clone(&error))
                {
                    tracing::error!(%message, "cpal capture thread failed");
                    if let Ok(mut slot) = error.lock() {
                        *slot = Some(message);
                    }
                }
            })
            .map_err(|e| CaptureError::Device(format!("capture thread spawn failed: {e}")))?
    };

    match ready_rx.recv() {
        Ok(Ok(())) => Ok(Box::new(CpalCapture {
            config,
            frames: frame_rx,
            stop,
            error,
            worker: Some(worker),
        })),
        Ok(Err(message)) => {
            let _ = worker.join();
            Err(CaptureError::Device(message))
        }
        Err(_) => {
            let _ = worker.join();
            let message = error
                .lock()
                .ok()
                .and_then(|mut e| e.take())
                .unwrap_or_else(|| "capture thread died during setup".to_string());
            Err(CaptureError::Device(message))
        }
    }
}

/// cpal stream'inin tüm yaşam döngüsü bu thread'de (Stream `Send` değil).
fn capture_thread(
    config: CaptureConfig,
    frames: SyncSender<Vec<i16>>,
    ready: SyncSender<Result<(), String>>,
    stop: Arc<AtomicBool>,
    async_error: Arc<Mutex<Option<String>>>,
) -> Result<(), String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "no default input device".to_string())?;
    let name = device.name().unwrap_or_else(|_| "<unknown>".to_string());

    let supported = device
        .default_input_config()
        .map_err(|e| format!("default input config: {e}"))?;
    let sample_format = supported.sample_format();
    let native_channels = supported.channels();
    let native_rate = supported.sample_rate().0;
    tracing::info!(
        device = %name,
        native_rate,
        channels = native_channels,
        ?sample_format,
        "cpal: opening default capture device"
    );
    let stream_config: StreamConfig = supported.into();

    let converter = Arc::new(Mutex::new(FrameConverter::new(
        native_rate,
        native_channels,
        config,
        frames.clone(),
    )));

    let err_flag = Arc::clone(&async_error);
    let error_callback = move |err: cpal::StreamError| {
        if let Ok(mut slot) = err_flag.lock() {
            if slot.is_none() {
                *slot = Some(format!("device error: {err}"));
            }
        }
    };

    let stream = build_stream(&device, &stream_config, sample_format, converter, error_callback)?;
    stream.play().map_err(|e| format!("stream play: {e}"))?;

    let _ = ready.send(Ok(()));

    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(POLL_INTERVAL);
        if async_error.lock().ok().is_some_and(|slot| slot.is_some()) {
            break;
        }
    }
    drop(stream);
    Ok(())
}

/// Sample format'a göre doğru tipli cpal input stream'i kurar; her callback
/// veriyi ortak `FrameConverter`'a iletir.
fn build_stream(
    device: &cpal::Device,
    stream_config: &StreamConfig,
    sample_format: SampleFormat,
    converter: Arc<Mutex<FrameConverter>>,
    error_callback: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<cpal::Stream, String> {
    let make_err = |e: cpal::BuildStreamError| format!("build input stream: {e}");
    match sample_format {
        SampleFormat::F32 => device
            .build_input_stream(
                stream_config,
                move |data: &[f32], _| {
                    if let Ok(mut c) = converter.lock() {
                        c.push_f32(data);
                    }
                },
                error_callback,
                None,
            )
            .map_err(make_err),
        SampleFormat::I16 => device
            .build_input_stream(
                stream_config,
                move |data: &[i16], _| {
                    if let Ok(mut c) = converter.lock() {
                        c.push_i16(data);
                    }
                },
                error_callback,
                None,
            )
            .map_err(make_err),
        SampleFormat::U16 => device
            .build_input_stream(
                stream_config,
                move |data: &[u16], _| {
                    if let Ok(mut c) = converter.lock() {
                        c.push_u16(data);
                    }
                },
                error_callback,
                None,
            )
            .map_err(make_err),
        other => Err(format!("unsupported input sample format: {other:?}")),
    }
}

/// Ham cpal callback verisini (herhangi bir sample format, N kanal, native
/// hız) çekirdeğin beklediği 16 kHz mono i16 karelere çevirir: downmix →
/// resample → framing → `try_send` (tüketici yavaşsa kare düşürülür, aynı
/// wasapi.rs politikası).
struct FrameConverter {
    channels: u16,
    resampler: LinearResampler,
    /// Downmix edilmiş, henüz kareye bölünmemiş 16 kHz mono örnekler.
    pending: VecDeque<i16>,
    frame_samples: usize,
    frames: SyncSender<Vec<i16>>,
    dropped: u64,
}

impl FrameConverter {
    fn new(native_rate: u32, native_channels: u16, config: CaptureConfig, frames: SyncSender<Vec<i16>>) -> Self {
        FrameConverter {
            channels: native_channels.max(1),
            resampler: LinearResampler::new(native_rate, config.sample_rate),
            pending: VecDeque::new(),
            frame_samples: config.frame_samples,
            frames,
            dropped: 0,
        }
    }

    fn push_f32(&mut self, data: &[f32]) {
        self.push_mono(&downmix_f32(data, self.channels));
    }

    fn push_i16(&mut self, data: &[i16]) {
        let as_f32: Vec<f32> = data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
        self.push_mono(&downmix_f32(&as_f32, self.channels));
    }

    fn push_u16(&mut self, data: &[u16]) {
        let as_f32: Vec<f32> = data
            .iter()
            .map(|&s| (s as f32 - u16::MAX as f32 / 2.0) / (u16::MAX as f32 / 2.0))
            .collect();
        self.push_mono(&downmix_f32(&as_f32, self.channels));
    }

    fn push_mono(&mut self, mono_native: &[f32]) {
        let mut resampled = Vec::new();
        self.resampler.push_and_drain(mono_native, &mut resampled);
        self.pending.extend(resampled);

        while self.pending.len() >= self.frame_samples {
            let frame: Vec<i16> = self.pending.drain(..self.frame_samples).collect();
            match self.frames.try_send(frame) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {
                    self.dropped += 1;
                    if self.dropped.is_power_of_two() {
                        tracing::warn!(dropped = self.dropped, "cpal: consumer slow, dropping frames");
                    }
                }
                Err(TrySendError::Disconnected(_)) => {
                    // Tüketici gitti — bundan sonraki callback'ler no-op.
                    self.pending.clear();
                    return;
                }
            }
        }
    }
}

/// N kanallı interleaved örnekleri kanal ortalamasıyla mono'ya indirger.
fn downmix_f32(interleaved: &[f32], channels: u16) -> Vec<f32> {
    let channels = channels.max(1) as usize;
    if channels == 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
        .collect()
}

/// Basit doğrusal enterpolasyonlu resampler — audiophile kalitede değil ama
/// STT girdisi için yeterli (bkz. dosya başı doc yorumu). Çağrılar arasında
/// kesirli faz durumunu korur, bu yüzden callback sınırlarında tıklama/atlama
/// olmaz.
struct LinearResampler {
    /// Çıktı örneği başına girdi örneği (from_hz / to_hz). 1.0 ise resample
    /// gerekmez (aşağıda kısayol var).
    ratio: f64,
    passthrough: bool,
    buf: VecDeque<f32>,
    /// `buf` içine kesirli okuma konumu (0.0 == buf[0]).
    pos: f64,
}

impl LinearResampler {
    fn new(from_hz: u32, to_hz: u32) -> Self {
        LinearResampler {
            ratio: from_hz as f64 / to_hz as f64,
            passthrough: from_hz == to_hz,
            buf: VecDeque::new(),
            pos: 0.0,
        }
    }

    fn push_and_drain(&mut self, input: &[f32], out: &mut Vec<i16>) {
        if self.passthrough {
            out.extend(input.iter().map(|&s| to_i16(s)));
            return;
        }
        self.buf.extend(input.iter().copied());
        loop {
            let i0 = self.pos.floor() as usize;
            let i1 = i0 + 1;
            if i1 >= self.buf.len() {
                break; // bir sonraki çıktı örneğini enterpole etmek için daha fazla girdi gerek
            }
            let frac = self.pos - i0 as f64;
            let s0 = self.buf[i0] as f64;
            let s1 = self.buf[i1] as f64;
            out.push(to_i16((s0 + (s1 - s0) * frac) as f32));
            self.pos += self.ratio;
        }
        let drop_n = (self.pos.floor() as usize).min(self.buf.len().saturating_sub(1));
        for _ in 0..drop_n {
            self.buf.pop_front();
        }
        self.pos -= drop_n as f64;
    }
}

fn to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmix_averages_channels() {
        // 2ch: (1.0, -1.0), (0.5, 0.5) -> mono (0.0, 0.5)
        let out = downmix_f32(&[1.0, -1.0, 0.5, 0.5], 2);
        assert_eq!(out, vec![0.0, 0.5]);
    }

    #[test]
    fn downmix_is_noop_for_mono() {
        let out = downmix_f32(&[0.1, 0.2, 0.3], 1);
        assert_eq!(out, vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn resampler_passthrough_when_rates_match() {
        let mut r = LinearResampler::new(16_000, 16_000);
        let mut out = Vec::new();
        r.push_and_drain(&[0.0, 0.5, -0.5, 1.0], &mut out);
        assert_eq!(out.len(), 4);
        assert_eq!(out[3], i16::MAX);
    }

    #[test]
    fn resampler_downsamples_to_roughly_the_expected_length() {
        // 48kHz -> 16kHz is a 3:1 ratio.
        let mut r = LinearResampler::new(48_000, 16_000);
        let input = vec![0.0f32; 4800]; // 100ms @ 48kHz
        let mut out = Vec::new();
        r.push_and_drain(&input, &mut out);
        // ~1600 samples expected (100ms @ 16kHz); allow resampler edge slack.
        assert!(
            (1590..=1600).contains(&out.len()),
            "unexpected resampled length: {}",
            out.len()
        );
    }

    #[test]
    fn resampler_carries_phase_across_chunk_boundaries() {
        // Feeding the same total input in one call vs. many small calls
        // should yield (almost) the same amount of output — phase isn't
        // reset/lost at arbitrary chunk boundaries.
        let input: Vec<f32> = (0..4800).map(|i| (i as f32 / 4800.0).sin()).collect();

        let mut whole = LinearResampler::new(48_000, 16_000);
        let mut out_whole = Vec::new();
        whole.push_and_drain(&input, &mut out_whole);

        let mut chunked = LinearResampler::new(48_000, 16_000);
        let mut out_chunked = Vec::new();
        for chunk in input.chunks(97) {
            chunked.push_and_drain(chunk, &mut out_chunked);
        }

        assert!(
            (out_whole.len() as i64 - out_chunked.len() as i64).abs() <= 1,
            "whole={} chunked={}",
            out_whole.len(),
            out_chunked.len()
        );
    }

    #[test]
    fn to_i16_clamps_out_of_range_samples() {
        assert_eq!(to_i16(2.0), i16::MAX);
        assert_eq!(to_i16(-2.0), -i16::MAX);
    }

    /// Needs a real input device — not run by default (most CI runners have
    /// none). Run explicitly with `cargo test -- --ignored` on a machine
    /// with a microphone, same convention as this codebase's other
    /// real-hardware/real-service tests (e.g. backend's
    /// `users_persist_roundtrip_against_real_db`).
    #[test]
    #[ignore]
    fn opens_the_real_default_input_device_and_captures_frames() {
        let mut source = open(CaptureConfig::default()).expect("open default input device");
        for _ in 0..5 {
            let frame = source
                .next_frame()
                .expect("next_frame should not error")
                .expect("stream should not end while capturing");
            assert_eq!(frame.len(), CaptureConfig::default().frame_samples);
        }
    }
}
