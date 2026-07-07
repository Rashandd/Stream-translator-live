//! Fan-out orchestration: one ingest connection (OBS publishing locally) is
//! relayed to N destination platforms concurrently, each on its own outbound
//! connection with its own stream key. One target failing/reconnecting never
//! blocks the others or the ingest side.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;

use super::chunk::{write_message, RtmpMessage, DEFAULT_CHUNK_SIZE};
use super::message::{
    CSID_AUDIO, CSID_DATA, CSID_VIDEO, MSG_TYPE_AMF0_DATA, MSG_TYPE_AUDIO, MSG_TYPE_VIDEO,
};
use super::session::{accept_publisher, connect_publish, RtmpError};
use super::target::RelayTarget;
use crate::core::gate::pcm_to_le_bytes;
use crate::core::mux::audio_scheduler::DubAudioScheduler;
use crate::core::mux::ffmpeg_process::{FfmpegError, FfmpegProcess};
use crate::core::mux::flv::FlvWriter;

/// Fan-out channel backlog. Small on purpose: a target that falls behind
/// should drop stale media (see `RecvError::Lagged`) rather than stall the
/// whole relay for everyone else.
const CHANNEL_CAPACITY: usize = 256;
const RECONNECT_BACKOFF: Duration = Duration::from_secs(2);

/// Dub audio arrives in a handful of few-seconds utterances at a time — a
/// small buffer is plenty (the scheduler drains continuously).
const DUB_AUDIO_CHANNEL_CAPACITY: usize = 32;
/// Matches `gateway::elevenlabs`'s `pcm_24000` output format.
const DUB_PCM_SAMPLE_RATE: u32 = 24_000;
/// How often the dub audio scheduler is ticked to feed ffmpeg's PCM input —
/// same cadence as the capture side's frame size (`capture::DEFAULT_FRAME_SAMPLES`
/// is 20ms at 16kHz).
const DUB_PCM_TICK: Duration = Duration::from_millis(20);
/// Samples per tick at `DUB_PCM_SAMPLE_RATE` (24_000 Hz × 20ms = 480).
const DUB_PCM_SAMPLES_PER_TICK: usize = 480;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetState {
    Connecting,
    Live,
    Reconnecting,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetStatus {
    pub id: String,
    pub label: String,
    pub state: TargetState,
}

type StatusMap = Arc<Mutex<HashMap<String, TargetStatus>>>;

fn lock_statuses(
    m: &Mutex<HashMap<String, TargetStatus>>,
) -> MutexGuard<'_, HashMap<String, TargetStatus>> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Runs the ingest handshake, then relays media to every target. Owns the
/// per-target tasks; drop or call [`RelayEngine::shutdown`] to stop them.
pub struct RelayEngine {
    statuses: StatusMap,
    handles: Vec<JoinHandle<()>>,
    dub_audio_sinks: HashMap<String, mpsc::Sender<Vec<i16>>>,
}

impl RelayEngine {
    pub async fn start<S>(mut ingest: S, targets: Vec<RelayTarget>) -> Result<Self, RtmpError>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (mut reader, _publisher) = accept_publisher(&mut ingest).await?;
        let (tx, _rx) = broadcast::channel::<Arc<RtmpMessage>>(CHANNEL_CAPACITY);
        let statuses: StatusMap = Arc::new(Mutex::new(HashMap::new()));
        let mut dub_audio_sinks = HashMap::new();

        let mut handles = Vec::new();
        for target in targets {
            lock_statuses(&statuses).insert(
                target.id.clone(),
                TargetStatus {
                    id: target.id.clone(),
                    label: target.label.clone(),
                    state: TargetState::Connecting,
                },
            );
            let rx = tx.subscribe();
            let statuses = statuses.clone();
            match target.dub_language.clone() {
                Some(lang) => {
                    let (dub_tx, dub_rx) = mpsc::channel::<Vec<i16>>(DUB_AUDIO_CHANNEL_CAPACITY);
                    dub_audio_sinks.insert(lang, dub_tx);
                    handles.push(tokio::spawn(run_dub_target(target, rx, dub_rx, statuses)));
                }
                None => {
                    handles.push(tokio::spawn(run_target(target, rx, statuses)));
                }
            }
        }

        handles.push(tokio::spawn(async move {
            loop {
                match reader.read_message(&mut ingest).await {
                    Ok(msg) if is_media(msg.msg_type_id) => {
                        let _ = tx.send(Arc::new(msg));
                    }
                    Ok(_) => continue,
                    Err(_) => break,
                }
            }
        }));

        Ok(RelayEngine {
            statuses,
            handles,
            dub_audio_sinks,
        })
    }

    /// Snapshot of every target's current state, sorted by id.
    pub fn statuses(&self) -> Vec<TargetStatus> {
        let mut v: Vec<_> = lock_statuses(&self.statuses).values().cloned().collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }

    /// Per-language senders for pushing decoded dub audio into the matching
    /// dub target's mux session (see `cloud_session::run_cloud_session`,
    /// which decodes `Subtitle.dubbed_audio` and forwards samples here by
    /// `lang`). Cheap to clone out — each `Sender` clone is just an `Arc`
    /// bump, callers get their own owned map. If two targets share the same
    /// `dub_language`, only the last one registered ends up in this map
    /// (keyed by language, not target id) — today's use case is one target
    /// per dub language.
    pub fn dub_audio_sinks(&self) -> HashMap<String, mpsc::Sender<Vec<i16>>> {
        self.dub_audio_sinks.clone()
    }

    /// Stops the ingest pump and every target task.
    pub fn shutdown(self) {
        for h in self.handles {
            h.abort();
        }
    }
}

