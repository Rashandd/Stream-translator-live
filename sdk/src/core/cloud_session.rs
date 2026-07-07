//! Wires the SDK's already-built pieces (capture → preprocess → VAD →
//! framing, `core::cloud::CloudClient`) into one running session: fetch the
//! broadcaster's saved language settings, open the cloud WebSocket, and pump
//! microphone audio to the backend while surfacing subtitles.
//!
//! Before this module, every piece existed and was tested in isolation but
//! nothing drove them together — `run_desktop_shell` only started the RTMP
//! relay. See `docs/sessions/claude.md` for the gap this closes.

use std::collections::HashMap;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde::Deserialize;
use tokio::sync::mpsc;

use super::capture::CaptureSource;
use super::cloud::{CloudClient, CloudError, DubbedAudioTrack, ServerMessage};
use super::gate::CapturePipeline;
use super::preprocess::Preprocessor;
use super::stream::{StreamConfig, StreamConfigError};
use super::vad::VadGate;

/// Capacity of the channel bridging the blocking capture thread into the
/// async cloud loop. Small on purpose — same rationale as the RTMP relay's
/// fan-out channel (drop stale audio rather than build up latency).
const FRAME_CHANNEL_CAPACITY: usize = 64;

/// How long to keep reading after capture ends / the socket signals it has
/// nothing more buffered, to avoid dropping a subtitle that was already in
/// flight for the last frame sent.
const DRAIN_WINDOW: Duration = Duration::from_millis(500);

#[derive(Debug, thiserror::Error)]
pub enum CloudSessionError {
    #[error("http request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("server rejected the request: {0}")]
    Server(reqwest::StatusCode),
    #[error("no target languages configured — set them on the dashboard first")]
    NoTargetLanguages,
    #[error("invalid stream config: {0}")]
    Config(#[from] StreamConfigError),
    #[error("cloud connection error: {0}")]
    Cloud(#[from] CloudError),
    #[error("backend rejected the stream: {code} {message}")]
    Backend { code: u16, message: String },
}

/// The subset of `GET /api/translate/languages` this module cares about.
#[derive(Debug, Deserialize)]
struct RemoteLanguageSettings {
    source_lang: String,
    target_langs: Vec<String>,
}

/// Fetches the broadcaster's saved source/target languages — the same
/// settings the dashboard's language picker writes via `PUT`. Mirrors
/// `relay_sync::fetch_relay_targets`'s auth pattern (device JWT as bearer).
async fn fetch_stream_settings(
    http: &reqwest::Client,
    base_url: &str,
    device_token: &str,
) -> Result<(String, Vec<String>), CloudSessionError> {
    let url = format!("{}/api/translate/languages", base_url.trim_end_matches('/'));
    let response = http.get(&url).bearer_auth(device_token).send().await?;
    if !response.status().is_success() {
        return Err(CloudSessionError::Server(response.status()));
    }
    let settings: RemoteLanguageSettings = response.json().await?;
    Ok((settings.source_lang, settings.target_langs))
}

/// Converts an `http(s)://` backend base URL into the `ws(s)://.../ws/stream`
/// endpoint. Already-`ws(s)://` input passes through unchanged.
fn ws_url(base_url: &str) -> String {
    let ws_base = if let Some(rest) = base_url.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = base_url.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        base_url.to_string()
    };
    format!("{}/ws/stream", ws_base.trim_end_matches('/'))
}

/// A session-local, process-unique id — not a secret, just needs to not
/// collide with a concurrent session in the same process.
fn generate_session_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("sdk-session-{nanos:x}")
}

/// What happened during a `run_cloud_session` call — enough for tests (and
/// future observability) to assert real behavior instead of just "no error".
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CloudSessionOutcome {
    pub subtitles_received: u64,
}

/// Decodes a base64 dub-audio payload (`gateway::elevenlabs`'s `pcm_24000` —
/// raw PCM s16le, no container/codec to decode) into `i16` samples. Returns
/// `None` on malformed base64 rather than panicking — a corrupt dub track
/// should drop silently (silence keeps playing at the target), not take the
/// whole session down.
fn decode_dub_audio(audio_base64: &str) -> Option<Vec<i16>> {
    let bytes = B64.decode(audio_base64).ok()?;
    Some(
        bytes
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect(),
    )
}

/// Routes one dubbed-audio track to the matching dub target's mux scheduler
/// (see `rtmp::relay::run_dub_target` / `RelayEngine::dub_audio_sinks`) —
/// best-effort: no sink registered for this language, malformed base64, or a
/// closed sink all just drop the track (silence continues at that target)
/// rather than erroring the whole cloud session.
async fn route_dub_audio(sinks: &HashMap<String, mpsc::Sender<Vec<i16>>>, track: &DubbedAudioTrack) {
    let Some(sink) = sinks.get(&track.lang) else {
        return;
    };
    let Some(samples) = decode_dub_audio(&track.audio_base64) else {
        tracing::warn!(lang = %track.lang, "cloud session: dub audio base64 decode failed, dropping");
        return;
    };
    if sink.send(samples).await.is_err() {
        tracing::warn!(lang = %track.lang, "cloud session: dub audio sink closed, dropping");
    }
}

