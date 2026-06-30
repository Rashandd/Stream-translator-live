//! TLS 1.3 ve mTLS yapılandırması (rustls).
//!
//! - Transit şifreleme TLS 1.3 ile yapılır (ses akışı dahil; üstüne AES yok).
//! - mTLS: `client_ca` verilirse istemci sertifikası zorunlu olur — SDK ↔ bulut
//!   karşılıklı doğrulama (`docs/security.md`).
//!
//! Sertifikalar env yollarından okunur; üretimde Vault/dosya, local dev'de
//! `infra/tls/gen-dev-certs.sh` ile üretilir. Sertifika yoksa sunucu düz HTTP
//! çalışır (reverse-proxy TLS terminasyonu senaryosu için).

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::server::WebPkiClientVerifier;
use rustls::{RootCertStore, ServerConfig};

/// TLS dosya yolları (env'den).
#[derive(Debug, Clone)]
pub struct TlsSettings {
    pub cert_path: String,
    pub key_path: String,
    /// Verilirse mTLS aktive olur — istemci sertifikası bu CA ile doğrulanır.
    pub client_ca_path: Option<String>,
}

/// TLS kurulum hataları.
#[derive(Debug, thiserror::Error)]
pub enum TlsError {
    #[error("failed to read {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("no private key found in {0}")]
    NoKey(String),
    #[error("no certificates found in {0}")]
    NoCerts(String),
    #[error("invalid client CA: {0}")]
    ClientCa(String),
    #[error("rustls config error: {0}")]
    Rustls(#[from] rustls::Error),
    #[error("client verifier error: {0}")]
    Verifier(String),
}

impl TlsSettings {
    /// `TLS_CERT_PATH` ve `TLS_KEY_PATH` ikisi de varsa ayarları kurar.
    /// `TLS_CLIENT_CA_PATH` opsiyoneldir (mTLS).
    pub fn from_env() -> Option<Self> {
        let cert_path = std::env::var("TLS_CERT_PATH").ok()?;
        let key_path = std::env::var("TLS_KEY_PATH").ok()?;
        let client_ca_path = std::env::var("TLS_CLIENT_CA_PATH").ok();
        Some(TlsSettings {
            cert_path,
            key_path,
            client_ca_path,
        })
    }

    /// mTLS açık mı?
    pub fn mutual(&self) -> bool {
        self.client_ca_path.is_some()
    }
}

/// PEM'den sertifika zincirini okur.
fn load_certs(path: &str) -> Result<Vec<CertificateDer<'static>>, TlsError> {
    let mut reader = open(path)?;
    let certs: Vec<_> = rustls_pemfile::certs(&mut reader)
        .collect::<Result<_, _>>()
        .map_err(|source| TlsError::Io {
            path: path.to_string(),
            source,
        })?;
    if certs.is_empty() {
        return Err(TlsError::NoCerts(path.to_string()));
    }
    Ok(certs)
}

/// PEM'den özel anahtarı okur.
fn load_key(path: &str) -> Result<PrivateKeyDer<'static>, TlsError> {
    let mut reader = open(path)?;
    rustls_pemfile::private_key(&mut reader)
        .map_err(|source| TlsError::Io {
            path: path.to_string(),
            source,
        })?
        .ok_or_else(|| TlsError::NoKey(path.to_string()))
}

fn open(path: &str) -> Result<BufReader<File>, TlsError> {
    let file = File::open(Path::new(path)).map_err(|source| TlsError::Io {
        path: path.to_string(),
        source,
    })?;
    Ok(BufReader::new(file))
}

/// TLS 1.3-only bir `ServerConfig` kurar. `client_ca` varsa mTLS zorunlu olur.
///
/// Crypto provider explicit verilir: rustls feature'larında hem `aws-lc-rs`
/// (rustls default) hem `ring` (tokio-tungstenite) aktif olduğundan process-level
/// default belirsiz kalır; bu yüzden aws-lc-rs provider'ı doğrudan kullanılır.
pub fn build_server_config(settings: &TlsSettings) -> Result<Arc<ServerConfig>, TlsError> {
    let certs = load_certs(&settings.cert_path)?;
    let key = load_key(&settings.key_path)?;

    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());

    // Yalnızca TLS 1.3, explicit provider ile.
    let builder = ServerConfig::builder_with_provider(Arc::clone(&provider))
        .with_protocol_versions(&[&rustls::version::TLS13])?;

    let config = match &settings.client_ca_path {
        Some(ca_path) => {
            let ca_certs = load_certs(ca_path)?;
            let mut roots = RootCertStore::empty();
            for cert in ca_certs {
                roots
                    .add(cert)
                    .map_err(|e| TlsError::ClientCa(e.to_string()))?;
            }
            let verifier = WebPkiClientVerifier::builder_with_provider(Arc::new(roots), provider)
                .build()
                .map_err(|e| TlsError::Verifier(e.to_string()))?;
            builder
                .with_client_cert_verifier(verifier)
                .with_single_cert(certs, key)?
        }
        None => builder.with_no_client_auth().with_single_cert(certs, key)?,
    };

    Ok(Arc::new(config))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_cert_file_errors() {
        let settings = TlsSettings {
            cert_path: "/nonexistent/cert.pem".into(),
            key_path: "/nonexistent/key.pem".into(),
            client_ca_path: None,
        };
        assert!(matches!(
            build_server_config(&settings),
            Err(TlsError::Io { .. })
        ));
    }

    #[test]
    fn mutual_flag_reflects_client_ca() {
        let mut s = TlsSettings {
            cert_path: "c".into(),
            key_path: "k".into(),
            client_ca_path: None,
        };
        assert!(!s.mutual());
        s.client_ca_path = Some("ca.pem".into());
        assert!(s.mutual());
    }
}
