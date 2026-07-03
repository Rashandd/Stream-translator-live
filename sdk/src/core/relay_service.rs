//! Localhost RTMP ingest dinleyicisi — OBS bu porta `rtmp://127.0.0.1:<port>/live`
//! ile bağlanır. `core::rtmp::relay` motorunun kabul ettiği "ingest" ucunu
//! gerçek bir TCP soketine bağlayan desktop-shell wiring parçası (bkz.
//! `core::rtmp::relay` modül dokümanındaki bölünme notu).

use tokio::net::{TcpListener, TcpStream};

use super::rtmp::{RelayEngine, RelayTarget, RtmpError};

#[derive(Debug, thiserror::Error)]
pub enum RelayServiceError {
    #[error("failed to bind/accept ingest listener: {0}")]
    Io(#[from] std::io::Error),
    #[error("rtmp relay error: {0}")]
    Rtmp(#[from] RtmpError),
}

/// OBS'in bağlanacağı localhost dinleyicisini açar. Adres tipik olarak
/// `127.0.0.1:<port>` — dışa açık olmamalı (yayıncının kendi makinesi).
pub async fn bind_ingest_listener(addr: &str) -> std::io::Result<TcpListener> {
    TcpListener::bind(addr).await
}

/// Bir OBS bağlantısı kabul eder ve relay motorunu verilen hedeflerle
/// başlatır. Tek seferliktir: OBS koparsa çağıran taraf tekrar
/// `accept_and_relay` çağırmalı (yeniden yayın başlatma senaryosu).
pub async fn accept_and_relay(
    listener: &TcpListener,
    targets: Vec<RelayTarget>,
) -> Result<RelayEngine, RelayServiceError> {
    let (socket, _peer): (TcpStream, _) = listener.accept().await?;
    let engine = RelayEngine::start(socket, targets).await?;
    Ok(engine)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn bind_yields_a_usable_local_address() {
        let listener = bind_ingest_listener("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
        assert!(addr.port() > 0);
    }

    /// `accept_and_relay` gerçek bir TCP bağlantısını `RelayEngine::start`'a
    /// iletir. Tam RTMP publish akışının kendisi (AMF0 komutları dahil)
    /// zaten `core::rtmp::relay`'in kendi testinde kapsanıyor — burada
    /// sadece yeni "gerçek dinleyici → accept → motor" bağının çalıştığını
    /// kanıtlamak için kasıtlı olarak eksik bir handshake (yalnızca C0)
    /// gönderip bağlantıyı kapatıyoruz: sunucu C1'i okurken EOF alır ve
    /// `RtmpError::Handshake` ile döner — engine::start'a gerçekten
    /// ulaşıldığının kanıtı.
    #[tokio::test]
    async fn accept_and_relay_forwards_the_real_socket_into_the_engine() {
        let listener = bind_ingest_listener("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");

        let obs = tokio::spawn(async move {
            let mut sock = TcpStream::connect(addr).await.expect("connect");
            sock.write_all(&[3]).await.expect("write c0"); // RTMP version byte only
            sock.shutdown().await.expect("shutdown"); // close before sending C1
        });

        let result = accept_and_relay(&listener, Vec::new()).await;
        obs.await.expect("obs task");

        match result {
            Err(RelayServiceError::Rtmp(RtmpError::Handshake(_))) => {}
            Err(other) => panic!("expected a handshake failure, got a different error: {other}"),
            Ok(_) => panic!("expected accept_and_relay to fail on an incomplete handshake"),
        }
    }
}
