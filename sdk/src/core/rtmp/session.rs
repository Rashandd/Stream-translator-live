//! Session negotiation on top of handshake + chunk stream + commands.
//!
//! Two roles:
//! - [`accept_publisher`]: server role, used for the local ingest connection
//!   OBS publishes into.
//! - [`connect_publish`]: client role, used for each outbound connection to
//!   a destination platform (its own app/tcUrl/stream key).

use tokio::io::{AsyncRead, AsyncWrite};

use super::amf0::{Amf0Error, Amf0Value};
use super::chunk::{write_message, ChunkError, ChunkReader, RtmpMessage, DEFAULT_CHUNK_SIZE};
use super::handshake::{self, HandshakeError};
use super::message::{
    connect_command, connect_result, create_stream_command, create_stream_result, parse_command,
    publish_command, publish_start_status, reply_signals_error, CommandError, CSID_COMMAND,
    MSG_TYPE_AMF0_COMMAND,
};
use super::target::RelayTarget;

#[derive(Debug, thiserror::Error)]
pub enum RtmpError {
    #[error("handshake failed: {0}")]
    Handshake(#[from] HandshakeError),
    #[error("chunk stream error: {0}")]
    Chunk(#[from] ChunkError),
    #[error("command decode error: {0}")]
    Command(#[from] CommandError),
    #[error("expected {expected} command, got {got}")]
    UnexpectedCommand { expected: &'static str, got: String },
    #[error("peer rejected {step}: {detail}")]
    Rejected { step: &'static str, detail: String },
}

impl From<Amf0Error> for RtmpError {
    fn from(e: Amf0Error) -> Self {
        RtmpError::Command(CommandError::from(e))
    }
}

/// What the local publisher (OBS) told us when it started publishing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublisherInfo {
    pub stream_key: String,
}

async fn send_command<S>(io: &mut S, msg_stream_id: u32, payload: Vec<u8>) -> Result<(), RtmpError>
where
    S: AsyncWrite + Unpin,
{
    write_message(
        io,
        DEFAULT_CHUNK_SIZE,
        CSID_COMMAND,
        &RtmpMessage {
            msg_type_id: MSG_TYPE_AMF0_COMMAND,
            timestamp: 0,
            msg_stream_id,
            payload,
        },
    )
    .await?;
    Ok(())
}

/// Reads chunk-stream messages until an AMF0 command message arrives
/// (protocol-control replies the peer may interleave are skipped).
async fn next_command<S>(io: &mut S, reader: &mut ChunkReader) -> Result<RtmpMessage, RtmpError>
where
    S: AsyncRead + Unpin,
{
    loop {
        let msg = reader.read_message(io).await?;
        if msg.msg_type_id == MSG_TYPE_AMF0_COMMAND {
            return Ok(msg);
        }
    }
}

/// Server role: completes the handshake with a connecting OBS publisher,
/// negotiates connect/createStream/publish, and returns once OBS is clear to
/// start sending audio/video. The returned [`ChunkReader`] continues to
/// demux subsequent media messages from the same connection.
pub async fn accept_publisher<S>(io: &mut S) -> Result<(ChunkReader, PublisherInfo), RtmpError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    handshake::server_handshake(io).await?;
    let mut reader = ChunkReader::new();

    let msg = next_command(io, &mut reader).await?;
    let cmd = parse_command(&msg.payload)?;
    if cmd.name != "connect" {
        return Err(RtmpError::UnexpectedCommand {
            expected: "connect",
            got: cmd.name,
        });
    }
    send_command(io, 0, connect_result(cmd.transaction_id)).await?;

    let msg = next_command(io, &mut reader).await?;
    let cmd = parse_command(&msg.payload)?;
    if cmd.name != "createStream" {
        return Err(RtmpError::UnexpectedCommand {
            expected: "createStream",
            got: cmd.name,
        });
    }
    const ASSIGNED_STREAM_ID: u32 = 1;
    send_command(
        io,
        0,
        create_stream_result(cmd.transaction_id, ASSIGNED_STREAM_ID as f64),
    )
    .await?;

    let msg = next_command(io, &mut reader).await?;
    let cmd = parse_command(&msg.payload)?;
    if cmd.name != "publish" {
        return Err(RtmpError::UnexpectedCommand {
            expected: "publish",
            got: cmd.name,
        });
    }
    // publish command args: [Null (command object), String(stream_key), String("live")].
    let stream_key = match cmd.args.get(1) {
        Some(Amf0Value::String(s)) => s.clone(),
        _ => String::new(),
    };
    send_command(io, ASSIGNED_STREAM_ID, publish_start_status()).await?;

    Ok((reader, PublisherInfo { stream_key }))
}

/// Client role: connects (handshake + connect/createStream/publish) to a
/// destination platform using `target`'s own app/tcUrl/stream key. Returns
/// the message-stream id the target assigned, to use for forwarded media.
pub async fn connect_publish<S>(io: &mut S, target: &RelayTarget) -> Result<u32, RtmpError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    handshake::client_handshake(io).await?;
    let mut reader = ChunkReader::new();

    send_command(io, 0, connect_command(&target.app, &target.tc_url)).await?;
    let msg = next_command(io, &mut reader).await?;
    let cmd = parse_command(&msg.payload)?;
    if cmd.name != "_result" || reply_signals_error(&cmd) {
        return Err(RtmpError::Rejected {
            step: "connect",
            detail: cmd.name,
        });
    }

    send_command(io, 0, create_stream_command(2.0)).await?;
    let msg = next_command(io, &mut reader).await?;
    let cmd = parse_command(&msg.payload)?;
    if cmd.name != "_result" || reply_signals_error(&cmd) {
        return Err(RtmpError::Rejected {
            step: "createStream",
            detail: cmd.name,
        });
    }
    let stream_id = match cmd.args.get(1) {
        Some(Amf0Value::Number(n)) => *n as u32,
        _ => 1,
    };

    send_command(io, stream_id, publish_command(&target.stream_key)).await?;
    let msg = next_command(io, &mut reader).await?;
    let cmd = parse_command(&msg.payload)?;
    if cmd.name != "onStatus" || reply_signals_error(&cmd) {
        return Err(RtmpError::Rejected {
            step: "publish",
            detail: cmd.name,
        });
    }

    Ok(stream_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::rtmp::message::{
        connect_result as srv_connect_result, create_stream_result as srv_create_stream_result,
        publish_start_status as srv_publish_start_status, CSID_COMMAND as SRV_CSID_COMMAND,
        MSG_TYPE_AMF0_COMMAND as SRV_MSG_TYPE_AMF0_COMMAND,
    };
    use tokio::io::DuplexStream;

    async fn fake_target_server(mut io: DuplexStream, expected_key: String) -> u32 {
        handshake::server_handshake(&mut io)
            .await
            .expect("handshake");
        let mut reader = ChunkReader::new();

        let msg = next_command(&mut io, &mut reader).await.expect("connect");
        let cmd = parse_command(&msg.payload).expect("parse connect");
        assert_eq!(cmd.name, "connect");
        write_message(
            &mut io,
            DEFAULT_CHUNK_SIZE,
            SRV_CSID_COMMAND,
            &RtmpMessage {
                msg_type_id: SRV_MSG_TYPE_AMF0_COMMAND,
                timestamp: 0,
                msg_stream_id: 0,
                payload: srv_connect_result(cmd.transaction_id),
            },
        )
        .await
        .expect("write connect result");

        let msg = next_command(&mut io, &mut reader)
            .await
            .expect("createStream");
        let cmd = parse_command(&msg.payload).expect("parse createStream");
        assert_eq!(cmd.name, "createStream");
        let assigned_stream_id = 7u32;
        write_message(
            &mut io,
            DEFAULT_CHUNK_SIZE,
            SRV_CSID_COMMAND,
            &RtmpMessage {
                msg_type_id: SRV_MSG_TYPE_AMF0_COMMAND,
                timestamp: 0,
                msg_stream_id: 0,
                payload: srv_create_stream_result(cmd.transaction_id, assigned_stream_id as f64),
            },
        )
        .await
        .expect("write createStream result");

        let msg = next_command(&mut io, &mut reader).await.expect("publish");
        let cmd = parse_command(&msg.payload).expect("parse publish");
        assert_eq!(cmd.name, "publish");
        match cmd.args.get(1) {
            Some(Amf0Value::String(s)) => assert_eq!(*s, expected_key),
            other => panic!("expected stream key string, got {other:?}"),
        }
        write_message(
            &mut io,
            DEFAULT_CHUNK_SIZE,
            SRV_CSID_COMMAND,
            &RtmpMessage {
                msg_type_id: SRV_MSG_TYPE_AMF0_COMMAND,
                timestamp: 0,
                msg_stream_id: assigned_stream_id,
                payload: srv_publish_start_status(),
            },
        )
        .await
        .expect("write publish status");

        assigned_stream_id
    }

    #[tokio::test]
    async fn connect_publish_negotiates_with_target_specific_key() {
        let (client_io, server_io) = tokio::io::duplex(8192);
        let target =
            RelayTarget::parse("yt", "YouTube", "rtmp://127.0.0.1/live2", "target-only-key")
                .expect("parse target");

        let server = tokio::spawn(fake_target_server(server_io, "target-only-key".to_string()));
        let mut client_io = client_io;
        let stream_id = connect_publish(&mut client_io, &target)
            .await
            .expect("connect_publish");
        let assigned = server.await.expect("server task");
        assert_eq!(stream_id, assigned);
    }

    #[tokio::test]
    async fn accept_publisher_returns_stream_key_from_publish_command() {
        let (client_io, server_io) = tokio::io::duplex(8192);

        let client = tokio::spawn(async move {
            let mut client_io = client_io;
            handshake::client_handshake(&mut client_io)
                .await
                .expect("client handshake");
            let mut reader = ChunkReader::new();

            send_command(
                &mut client_io,
                0,
                connect_command("live", "rtmp://127.0.0.1/live"),
            )
            .await
            .expect("send connect");
            let msg = next_command(&mut client_io, &mut reader)
                .await
                .expect("connect result");
            assert_eq!(parse_command(&msg.payload).expect("parse").name, "_result");

            send_command(&mut client_io, 0, create_stream_command(2.0))
                .await
                .expect("send createStream");
            let msg = next_command(&mut client_io, &mut reader)
                .await
                .expect("createStream result");
            let cmd = parse_command(&msg.payload).expect("parse");
            assert_eq!(cmd.name, "_result");
            let stream_id = match cmd.args.get(1) {
                Some(Amf0Value::Number(n)) => *n as u32,
                _ => panic!("expected stream id"),
            };

            send_command(&mut client_io, stream_id, publish_command("obs-local-key"))
                .await
                .expect("send publish");
            let msg = next_command(&mut client_io, &mut reader)
                .await
                .expect("onStatus");
            assert_eq!(parse_command(&msg.payload).expect("parse").name, "onStatus");
        });

        let mut server_io = server_io;
        let (_reader, info) = accept_publisher(&mut server_io)
            .await
            .expect("accept_publisher");
        client.await.expect("client task");
        assert_eq!(info.stream_key, "obs-local-key");
    }

    #[tokio::test]
    async fn connect_publish_rejects_error_reply() {
        let (client_io, mut server_io) = tokio::io::duplex(8192);
        let target = RelayTarget::parse("t", "T", "rtmp://127.0.0.1/live", "k").expect("parse");

        let server = tokio::spawn(async move {
            handshake::server_handshake(&mut server_io)
                .await
                .expect("handshake");
            let mut reader = ChunkReader::new();
            let msg = next_command(&mut server_io, &mut reader)
                .await
                .expect("connect");
            let cmd = parse_command(&msg.payload).expect("parse");
            assert_eq!(cmd.name, "connect");
            // Reply with an error-level _result instead of success.
            let payload = {
                let mut out = Vec::new();
                Amf0Value::String("_error".into()).encode(&mut out);
                Amf0Value::Number(cmd.transaction_id).encode(&mut out);
                out
            };
            write_message(
                &mut server_io,
                DEFAULT_CHUNK_SIZE,
                SRV_CSID_COMMAND,
                &RtmpMessage {
                    msg_type_id: SRV_MSG_TYPE_AMF0_COMMAND,
                    timestamp: 0,
                    msg_stream_id: 0,
                    payload,
                },
            )
            .await
            .expect("write error reply");
        });

        let mut client_io = client_io;
        let err = connect_publish(&mut client_io, &target).await.unwrap_err();
        assert!(matches!(
            err,
            RtmpError::Rejected {
                step: "connect",
                ..
            }
        ));
        server.await.expect("server task");
    }
}
