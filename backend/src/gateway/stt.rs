//! STT (ses → metin) sağlayıcı soyutlaması.
//!
//! Gerçek Deepgram/AssemblyAI entegrasyonu Faz 1.7'de. Burada trait + mock,
//! böylece pipeline ve gateway yönlendirmesi sağlayıcıdan bağımsız test edilir.

use async_trait::async_trait;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