fn is_media(msg_type_id: u8) -> bool {
    matches!(
        msg_type_id,
        MSG_TYPE_AUDIO | MSG_TYPE_VIDEO | MSG_TYPE_AMF0_DATA
    )
}

fn csid_for(msg_type_id: u8) -> u32 {
    match msg_type_id {
        MSG_TYPE_AUDIO => CSID_AUDIO,
        MSG_TYPE_VIDEO => CSID_VIDEO,
        _ => CSID_DATA,
    }
}

fn set_state(statuses: &StatusMap, target: &RelayTarget, state: TargetState) {
    lock_statuses(statuses).insert(
        target.id.clone(),
        TargetStatus {
            id: target.id.clone(),
            label: target.label.clone(),
            state,
        },
    );
}

async fn run_target(
    target: RelayTarget,
    mut rx: broadcast::Receiver<Arc<RtmpMessage>>,
    statuses: StatusMap,
) {
    loop {
        set_state(&statuses, &target, TargetState::Connecting);
        match TcpStream::connect((target.host.as_str(), target.port)).await {
            Ok(mut sock) => match connect_publish(&mut sock, &target).await {
                Ok(stream_id) => {
                    set_state(&statuses, &target, TargetState::Live);
                    match forward_loop(&mut sock, stream_id, &mut rx).await {
                        Ok(()) => return, // broadcast channel closed: engine shutting down
                        Err(e) => {
                            set_state(&statuses, &target, TargetState::Reconnecting);
                            tracing::warn!(target = %target.label, error = %e, "relay target dropped, reconnecting");
                        }
                    }
                }
                Err(e) => set_state(&statuses, &target, TargetState::Failed(e.to_string())),
            },
            Err(e) => set_state(&statuses, &target, TargetState::Failed(e.to_string())),
        }
        tokio::time::sleep(RECONNECT_BACKOFF).await;
    }
}

