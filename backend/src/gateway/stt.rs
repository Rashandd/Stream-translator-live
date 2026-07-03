//! STT (ses → metin) sağlayıcı soyutlaması.
//!
//! Gerçek Deepgram/AssemblyAI entegrasyonu Faz 1.7'de. Burada trait + mock,
//! böylece pipeline ve gateway yönlendirmesi sağlayıcıdan bağımsız test edilir.

use std::sync::Arc;

use async_trait::async_trait;

use crate::config::ServiceConfig;
use crate::secrets::SecretRef;

/// Bir STT transkript sonucu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transcript {
    pub text: String,
    pub is_final: bool,
}

/// STT sağlayıcı hatası.
#[derive(Debug, thiserror::Error)]
pub enum SttError {
    #[error("stt provider unavailable: {0}")]
    Unavailable(String),
    #[error("stt request failed: {0}")]
    Request(String),
}

/// Ham ses parçalarını metne çeviren sağlayıcı.
#[async_trait]
pub trait SttProvider: Send + Sync {
    /// Sağlayıcının config anahtarındaki adı (`stt.provider` değeriyle eşleşir).
    fn name(&self) -> &str;

    /// Bir ses parçasını (codec'e göre ham payload) transkribe eder.
    async fn transcribe(&self, audio: &[u8], source_lang: &str) -> Result<Transcript, SttError>;
}

/// Test ve iskelet için deterministik mock STT.
pub struct MockStt {
    name: String,
}

impl MockStt {
    pub fn new(name: impl Into<String>) -> Self {
        MockStt { name: name.into() }
    }
}

#[async_trait]
impl SttProvider for MockStt {
    fn name(&self) -> &str {
        &self.name
    }

    async fn transcribe(&self, audio: &[u8], _source_lang: &str) -> Result<Transcript, SttError> {
        // İskelet davranış: boş payload hata, dolu payload sahte transkript.
        if audio.is_empty() {
            return Err(SttError::Request("empty audio".into()));
        }
        Ok(Transcript {
            text: format!("[{} bytes transcribed]", audio.len()),
            is_final: true,
        })
    }
}

/// Gerçek sağlayıcı + mock fallback sarmalayıcısı.
///
/// Key store'da varsa gerçek sağlayıcıya gider; yoksa `providers.mock_fallback`
/// config'i `true` iken (dev/CI varsayılanı) mock cevap üretir, `false` iken
/// (üretim) açık `Unavailable` hatası döner — sessizce mock'a düşmez.
/// Key admin panelden girildiği anda sonraki çağrı gerçek sağlayıcıyı kullanır.
pub struct MockFallbackStt {
    provider_name: String,
    key: SecretRef,
    real: Arc<dyn SttProvider>,
    mock: MockStt,
    config: Arc<ServiceConfig>,
}

impl MockFallbackStt {
    pub fn new(
        provider_name: impl Into<String>,
        key: SecretRef,
        real: Arc<dyn SttProvider>,
        config: Arc<ServiceConfig>,
    ) -> Self {
        let provider_name = provider_name.into();
        MockFallbackStt {
            mock: MockStt::new(provider_name.clone()),
            provider_name,
            key,
            real,
            config,
        }
    }

    fn mock_allowed(&self) -> bool {
        self.config.get("providers.mock_fallback").as_deref() != Some("false")
    }
}

#[async_trait]
impl SttProvider for MockFallbackStt {
    fn name(&self) -> &str {
        &self.provider_name
    }

    async fn transcribe(&self, audio: &[u8], source_lang: &str) -> Result<Transcript, SttError> {
        if self.key.resolve().await.is_some() {
            return self.real.transcribe(audio, source_lang).await;
        }
        if self.mock_allowed() {
            tracing::debug!(provider = %self.provider_name, "stt key not configured — using mock");
            return self.mock.transcribe(audio, source_lang).await;
        }
        Err(SttError::Unavailable(format!(
            "{} key not configured",
            self.provider_name
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::MemorySecretStore;

    #[tokio::test]
    async fn mock_transcribes_nonempty() {
        let stt = MockStt::new("deepgram");
        let t = stt.transcribe(&[1, 2, 3], "tr").await.expect("transcribe");
        assert!(t.is_final);
        assert!(t.text.contains("3 bytes"));
    }

    #[tokio::test]
    async fn mock_rejects_empty() {
        let stt = MockStt::new("deepgram");
        assert!(stt.transcribe(&[], "tr").await.is_err());
    }

    /// Her çağrıda hata veren sahte "gerçek" sağlayıcı — sarmalayıcının hangi
    /// yolu seçtiğini gözlemlemek için.
    struct FailingReal;

    #[async_trait]
    impl SttProvider for FailingReal {
        fn name(&self) -> &str {
            "deepgram"
        }
        async fn transcribe(&self, _a: &[u8], _l: &str) -> Result<Transcript, SttError> {
            Err(SttError::Request("real provider called".into()))
        }
    }

    fn wrapper_with(store: Arc<MemorySecretStore>, cfg: Arc<ServiceConfig>) -> MockFallbackStt {
        MockFallbackStt::new(
            "deepgram",
            SecretRef::store(store, "DEEPGRAM_KEY"),
            Arc::new(FailingReal),
            cfg,
        )
    }

    #[tokio::test]
    async fn falls_back_to_mock_without_key() {
        let cfg = Arc::new(ServiceConfig::with_defaults());
        let stt = wrapper_with(Arc::new(MemorySecretStore::new()), cfg);
        let t = stt.transcribe(&[1, 2, 3], "tr").await.expect("mock path");
        assert!(t.text.contains("3 bytes"));
    }

    #[tokio::test]
    async fn uses_real_provider_once_key_is_set() {
        let store = Arc::new(MemorySecretStore::new());
        let cfg = Arc::new(ServiceConfig::with_defaults());
        let stt = wrapper_with(Arc::clone(&store), cfg);

        // Key yok → mock.
        assert!(stt.transcribe(&[1], "tr").await.is_ok());
        // Key runtime'da girildi (admin panel) → restart'sız gerçek sağlayıcı.
        store.insert("DEEPGRAM_KEY", "dg-key");
        let err = stt.transcribe(&[1], "tr").await.unwrap_err();
        assert!(matches!(err, SttError::Request(msg) if msg.contains("real provider called")));
    }

    #[tokio::test]
    async fn strict_mode_errors_instead_of_mock() {
        let cfg = Arc::new(ServiceConfig::with_defaults());
        cfg.set("providers.mock_fallback", "false");
        let stt = wrapper_with(Arc::new(MemorySecretStore::new()), cfg);
        let err = stt.transcribe(&[1], "tr").await.unwrap_err();
        assert!(matches!(err, SttError::Unavailable(_)));
    }
}
