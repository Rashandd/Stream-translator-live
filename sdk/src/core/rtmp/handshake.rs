//! RTMP handshake (simple/uncompressed variant — no HMAC digest exchange).
//!
//! We control both the ingest side (accepting OBS) and, when the SDK plays
//! client, the target side (connecting to a platform's RTMP endpoint). The
//! simple handshake is universally accepted by RTMP servers/clients that
//! don't hard-require Adobe's complex digest handshake, which covers OBS and
//! every major platform ingest we relay to. The "random" payload bytes are
//! never verified by either side in this mode, so filling them with zeros is
//! valid and avoids pulling in a random-number crate for protocol filler.

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const RTMP_VERSION: u8 = 3;
const HANDSHAKE_SIZE: usize = 1536;

#[derive(Debug, thiserror::Error)]
pub enum HandshakeError {
    #[error("io error during handshake: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported rtmp version: {0}")]
    UnsupportedVersion(u8),
}

/// Server role: accepts C0/C1, replies S0/S1/S2, reads (and discards) C2.
pub async fn server_handshake<S>(io: &mut S) -> Result<(), HandshakeError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut c0 = [0u8; 1];
    io.read_exact(&mut c0).await?;
    if c0[0] != RTMP_VERSION {
        return Err(HandshakeError::UnsupportedVersion(c0[0]));
    }

    let mut c1 = [0u8; HANDSHAKE_SIZE];
    io.read_exact(&mut c1).await?;

    let s1 = [0u8; HANDSHAKE_SIZE];
    io.write_all(&[RTMP_VERSION]).await?;
    io.write_all(&s1).await?;
    io.write_all(&c1).await?; // S2 echoes C1
    io.flush().await?;

    let mut c2 = [0u8; HANDSHAKE_SIZE];
    io.read_exact(&mut c2).await?;
    Ok(())
}

/// Client role: sends C0/C1, reads S0/S1/S2, echoes S1 back as C2.
pub async fn client_handshake<S>(io: &mut S) -> Result<(), HandshakeError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let c1 = [0u8; HANDSHAKE_SIZE];
    io.write_all(&[RTMP_VERSION]).await?;
    io.write_all(&c1).await?;
    io.flush().await?;

    let mut s0 = [0u8; 1];
    io.read_exact(&mut s0).await?;
    if s0[0] != RTMP_VERSION {
        return Err(HandshakeError::UnsupportedVersion(s0[0]));
    }

    let mut s1 = [0u8; HANDSHAKE_SIZE];
    io.read_exact(&mut s1).await?;
    let mut s2 = [0u8; HANDSHAKE_SIZE];
    io.read_exact(&mut s2).await?;

    io.write_all(&s1).await?; // C2 echoes S1
    io.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn client_and_server_handshake_over_duplex() {
        let (mut client_io, mut server_io) = tokio::io::duplex(HANDSHAKE_SIZE * 4);
        let server = tokio::spawn(async move { server_handshake(&mut server_io).await });
        let client = tokio::spawn(async move { client_handshake(&mut client_io).await });

        let (server_res, client_res) = tokio::join!(server, client);
        server_res.expect("server task").expect("server handshake");
        client_res.expect("client task").expect("client handshake");
    }

    #[tokio::test]
    async fn server_rejects_bad_version() {
        let (mut client_io, mut server_io) = tokio::io::duplex(HANDSHAKE_SIZE * 4);
        let server = tokio::spawn(async move { server_handshake(&mut server_io).await });
        client_io.write_all(&[9]).await.expect("write bad version");
        client_io
            .write_all(&[0u8; HANDSHAKE_SIZE])
            .await
            .expect("write c1");
        client_io.flush().await.expect("flush");

        let result = server.await.expect("server task");
        assert!(matches!(result, Err(HandshakeError::UnsupportedVersion(9))));
    }
}