/// Forwards every fanned-out message to `sock`, rebasing timestamps to start
/// near zero for this target's connection (it may have joined/reconnected
/// long after the ingest stream started).
async fn forward_loop<S>(
    sock: &mut S,
    stream_id: u32,
    rx: &mut broadcast::Receiver<Arc<RtmpMessage>>,
) -> Result<(), RtmpError>
where
    S: AsyncWrite + Unpin,
{
    let mut base_ts: Option<u32> = None;
    loop {
        let msg = match rx.recv().await {
            Ok(m) => m,
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => return Ok(()),
        };
        let base = *base_ts.get_or_insert(msg.timestamp);
        let rebased = RtmpMessage {
            msg_type_id: msg.msg_type_id,
            timestamp: msg.timestamp.wrapping_sub(base),
            msg_stream_id: stream_id,
            payload: msg.payload.clone(),
        };
        let csid = csid_for(rebased.msg_type_id);
        write_message(sock, DEFAULT_CHUNK_SIZE, csid, &rebased).await?;
    }
}

/// Errors that end one dub-target ffmpeg session — all of them are
/// reconnect-worthy (ffmpeg crashed, a pipe/socket write failed), same
/// posture as `run_target`'s `RtmpError` from `forward_loop`.
#[derive(Debug, thiserror::Error)]
enum DubSessionError {
    #[error("ffmpeg error: {0}")]
    Ffmpeg(#[from] FfmpegError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("ffmpeg exited: {0:?}")]
    FfmpegExited(std::process::ExitStatus),
}

/// Builds the ffmpeg command for one dub target: video passes through
/// untouched from stdin (FLV, see `mux::flv`), dub audio comes in as raw PCM
/// over a local TCP socket (input 1), muxed together and republished to the
/// real platform under ffmpeg's own RTMP output muxer (our `rtmp` module is
/// publish-only, it has no "serve a stream to a client" role — see
/// `mux` module docs for why this is a local TCP source, not a shared-fd pipe).
///
/// `-probesize`/`-analyzeduration` on input 0 are load-bearing, not tuning:
/// ffmpeg opens inputs in order, and by default analyzes each one (reading
/// until it's confident about stream parameters, or EOF) *before* opening the
/// next — stdin here is a live, never-EOF feed, so without a bound ffmpeg
/// never gets around to opening input 1 at all (confirmed by hand: it hangs
/// until stdin is closed). Small explicit limits let it settle for "good
/// enough" quickly instead of waiting for a live stream to end.
fn build_dub_ffmpeg_args(pcm_port: u16, output_url: &str) -> Vec<String> {
    vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "warning".to_string(),
        "-probesize".to_string(),
        "32k".to_string(),
        "-analyzeduration".to_string(),
        "500000".to_string(),
        "-f".to_string(),
        "flv".to_string(),
        "-i".to_string(),
        "pipe:0".to_string(),
        "-f".to_string(),
        "s16le".to_string(),
        "-ar".to_string(),
        DUB_PCM_SAMPLE_RATE.to_string(),
        "-ac".to_string(),
        "1".to_string(),
        "-i".to_string(),
        format!("tcp://127.0.0.1:{pcm_port}"),
        "-map".to_string(),
        "0:v".to_string(),
        "-map".to_string(),
        "1:a".to_string(),
        "-c:v".to_string(),
        "copy".to_string(),
        "-c:a".to_string(),
        "aac".to_string(),
        "-f".to_string(),
        "flv".to_string(),
        output_url.to_string(),
    ]
}

/// Dub-target counterpart of `run_target`: instead of relaying the original
/// audio+video verbatim to the real platform, it replaces the audio with
/// cloud-synthesized dub audio (device mux, task 2.3) via a local ffmpeg
/// process, and republishes that. Same reconnect/backoff shape as
/// `run_target` — an ffmpeg crash or write failure just restarts the whole
/// session (fresh ffmpeg, fresh local port); the audio scheduler survives
/// across restarts so a dub track that arrived mid-reconnect isn't lost.
async fn run_dub_target(
    target: RelayTarget,
    mut rx: broadcast::Receiver<Arc<RtmpMessage>>,
    mut dub_audio_rx: mpsc::Receiver<Vec<i16>>,
    statuses: StatusMap,
) {
    let mut scheduler = DubAudioScheduler::new();
    loop {
        set_state(&statuses, &target, TargetState::Connecting);
        match run_dub_session(&target, &mut rx, &mut dub_audio_rx, &mut scheduler, &statuses).await
        {
            Ok(()) => return, // broadcast channel closed: engine shutting down
            Err(e) => {
                set_state(&statuses, &target, TargetState::Reconnecting);
                tracing::warn!(target = %target.label, error = %e, "dub target dropped, reconnecting");
            }
        }
        tokio::time::sleep(RECONNECT_BACKOFF).await;
    }
}

