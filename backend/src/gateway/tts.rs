//! TTS/ses klonlama sağlayıcı soyutlaması (Faz 2.1).
//!
//! Gerçek ElevenLabs entegrasyonu `gateway::elevenlabs`. Burada trait + mock,
//! böylece voice_models/gateway yönlendirmesi sağlayıcıdan bağımsız test
//! edilir — `gateway::stt`/`gateway::translate` ile aynı desen.

use std::sync::Arc;

use async_trait::async_trait;

use crate::config::ServiceConfig;
use crate::secrets::SecretRef;

/// TTS sağlayıcı hatası.
#[derive(Debug, thiserror::Error)]
pub enum TtsError {
    #[error("tts provider unavailable: {0}")]
    Unavailable(String),
    #[error("tts request failed: {0}")]
    Request(String),
}

/// Ses klonlama + metinden konuşma üreten sağlayıcı.
#[async_trait]
pub trait TtsProvider: Send + Sync {
    /// Sağlayıcının config anahtarındaki adı (`tts.provider` değeriyle eşleşir).
    fn name(&self) -> &str;

    /// Ses örneğinden klonlanmış bir ses modeli oluşturur, sağlayıcının
    /// model kimliğini döner (StreamDubber tarafında `VoiceModel.provider_voice_id`
    /// olarak saklanır).
    async fn clone_voice(&self, name: &str, sample: &[u8]) -> Result<String, TtsError>;

    /// Metni klonlanmış sesle konuşur, ham ses baytlarını döner (Faz 2.2
    /// gerçek zamanlı TTS pipeline'ı bunu kullanır).
    async fn synthesize(&self, voice_id: &str, text: &str) -> Result<Vec<u8>, TtsError>;
}

/// Test ve iskelet için deterministik mock TTS.
pub struct MockTts {
    name: String,
}

impl MockTts {
    pub fn new(name: impl Into<String>) -> Self {
        MockTts { name: name.into() }
    }
}

#[async_trait]
impl TtsProvider for MockTts {
    fn name(&self) -> &str {
        &self.name
    }

    async fn clone_voice(&self, name: &str, sample: &[u8]) -> Result<String, TtsError> {
        if sample.is_empty() {
            return Err(TtsError::Request("empty sample".into()));
        }
        Ok(format!("mock-voice-{name}-{}", sample.len()))
    }

    async fn synthesize(&self, voice_id: &str, text: &str) -> Result<Vec<u8>, TtsError> {
        if text.is_empty() {
            return Err(TtsError::Request("empty text".into()));
        }
        Ok(format!("[{voice_id}]{text}").into_bytes())
    }
}

/// Gerçek sağlayıcı + mock fallback sarmalayıcısı — `stt::MockFallbackStt`
/// ile birebir aynı desen (key yoksa mock, `providers.mock_fallback=false`
/// iken açık hata).
pub struct MockFallbackTts {
    provider_name: String,
    key: SecretRef,
    real: Arc<dyn TtsProvider>,
    mock: MockTts,
    config: Arc<ServiceConfig>,
}

impl MockFallbackTts {
    pub fn new(
        provider_name: impl Into<String>,
        key: SecretRef,
        real: Arc<dyn TtsProvider>,
        config: Arc<ServiceConfig>,
    ) -> Self {
        let provider_name = provider_name.into();
        MockFallbackTts {
            mock: MockTts::new(provider_name.clone()),
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
impl TtsProvider for MockFallbackTts {
    fn name(&self) -> &str {
        &self.provider_name
    }

    async fn clone_voice(&self, name: &str, sample: &[u8]) -> Result<String, TtsError> {
        if self.key.resolve().await.is_some() {
            return self.real.clone_voice(name, sample).await;
        }
        if self.mock_allowed() {
            tracing::debug!(provider = %self.provider_name, "tts key not configured — using mock");
            return self.mock.clone_voice(name, sample).await;
        }
        Err(TtsError::Unavailable(format!(
            "{} key not configured",
            self.provider_name
        )))
    }

    async fn synthesize(&self, voice_id: &str, text: &str) -> Result<Vec<u8>, TtsError> {
        if self.key.resolve().await.is_some() {
            return self.real.synthesize(voice_id, text).await;
        }
        if self.mock_allowed() {
            return self.mock.synthesize(voice_id, text).await;
        }
        Err(TtsError::Unavailable(format!(
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
    async fn mock_clones_and_synthesizes() {
        let tts = MockTts::new("elevenlabs");
        let voice_id = tts.clone_voice("my voice", &[1, 2, 3]).await.expect("clone");
        assert!(voice_id.contains("my voice"));
        let audio = tts.synthesize(&voice_id, "hello").await.expect("synth");
        assert!(!audio.is_empty());
    }

    #[tokio::test]
    async fn mock_rejects_empty_sample_or_text() {
        let tts = MockTts::new("elevenlabs");
        assert!(tts.clone_voice("x", &[]).await.is_err());
        assert!(tts.synthesize("voice-1", "").await.is_err());
    }

    struct FailingReal;

    #[async_trait]
    impl TtsProvider for FailingReal {
        fn name(&self) -> &str {
            "elevenlabs"
        }
        async fn clone_voice(&self, _name: &str, _sample: &[u8]) -> Result<String, TtsError> {
            Err(TtsError::Request("real provider called".into()))
        }
        async fn synthesize(&self, _voice_id: &str, _text: &str) -> Result<Vec<u8>, TtsError> {
            Err(TtsError::Request("real provider called".into()))
        }
    }

    fn wrapper_with(store: Arc<MemorySecretStore>, cfg: Arc<ServiceConfig>) -> MockFallbackTts {
        MockFallbackTts::new(
            "elevenlabs",
            SecretRef::store(store, "ELEVENLABS_KEY"),
            Arc::new(FailingReal),
            cfg,
        )
    }

    #[tokio::test]
    async fn falls_back_to_mock_without_key() {
        let cfg = Arc::new(ServiceConfig::with_defaults());
        let tts = wrapper_with(Arc::new(MemorySecretStore::new()), cfg);
        let voice_id = tts.clone_voice("x", &[1, 2, 3]).await.expect("mock path");
        assert!(voice_id.starts_with("mock-voice-"));
    }

    #[tokio::test]
    async fn uses_real_provider_once_key_is_set() {
        let store = Arc::new(MemorySecretStore::new());
        let cfg = Arc::new(ServiceConfig::with_defaults());
        let tts = wrapper_with(Arc::clone(&store), cfg);

        assert!(tts.clone_voice("x", &[1]).await.is_ok());
        store.insert("ELEVENLABS_KEY", "el-key");
        let err = tts.clone_voice("x", &[1]).await.unwrap_err();
        assert!(matches!(err, TtsError::Request(msg) if msg.contains("real provider called")));
    }

    #[tokio::test]
    async fn strict_mode_errors_instead_of_mock() {
        let cfg = Arc::new(ServiceConfig::with_defaults());
        cfg.set("providers.mock_fallback", "false");
        let tts = wrapper_with(Arc::new(MemorySecretStore::new()), cfg);
        let err = tts.clone_voice("x", &[1]).await.unwrap_err();
        assert!(matches!(err, TtsError::Unavailable(_)));
    }
}