/// Runs one cloud audio session end-to-end: fetch languages, connect,
/// authenticate, `stream_start`, then pump mic audio in and read subtitles
/// back until the capture source ends or the connection closes. Dub audio
/// carried on `Subtitle` messages is routed to `dub_audio_sinks` by language
/// (empty map = no dub targets, today's default — nothing changes for
/// broadcasters without one configured).
///
/// `capture` is dependency-injected so tests can pass a [`super::capture::MockCapture`]
/// — production callers pass `audio::open_capture(CaptureConfig::default())`.
pub async fn run_cloud_session(
    http: &reqwest::Client,
    backend_base_url: &str,
    device_token: &str,
    capture: Box<dyn CaptureSource>,
    dub_audio_sinks: HashMap<String, mpsc::Sender<Vec<i16>>>,
) -> Result<CloudSessionOutcome, CloudSessionError> {
    let (source_lang, target_langs) = fetch_stream_settings(http, backend_base_url, device_token).await?;
    if target_langs.is_empty() {
        return Err(CloudSessionError::NoTargetLanguages);
    }

    let cfg = StreamConfig::new(
        generate_session_id(),
        source_lang,
        target_langs,
        super::audio_frame::AudioCodec::PcmS16Le,
    );
    cfg.validate()?;

    let mut client = CloudClient::connect(&ws_url(backend_base_url)).await?;
    client.authenticate(device_token).await?;
    client.start_stream(&cfg).await?;

    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(FRAME_CHANNEL_CAPACITY);
    // Capture is blocking (thread/mpsc-backed device I/O) — pump it on the
    // blocking pool and bridge into the async world via a channel. Not
    // awaited below on purpose: on a live mic it may not notice the channel
    // closed until the next voiced frame, so joining it could hang the
    // caller past session end. Detaching is harmless — it just winds down
    // in the background (same "fire and forget" posture as stt_control's
    // poll loop, which run_desktop_shell also never joins).
    tokio::task::spawn_blocking(move || {
        let mut pipeline = CapturePipeline::new(capture, VadGate::default())
            .with_preprocessor(Preprocessor::default());
        loop {
            match pipeline.next_frame() {
                Ok(Some(frame)) => {
                    if tx.blocking_send(frame).is_err() {
                        break; // receiver dropped — session ending
                    }
                }
                Ok(None) => break, // capture source exhausted
                Err(err) => {
                    tracing::warn!(%err, "cloud session: capture pipeline error, stopping capture");
                    break;
                }
            }
        }
    });

    let mut outcome = CloudSessionOutcome::default();

    loop {
        tokio::select! {
            frame = rx.recv() => {
                match frame {
                    Some(frame) => {
                        client.send_audio(&frame).await?;
                    }
                    None => break,
                }
            }
            msg = client.next_message() => {
                match msg? {
                    Some(ServerMessage::Subtitle { dubbed_audio, .. }) => {
                        outcome.subtitles_received += 1;
                        for track in &dubbed_audio {
                            route_dub_audio(&dub_audio_sinks, track).await;
                        }
                    }
                    Some(ServerMessage::Error { code, message, .. }) => {
                        return Err(CloudSessionError::Backend { code, message });
                    }
                    Some(_) => {}
                    None => break,
                }
            }
        }
    }

    // Capture ended or the socket signalled it's drained — briefly keep
    // reading in case a subtitle for the last frame sent is still in flight
    // (it may not have been read yet even though it already arrived).
    loop {
        match tokio::time::timeout(DRAIN_WINDOW, client.next_message()).await {
            Ok(Ok(Some(ServerMessage::Subtitle { dubbed_audio, .. }))) => {
                outcome.subtitles_received += 1;
                for track in &dubbed_audio {
                    route_dub_audio(&dub_audio_sinks, track).await;
                }
            }
            Ok(Ok(Some(_))) => {}
            Ok(Ok(None)) | Ok(Err(_)) | Err(_) => break,
        }
    }

    let _ = client.stop_stream(&cfg.session_id).await;
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_url_converts_http_schemes() {
        assert_eq!(ws_url("http://127.0.0.1:8080"), "ws://127.0.0.1:8080/ws/stream");
        assert_eq!(ws_url("https://api.strovak.com"), "wss://api.strovak.com/ws/stream");
        assert_eq!(ws_url("http://127.0.0.1:8080/"), "ws://127.0.0.1:8080/ws/stream");
    }

    #[test]
    fn ws_url_passes_through_already_ws_schemes() {
        assert_eq!(ws_url("ws://127.0.0.1:8080"), "ws://127.0.0.1:8080/ws/stream");
    }

    #[test]
    fn generate_session_id_is_unique_across_calls() {
        let a = generate_session_id();
        let b = generate_session_id();
        assert_ne!(a, b);
        assert!(a.starts_with("sdk-session-"));
    }
}
