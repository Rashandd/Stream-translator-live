//! SSH tabanlı kurulum soyutlaması (roadmap 2.6 adım 2/6).
//!
//! Gerçek implementasyon (`SshProvisioner`, adım 6'da eklenecek) admin'in
//! eklediği herhangi bir GPU'lu VPS/VDS'e (sağlayıcıdan bağımsız — Hetzner,
//! OVH, Vast.ai, kendi donanımı, fark etmez) SSH ile bağlanıp
//! `infra/stt-server/bootstrap.sh`'i çalıştırır. Bu dosya sadece trait +
//! test/iskelet implementasyonunu (`FakeProvisioner`) taşır — böylece admin
//! route'ları ve durum makinesi gerçek SSH olmadan test edilebilir
//! (`SttProvider`/`SecretStore` trait'lerinde zaten kullanılan aynı desen).

use async_trait::async_trait;

use super::SttServer;

/// Bir kurulumun başarılı sonucu — `SttServerStore::mark_ready`'e geçirilir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisionOutcome {
    pub endpoint_url: String,
    pub api_token: String,
    pub gpu_name: Option<String>,
    pub vram_mb: Option<u64>,
}

/// Kurulum hataları.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum ProvisionError {
    #[error("ssh connection failed: {0}")]
    Connection(String),
    #[error("host fingerprint mismatch — possible reprovisioned/MITM host: {0}")]
    FingerprintMismatch(String),
    #[error("bootstrap script failed: {0}")]
    Bootstrap(String),
    #[error("unexpected bootstrap output: {0}")]
    Malformed(String),
}

/// Bir sunucuyu SSH ile kurup hazır hale getiren soyutlama.
#[async_trait]
pub trait Provisioner: Send + Sync {
    async fn provision(&self, server: &SttServer) -> Result<ProvisionOutcome, ProvisionError>;
}

/// Test/iskelet implementasyonu: sabit bir sonuç döner, gerçek SSH yapmaz.
pub struct FakeProvisioner {
    result: Result<ProvisionOutcome, ProvisionError>,
}

impl FakeProvisioner {
    /// Verilen sonucu her çağrıda aynen döner.
    pub fn always(result: Result<ProvisionOutcome, ProvisionError>) -> Self {
        FakeProvisioner { result }
    }
}

#[async_trait]
impl Provisioner for FakeProvisioner {
    async fn provision(&self, _server: &SttServer) -> Result<ProvisionOutcome, ProvisionError> {
        self.result.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server() -> SttServer {
        super::super::SttServerStore::in_memory()
            .add("gpu-1", "1.2.3.4", 22, "root")
            .expect("add")
    }

    #[tokio::test]
    async fn fake_provisioner_returns_configured_success() {
        let outcome = ProvisionOutcome {
            endpoint_url: "https://gpu-1:8443".into(),
            api_token: "tok".into(),
            gpu_name: Some("RTX 3070".into()),
            vram_mb: Some(8192),
        };
        let provisioner = FakeProvisioner::always(Ok(outcome.clone()));
        assert_eq!(provisioner.provision(&server()).await, Ok(outcome));
    }

    #[tokio::test]
    async fn fake_provisioner_returns_configured_error() {
        let provisioner = FakeProvisioner::always(Err(ProvisionError::Connection("timeout".into())));
        assert!(matches!(
            provisioner.provision(&server()).await,
            Err(ProvisionError::Connection(_))
        ));
    }
}
