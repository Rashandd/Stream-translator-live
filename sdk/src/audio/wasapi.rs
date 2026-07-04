//! Windows WASAPI capture adapter — gerçek cihaz I/O (1.3).
//!
//! Varsayılan kayıt cihazını (mikrofon) shared-mode + event-driven açar ve
//! `AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM` ile ses motoruna 16 kHz mono s16le'ye
//! dönüştürtür — çekirdeğin beklediği format (bkz. `core::capture`), ayrıca
//! resampler gerekmez.
//!
//! İş parçacığı modeli: tüm COM/WASAPI nesneleri `open()` içinde başlatılan
//! özel bir capture thread'inde yaşar (COM thread-affinity'si dışarı sızmaz);
//! [`CaptureSource`] tarafı yalnız bir mpsc alıcısı tutar, bu yüzden `Send`.
//! Tüketici yavaş kalırsa kare DÜŞÜRÜLÜR (canlı yayında gecikme birikmesi
//! kabul edilemez) ve düşen kare sayısı loglanır.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{AudioAdapterInfo, AudioBackend};
use crate::core::capture::{CaptureConfig, CaptureError, CaptureSource};

/// Kuyrukta bekleyebilecek azami kare (20 ms/kare → ~1.3 sn tampon).
const CHANNEL_CAPACITY: usize = 64;
/// WASAPI paylaşımlı tampon süresi (100 ns birimi) — 200 ms.
const BUFFER_DURATION_HNS: i64 = 2_000_000;
/// Event beklemesi zaman aşımı (ms) — stop bayrağının denetim aralığı.
const EVENT_TIMEOUT_MS: u32 = 100;

pub fn adapter_info() -> AudioAdapterInfo {
    AudioAdapterInfo {
        backend: AudioBackend::Wasapi,
        enabled: true,
    }
}