/// One ffmpeg run for a dub target: spawns ffmpeg, accepts its PCM input
/// connection, then drives three things concurrently until one of them
/// signals the session is over — original-stream video/metadata into
/// ffmpeg's stdin (FLV), the dub audio scheduler's steady PCM tick into
/// ffmpeg's TCP input, and ffmpeg's own exit (crash detection).
async fn run_dub_session(
    target: &RelayTarget,
    rx: &mut broadcast::Receiver<Arc<RtmpMessage>>,
    dub_audio_rx: &mut mpsc::Receiver<Vec<i16>>,
    scheduler: &mut DubAudioScheduler,
    statuses: &StatusMap,
) -> Result<(), DubSessionError> {
    let pcm_listener = TcpListener::bind("127.0.0.1:0").await?;
    let pcm_port = pcm_listener.local_addr()?.port();
    let output_url = format!("{}/{}", target.tc_url, target.stream_key);
    let mut ffmpeg = FfmpegProcess::spawn(&build_dub_ffmpeg_args(pcm_port, &output_url)).await?;

    let mut flv = FlvWriter::new();
    let mut tick = tokio::time::interval(DUB_PCM_TICK);
    let mut dub_channel_closed = false;
    // ffmpeg connects to us as the client for its second (PCM) input — but
    // NOT before it starts reading/probing stdin (input 0), so this accept
    // must not block anything else: ffmpeg's probing needs FLV bytes from
    // the `rx.recv()` branch below to make progress far enough to even
    // attempt opening input 1. Awaiting this accept up front (before the
    // select loop) deadlocks against that. `TcpListener::accept` is
    // cancel-safe, so re-polling it every iteration is fine.
    let mut pcm_sock: Option<TcpStream> = None;

    loop {
        tokio::select! {
            accepted = pcm_listener.accept(), if pcm_sock.is_none() => {
                let (sock, _) = accepted?;
                pcm_sock = Some(sock);
                set_state(statuses, target, TargetState::Live);
            }
            msg = rx.recv() => {
                match msg {
                    Ok(m) => {
                        let mut buf = Vec::new();
                        flv.write_tag(&mut buf, &m);
                        if !buf.is_empty() {
                            ffmpeg.stdin().write_all(&buf).await?;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => return Ok(()),
                }
            }
            dub = dub_audio_rx.recv(), if !dub_channel_closed => {
                match dub {
                    Some(samples) => scheduler.push_dub_audio(samples),
                    // Sender dropped (engine shutting down) — no more dub
                    // audio will ever arrive; keep relaying on silence
                    // rather than tearing the target down over it, and stop
                    // selecting on this branch (else it would fire on every
                    // poll from here on).
                    None => dub_channel_closed = true,
                }
            }
            _ = tick.tick() => {
                let chunk = scheduler.next_chunk(DUB_PCM_SAMPLES_PER_TICK);
                // Not connected yet — drop this tick's samples (nothing to
                // write to); once ffmpeg connects it starts fresh from
                // whatever's queued in the scheduler at that point.
                if let Some(sock) = pcm_sock.as_mut() {
                    sock.write_all(&pcm_to_le_bytes(&chunk)).await?;
                }
            }
            status = ffmpeg.wait() => {
                return Err(DubSessionError::FfmpegExited(status?));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::rtmp::amf0::Amf0Value;
    use crate::core::rtmp::chunk::ChunkReader;
    use crate::core::rtmp::handshake;
    use crate::core::rtmp::message::{
        connect_command, connect_result, create_stream_command, create_stream_result,
        parse_command, publish_command, publish_start_status, CSID_COMMAND, MSG_TYPE_AMF0_COMMAND,
    };
    use tokio::net::{TcpListener, TcpStream};

    async fn next_command(sock: &mut TcpStream, reader: &mut ChunkReader) -> RtmpMessage {
        loop {
            let m = reader.read_message(sock).await.expect("read command");
            if m.msg_type_id == MSG_TYPE_AMF0_COMMAND {
                return m;
            }
        }
    }

    /// Accepts one connection and completes the destination-side RTMP
    /// handshake (connect/createStream/publish, asserting the publish stream
    /// key matches `expected_key`) — shared by `fake_target` (exact-count
    /// collection) and the dub-mux test's timeout-based collector, since both
    /// need the identical negotiation dance before they start collecting
    /// media tags differently.
    async fn accept_and_negotiate_publish(
        listener: TcpListener,
        expected_key: &str,
    ) -> (TcpStream, ChunkReader, u32) {
        let (mut sock, _) = listener.accept().await.expect("accept");
        handshake::server_handshake(&mut sock)
            .await
            .expect("handshake");
        let mut reader = ChunkReader::new();
        let stream_id = 9u32;

        // Loop rather than expecting an exact 3-command sequence: our own
        // `connect_publish` (session.rs) only ever sends connect/createStream
        // /publish, but a real RTMP client — ffmpeg's own output muxer, used
        // by the dub-mux test below — also sends `releaseStream`/`FCPublish`
        // right after `connect`, without waiting for (or caring about) a
        // reply to either. Anything that isn't one of the three we care
        // about is silently ignored; we stop as soon as `publish` arrives.
        loop {
            let msg = next_command(&mut sock, &mut reader).await;
            let cmd = parse_command(&msg.payload).expect("parse command");
            match cmd.name.as_str() {
                "connect" => {
                    write_message(
                        &mut sock,
                        DEFAULT_CHUNK_SIZE,
                        CSID_COMMAND,
                        &RtmpMessage {
                            msg_type_id: MSG_TYPE_AMF0_COMMAND,
                            timestamp: 0,
                            msg_stream_id: 0,
                            payload: connect_result(cmd.transaction_id),
                        },
                    )
                    .await
                    .expect("write connect result");
                }
                "createStream" => {
                    write_message(
                        &mut sock,
                        DEFAULT_CHUNK_SIZE,
                        CSID_COMMAND,
                        &RtmpMessage {
                            msg_type_id: MSG_TYPE_AMF0_COMMAND,
                            timestamp: 0,
                            msg_stream_id: 0,
                            payload: create_stream_result(cmd.transaction_id, stream_id as f64),
                        },
                    )
                    .await
                    .expect("write createStream result");
                }
                "publish" => {
                    match cmd.args.get(1) {
                        Some(Amf0Value::String(s)) => assert_eq!(s, expected_key),
                        other => panic!("expected stream key, got {other:?}"),
                    }
                    write_message(
                        &mut sock,
                        DEFAULT_CHUNK_SIZE,
                        CSID_COMMAND,
                        &RtmpMessage {
                            msg_type_id: MSG_TYPE_AMF0_COMMAND,
                            timestamp: 0,
                            msg_stream_id: stream_id,
                            payload: publish_start_status(),
                        },
                    )
                    .await
                    .expect("write publish status");
                    return (sock, reader, stream_id);
                }
                _ => continue, // releaseStream, FCPublish, etc. — no reply needed
            }
        }
    }

    /// Fake destination platform: negotiates the publish handshake, then
    /// collects exactly `want` forwarded media messages.
    async fn fake_target(
        listener: TcpListener,
        expected_key: String,
        want: usize,
    ) -> Vec<RtmpMessage> {
        let (mut sock, mut reader, _stream_id) =
            accept_and_negotiate_publish(listener, &expected_key).await;
        let mut received = Vec::new();
        while received.len() < want {
            let msg = reader.read_message(&mut sock).await.expect("read media");
            if matches!(msg.msg_type_id, MSG_TYPE_AUDIO | MSG_TYPE_VIDEO) {
                received.push(msg);
            }
        }
        received
    }

    /// Same negotiation as `fake_target`, but collects media messages until
    /// `timeout` elapses instead of an exact count — real ffmpeg encode/mux
    /// timing isn't something to hard-code a message count against.
    async fn collect_media_with_timeout(
        listener: TcpListener,
        expected_key: String,
        timeout: Duration,
    ) -> Vec<RtmpMessage> {
        let (mut sock, mut reader, _stream_id) =
            accept_and_negotiate_publish(listener, &expected_key).await;
        let mut received = Vec::new();
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            match tokio::time::timeout(remaining, reader.read_message(&mut sock)).await {
                Ok(Ok(msg)) if matches!(msg.msg_type_id, MSG_TYPE_AUDIO | MSG_TYPE_VIDEO) => {
                    received.push(msg)
                }
                Ok(Ok(_)) => continue,
                Ok(Err(_)) | Err(_) => break,
            }
        }
        received
    }

    /// Fake OBS: connects to the SDK's local ingest listener, negotiates
    /// connect/createStream/publish with `key`, then sends `frames` and
    /// holds the connection open for `hold` (so targets — real ffmpeg
    /// especially — have time to encode/mux/deliver before ingest tears down).
    async fn fake_obs(addr: std::net::SocketAddr, key: &str, frames: Vec<RtmpMessage>, hold: Duration) {
        let mut sock = TcpStream::connect(addr).await.expect("connect ingest");
        handshake::client_handshake(&mut sock)
            .await
            .expect("client handshake");
        let mut reader = ChunkReader::new();

        write_message(
            &mut sock,
            DEFAULT_CHUNK_SIZE,
            CSID_COMMAND,
            &RtmpMessage {
                msg_type_id: MSG_TYPE_AMF0_COMMAND,
                timestamp: 0,
                msg_stream_id: 0,
                payload: connect_command("live", "rtmp://127.0.0.1/live"),
            },
        )
        .await
        .expect("send connect");
        next_command(&mut sock, &mut reader).await;

        write_message(
            &mut sock,
            DEFAULT_CHUNK_SIZE,
            CSID_COMMAND,
            &RtmpMessage {
                msg_type_id: MSG_TYPE_AMF0_COMMAND,
                timestamp: 0,
                msg_stream_id: 0,
                payload: create_stream_command(2.0),
            },
        )
        .await
        .expect("send createStream");
        let msg = next_command(&mut sock, &mut reader).await;
        let cmd = parse_command(&msg.payload).expect("parse createStream result");
        let stream_id = match cmd.args.get(1) {
            Some(Amf0Value::Number(n)) => *n as u32,
            _ => 1,
        };

        write_message(
            &mut sock,
            DEFAULT_CHUNK_SIZE,
            CSID_COMMAND,
            &RtmpMessage {
                msg_type_id: MSG_TYPE_AMF0_COMMAND,
                timestamp: 0,
                msg_stream_id: stream_id,
                payload: publish_command(key),
            },
        )
        .await
        .expect("send publish");
        next_command(&mut sock, &mut reader).await;

        for mut frame in frames {
            frame.msg_stream_id = stream_id;
            let csid = csid_for(frame.msg_type_id);
            write_message(&mut sock, DEFAULT_CHUNK_SIZE, csid, &frame)
                .await
                .expect("send media");
        }
        // Keep the socket open so targets finish draining the broadcast
        // channel before this connection (and the ingest pump reading it)
        // goes away.
        tokio::time::sleep(hold).await;
    }

    #[tokio::test]
    async fn fans_out_to_two_independent_targets_with_own_keys() {
        let target1_listener = TcpListener::bind("127.0.0.1:0").await.expect("bind t1");
        let target1_addr = target1_listener.local_addr().expect("addr t1");
        let target2_listener = TcpListener::bind("127.0.0.1:0").await.expect("bind t2");
        let target2_addr = target2_listener.local_addr().expect("addr t2");

        let targets = vec![
            RelayTarget::parse(
                "t1",
                "Target1",
                &format!("rtmp://127.0.0.1:{}/app", target1_addr.port()),
                "key-one",
            )
            .expect("target1"),
            RelayTarget::parse(
                "t2",
                "Target2",
                &format!("rtmp://127.0.0.1:{}/app", target2_addr.port()),
                "key-two",
            )
            .expect("target2"),
        ];

        let ingest_listener = TcpListener::bind("127.0.0.1:0").await.expect("bind ingest");
        let ingest_addr = ingest_listener.local_addr().expect("addr ingest");

        let frames = vec![
            RtmpMessage {
                msg_type_id: MSG_TYPE_VIDEO,
                timestamp: 1000,
                msg_stream_id: 0,
                payload: vec![1, 2, 3, 4],
            },
            RtmpMessage {
                msg_type_id: MSG_TYPE_AUDIO,
                timestamp: 1005,
                msg_stream_id: 0,
                payload: vec![5, 6],
            },
            RtmpMessage {
                msg_type_id: MSG_TYPE_VIDEO,
                timestamp: 1040,
                msg_stream_id: 0,
                payload: vec![7, 8, 9],
            },
        ];

        let target1_task = tokio::spawn(fake_target(target1_listener, "key-one".to_string(), 3));
        let target2_task = tokio::spawn(fake_target(target2_listener, "key-two".to_string(), 3));
        let obs_task = tokio::spawn(fake_obs(
            ingest_addr,
            "obs-local-key",
            frames,
            Duration::from_millis(200),
        ));

        let (ingest_sock, _) = ingest_listener.accept().await.expect("accept ingest");
        let engine = RelayEngine::start(ingest_sock, targets)
            .await
            .expect("start engine");

        obs_task.await.expect("obs task");
        let received1 = target1_task.await.expect("target1 task");
        let received2 = target2_task.await.expect("target2 task");

        for received in [&received1, &received2] {
            assert_eq!(received.len(), 3);
            assert_eq!(received[0].payload, vec![1, 2, 3, 4]);
            assert_eq!(received[1].payload, vec![5, 6]);
            assert_eq!(received[2].payload, vec![7, 8, 9]);
            // Rebased per-target: first forwarded message starts at t=0.
            assert_eq!(received[0].timestamp, 0);
            assert_eq!(received[2].timestamp, 40);
        }

        let statuses = engine.statuses();
        assert_eq!(statuses.len(), 2);
        assert!(statuses.iter().all(|s| s.state == TargetState::Live));

        engine.shutdown();
    }

    /// Generates a tiny valid H.264-in-FLV clip with real ffmpeg (synthetic
    /// `testsrc`, no capture hardware needed) — used as an ingest fixture so
    /// the dub-mux test exercises `-c:v copy` against real, ffmpeg-parseable
    /// video instead of arbitrary bytes.
    async fn generate_test_h264_flv() -> Vec<u8> {
        let output = tokio::process::Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=1:size=64x64:rate=5",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-preset",
                "ultrafast",
                "-f",
                "flv",
                "pipe:1",
            ])
            .output()
            .await
            .expect("spawn ffmpeg to generate test fixture");
        assert!(
            output.status.success(),
            "ffmpeg fixture generation failed: {:?}",
            output.status
        );
        output.stdout
    }

    /// Reverses `mux::flv::FlvWriter`'s framing: parses an FLV byte buffer
    /// (as produced by real ffmpeg above) back into the `RtmpMessage`s it
    /// contains, so they can be fed through `fake_obs` like any other test
    /// fixture.
    fn parse_flv_into_messages(buf: &[u8]) -> Vec<RtmpMessage> {
        let mut messages = Vec::new();
        let mut pos = 13usize; // 9-byte FLV header + 4-byte PreviousTagSize0
        while pos + 11 <= buf.len() {
            let tag_type = buf[pos];
            let data_size =
                u32::from_be_bytes([0, buf[pos + 1], buf[pos + 2], buf[pos + 3]]) as usize;
            let ts_lower = u32::from_be_bytes([0, buf[pos + 4], buf[pos + 5], buf[pos + 6]]);
            let ts_upper = buf[pos + 7] as u32;
            let timestamp = (ts_upper << 24) | ts_lower;
            let data_start = pos + 11;
            let data_end = data_start + data_size;
            if data_end > buf.len() {
                break;
            }
            messages.push(RtmpMessage {
                msg_type_id: tag_type,
                timestamp,
                msg_stream_id: 0,
                payload: buf[data_start..data_end].to_vec(),
            });
            pos = data_end + 4; // skip trailing PreviousTagSize
        }
        messages
    }

    /// End-to-end proof that device mux works against a REAL ffmpeg binary:
    /// a dub target replaces the original video's audio with pushed dub
    /// audio and republishes to a real (fake) RTMP destination. Skips
    /// (doesn't fail) if ffmpeg isn't installed on this machine.
    #[tokio::test]
    async fn dub_target_mux_republishes_video_with_pushed_dub_audio() {
        if !crate::core::mux::ffmpeg_process::ffmpeg_available() {
            eprintln!("skipping: ffmpeg not installed on this machine");
            return;
        }

        let flv_bytes = generate_test_h264_flv().await;
        let frames = parse_flv_into_messages(&flv_bytes);
        assert!(!frames.is_empty(), "fixture must contain at least one FLV tag");

        let dest_listener = TcpListener::bind("127.0.0.1:0").await.expect("bind dest");
        let dest_addr = dest_listener.local_addr().expect("addr dest");

        let dub_target = RelayTarget::parse(
            "dub-en",
            "DubEN",
            &format!("rtmp://127.0.0.1:{}/app", dest_addr.port()),
            "dub-stream-key",
        )
        .expect("dub target")
        .with_dub_language(Some("en".to_string()));

        let ingest_listener = TcpListener::bind("127.0.0.1:0").await.expect("bind ingest");
        let ingest_addr = ingest_listener.local_addr().expect("addr ingest");

        let dest_task = tokio::spawn(collect_media_with_timeout(
            dest_listener,
            "dub-stream-key".to_string(),
            Duration::from_secs(8),
        ));
        let obs_task = tokio::spawn(fake_obs(
            ingest_addr,
            "obs-key",
            frames,
            Duration::from_secs(3),
        ));

        let (ingest_sock, _) = ingest_listener.accept().await.expect("accept ingest");
        let engine = RelayEngine::start(ingest_sock, vec![dub_target])
            .await
            .expect("start engine");

        // Give ffmpeg a moment to spawn and connect its PCM input before
        // pushing dub audio — otherwise the push lands before anyone's
        // listening on the other end of the sink (harmless — it would just
        // queue in the scheduler — but waiting keeps the timing realistic).
        tokio::time::sleep(Duration::from_millis(300)).await;
        let sinks = engine.dub_audio_sinks();
        let dub_tx = sinks.get("en").expect("dub sink registered for 'en'");
        dub_tx
            .send(vec![1000i16; 24_000]) // ~1s of non-silent PCM at 24kHz
            .await
            .expect("push dub audio");

        obs_task.await.expect("obs task");
        let received = dest_task.await.expect("dest task");

        let has_video = received.iter().any(|m| m.msg_type_id == MSG_TYPE_VIDEO);
        let has_audio = received.iter().any(|m| m.msg_type_id == MSG_TYPE_AUDIO);
        assert!(has_video, "expected at least one video tag at the real destination");
        assert!(
            has_audio,
            "expected at least one AAC audio tag at the real destination (dub audio mux)"
        );

        engine.shutdown();
    }
}
