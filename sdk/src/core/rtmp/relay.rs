//! Fan-out orchestration: one ingest connection (OBS publishing locally) is
//! relayed to N destination platforms concurrently, each on its own outbound
//! connection with its own stream key. One target failing/reconnecting never
//! blocks the others or the ingest side.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use super::chunk::{write_message, RtmpMessage, DEFAULT_CHUNK_SIZE};
use super::message::{
    CSID_AUDIO, CSID_DATA, CSID_VIDEO, MSG_TYPE_AMF0_DATA, MSG_TYPE_AUDIO, MSG_TYPE_VIDEO,
};
use super::session::{accept_publisher, connect_publish, RtmpError};
use super::target::RelayTarget;

/// Fan-out channel backlog. Small on purpose: a target that falls behind
/// should drop stale media (see `RecvError::Lagged`) rather than stall the
/// whole relay for everyone else.
const CHANNEL_CAPACITY: usize = 256;
const RECONNECT_BACKOFF: Duration = Duration::from_secs(2);

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
}

impl RelayEngine {
    pub async fn start<S>(mut ingest: S, targets: Vec<RelayTarget>) -> Result<Self, RtmpError>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (mut reader, _publisher) = accept_publisher(&mut ingest).await?;
        let (tx, _rx) = broadcast::channel::<Arc<RtmpMessage>>(CHANNEL_CAPACITY);
        let statuses: StatusMap = Arc::new(Mutex::new(HashMap::new()));

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
            handles.push(tokio::spawn(run_target(target, rx, statuses)));
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

        Ok(RelayEngine { statuses, handles })
    }

    /// Snapshot of every target's current state, sorted by id.
    pub fn statuses(&self) -> Vec<TargetStatus> {
        let mut v: Vec<_> = lock_statuses(&self.statuses).values().cloned().collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
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

    /// Fake destination platform: accepts one connection, negotiates
    /// connect/createStream/publish (asserting the publish stream key
    /// matches `expected_key`), then collects `want` forwarded media
    /// messages.
    async fn fake_target(
        listener: TcpListener,
        expected_key: String,
        want: usize,
    ) -> Vec<RtmpMessage> {
        let (mut sock, _) = listener.accept().await.expect("accept");
        handshake::server_handshake(&mut sock)
            .await
            .expect("handshake");
        let mut reader = ChunkReader::new();

        let msg = next_command(&mut sock, &mut reader).await;
        let cmd = parse_command(&msg.payload).expect("parse connect");
        assert_eq!(cmd.name, "connect");
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

        let msg = next_command(&mut sock, &mut reader).await;
        let cmd = parse_command(&msg.payload).expect("parse createStream");
        assert_eq!(cmd.name, "createStream");
        let stream_id = 9u32;
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

        let msg = next_command(&mut sock, &mut reader).await;
        let cmd = parse_command(&msg.payload).expect("parse publish");
        assert_eq!(cmd.name, "publish");
        match cmd.args.get(1) {
            Some(Amf0Value::String(s)) => assert_eq!(*s, expected_key),
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

        let mut received = Vec::new();
        while received.len() < want {
            let msg = reader.read_message(&mut sock).await.expect("read media");
            if matches!(msg.msg_type_id, MSG_TYPE_AUDIO | MSG_TYPE_VIDEO) {
                received.push(msg);
            }
        }
        received
    }

    /// Fake OBS: connects to the SDK's local ingest listener, negotiates
    /// connect/createStream/publish with `key`, then sends `frames`.
    async fn fake_obs(addr: std::net::SocketAddr, key: &str, frames: Vec<RtmpMessage>) {
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
        // Keep the socket open briefly so targets finish draining the
        // broadcast channel before this connection (and the ingest pump
        // reading it) goes away.
        tokio::time::sleep(Duration::from_millis(200)).await;
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
        let obs_task = tokio::spawn(fake_obs(ingest_addr, "obs-local-key", frames));

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
}