/// Capture thread'inden gelen kareleri aktaran kaynak.
struct WasapiCapture {
    config: CaptureConfig,
    frames: Receiver<Vec<i16>>,
    stop: Arc<AtomicBool>,
    /// Thread ölümcül hatayla bittiyse mesajı buraya bırakır.
    error: Arc<Mutex<Option<String>>>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl CaptureSource for WasapiCapture {
    fn config(&self) -> CaptureConfig {
        self.config
    }

    fn next_frame(&mut self) -> Result<Option<Vec<i16>>, CaptureError> {
        loop {
            match self.frames.recv_timeout(Duration::from_secs(5)) {
                Ok(frame) => return Ok(Some(frame)),
                Err(RecvTimeoutError::Timeout) => {
                    // Cihaz event üretmiyor ama thread yaşıyorsa beklemeye
                    // devam (sessiz kaynak değil — WASAPI sessizlikte de
                    // event üretir; 5 sn tam sessizlik anormaldir, logla).
                    tracing::warn!("wasapi: no frames for 5s, still waiting");
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

impl Drop for WasapiCapture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Varsayılan kayıt cihazında bir WASAPI capture akışı açar.
///
/// Cihaz açılışı senkron doğrulanır: thread ilk kurulum hatasını hemen
/// raporlar, `open()` bu durumda `Err(Device)` döner — çağıran "açıldı ama
/// hiç ses gelmiyor" belirsizliğine düşmez.
pub fn open(config: CaptureConfig) -> Result<Box<dyn CaptureSource>, CaptureError> {
    let (frame_tx, frame_rx) = std::sync::mpsc::sync_channel::<Vec<i16>>(CHANNEL_CAPACITY);
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);
    let stop = Arc::new(AtomicBool::new(false));
    let error = Arc::new(Mutex::new(None::<String>));

    let worker = {
        let stop = Arc::clone(&stop);
        let error = Arc::clone(&error);
        std::thread::Builder::new()
            .name("wasapi-capture".into())
            .spawn(move || {
                if let Err(message) = capture_thread(config, frame_tx, ready_tx, stop) {
                    tracing::error!(%message, "wasapi capture thread failed");
                    if let Ok(mut slot) = error.lock() {
                        *slot = Some(message);
                    }
                }
            })
            .map_err(|e| CaptureError::Device(format!("capture thread spawn failed: {e}")))?
    };

    // Kurulumun sonucunu bekle (cihaz yok / format reddi burada yakalanır).
    match ready_rx.recv() {
        Ok(Ok(())) => Ok(Box::new(WasapiCapture {
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

/// COM + WASAPI yaşam döngüsünün tamamı bu thread'de.
fn capture_thread(
    config: CaptureConfig,
    frames: SyncSender<Vec<i16>>,
    ready: SyncSender<Result<(), String>>,
    stop: Arc<AtomicBool>,
) -> Result<(), String> {
    use wasapi::{DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat};

    // Bu thread için COM (MTA). Hata da olsa thread sonunda deinitialize.
    wasapi::initialize_mta()
        .ok()
        .map_err(|e| format!("COM init failed: {e}"))?;
    let result = (|| -> Result<(), String> {
        let enumerator =
            DeviceEnumerator::new().map_err(|e| format!("device enumerator: {e}"))?;
        let device = enumerator
            .get_default_device(&Direction::Capture)
            .map_err(|e| format!("no default capture device: {e}"))?;
        let name = device
            .get_friendlyname()
            .unwrap_or_else(|_| "<unknown>".to_string());
        tracing::info!(device = %name, "wasapi: opening default capture device");

        let mut audio_client = device
            .get_iaudioclient()
            .map_err(|e| format!("audio client: {e}"))?;

        // Çekirdek formatı: s16le, autoconvert ses motoruna dönüştürtür.
        let format = WaveFormat::new(
            16,
            16,
            &SampleType::Int,
            config.sample_rate as usize,
            config.channels as usize,
            None,
        );
        audio_client
            .initialize_client(
                &format,
                &Direction::Capture,
                &StreamMode::EventsShared {
                    autoconvert: true,
                    buffer_duration_hns: BUFFER_DURATION_HNS,
                },
            )
            .map_err(|e| format!("client init ({}Hz/{}ch s16): {e}", config.sample_rate, config.channels))?;

        let event = audio_client
            .set_get_eventhandle()
            .map_err(|e| format!("event handle: {e}"))?;
        let capture_client = audio_client
            .get_audiocaptureclient()
            .map_err(|e| format!("capture client: {e}"))?;
        audio_client
            .start_stream()
            .map_err(|e| format!("start stream: {e}"))?;

        // Kurulum tamam — open() dönebilir.
        let _ = ready.send(Ok(()));

        let bytes_per_sample = 2usize;
        let frame_bytes = config.frame_samples * config.channels as usize * bytes_per_sample;
        let mut byte_queue: VecDeque<u8> = VecDeque::with_capacity(frame_bytes * 8);
        let mut dropped: u64 = 0;

        while !stop.load(Ordering::Relaxed) {
            // Timeout stop bayrağını yoklamak için; timeout hata değildir.
            if event.wait_for_event(EVENT_TIMEOUT_MS).is_err() {
                continue;
            }
            capture_client
                .read_from_device_to_deque(&mut byte_queue)
                .map_err(|e| format!("device read: {e}"))?;

            while byte_queue.len() >= frame_bytes {
                let mut frame = Vec::with_capacity(config.frame_samples);
                for _ in 0..config.frame_samples * config.channels as usize {
                    let lo = byte_queue.pop_front().unwrap_or(0);
                    let hi = byte_queue.pop_front().unwrap_or(0);
                    frame.push(i16::from_le_bytes([lo, hi]));
                }
                match frames.try_send(frame) {
                    Ok(()) => {}
                    Err(TrySendError::Full(_)) => {
                        // Canlıda gecikme biriktirmek yerine kare düşür.
                        dropped += 1;
                        if dropped.is_power_of_two() {
                            tracing::warn!(dropped, "wasapi: consumer slow, dropping frames");
                        }
                    }
                    Err(TrySendError::Disconnected(_)) => {
                        // Tüketici gitti — temiz kapanış.
                        let _ = audio_client.stop_stream();
                        return Ok(());
                    }
                }
            }
        }
        let _ = audio_client.stop_stream();
        Ok(())
    })();

    // Kurulum aşamasında düştüysek open()'a haber ver (kanal doluysa zaten
    // ready gönderilmişti, ikinci gönderim sessizce düşer).
    if let Err(message) = &result {
        let _ = ready.try_send(Err(message.clone()));
    }
    wasapi::deinitialize();
    result
}
