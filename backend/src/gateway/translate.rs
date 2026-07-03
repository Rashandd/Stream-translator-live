//! Çeviri sağlayıcı soyutlaması.
//!
//! Gerçek DeepL/Google entegrasyonu Faz 1.8'de. Burada trait + mock.

use std::sync::Arc;

use async_trait::async_trait;

use crate::config::ServiceConfig;
use crate::secrets::SecretRef;

/// Çeviri sağlayıcı hatası.
#[derive(Debug, thiserror::Error)]
pub enum TranslateError {
    #[error("translate provider unavailable: {0}")]
    Unavailable(String),
    #[error("translate request failed: {0}")]
    Request(String),
}

/// Metni bir dilden diğerine çeviren sağlayıcı.
#[async_trait]
pub trait TranslateProvider: Send + Sync {
    /// Sağlayıcının adı (`translate.provider` değeriyle eşleşir).
    fn name(&self) -> &str;

    /// `text`'i `source` dilinden `target` diline çevirir.
    async fn translate(
        &self,
        text: &str,
        source: &str,
        target: &str,
    ) -> Result<String, TranslateError>;
}

/// Test ve iskelet için deterministik mock çeviri.
pub struct MockTranslate {
    name: String,
}

impl MockTranslate {
    pub fn new(name: impl Into<String>) -> Self {
        MockTranslate { name: name.into() }
    }
}

#[async_trait]
impl TranslateProvider for MockTranslate {
    fn name(&self) -> &str {
        &self.name
    }

    async fn translate(
        &self,
        text: &str,
        source: &str,
        target: &str,
    ) -> Result<String, TranslateError> {
        if text.is_empty() {
            return Err(TranslateError::Request("empty text".into()));
        }
        Ok(format!("[{source}->{target}] {text}"))
    }
}

/// Gerçek çeviri sağlayıcısı + mock fallback sarmalayıcısı.
///
/// Davranış [`crate::gateway::stt::MockFallbackStt`] ile aynıdır: key store'da
/// varsa gerçek sağlayıcı; yoksa `providers.mock_fallback=true` iken mock,
/// `false` iken açık `Unavailable` hatası.
pub struct MockFallbackTranslate {
    provider_name: String,
    key: SecretRef,
    real: Arc<dyn TranslateProvider>,
    mock: MockTranslate,
    config: Arc<ServiceConfig>,
}

impl MockFallbackTranslate {
    pub fn new(
        provider_name: impl Into<String>,
        key: SecretRef,
        real: Arc<dyn TranslateProvider>,
        config: Arc<ServiceConfig>,
    ) -> Self {
        let provider_name = provider_name.into();
        MockFallbackTranslate {
            mock: MockTranslate::new(provider_name.clone()),
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
impl TranslateProvider for MockFallbackTranslate {
    fn name(&self) -> &str {
        &self.provider_name
    }

    async fn translate(
        &self,
        text: &str,
        source: &str,
        target: &str,
    ) -> Result<String, TranslateError> {
        if self.key.resolve().await.is_some() {
            return self.real.translate(text, source, target).await;
        }
        if self.mock_allowed() {
            tracing::debug!(provider = %self.provider_name, "translate key not configured — using mock");
            return self.mock.translate(text, source, target).await;
        }
        Err(TranslateError::Unavailable(format!(
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
    async fn mock_translates() {
        let tr = MockTranslate::new("deepl");
        let out = tr.translate("merhaba", "tr", "en").await.expect("translate");
        assert_eq!(out, "[tr->en] merhaba");
    }

    #[tokio::test]
    async fn mock_rejects_empty() {
        let tr = MockTranslate::new("deepl");
        assert!(tr.translate("", "tr", "en").await.is_err());
    }

    struct FailingReal;

    #[async_trait]
    impl TranslateProvider for FailingReal {
        fn name(&self) -> &str {
            "deepl"
        }
        async fn translate(&self, _t: &str, _s: &str, _g: &str) -> Result<String, TranslateError> {
            Err(TranslateError::Request("real provider called".into()))
        }
    }

    #[tokio::test]
    async fn switches_from_mock_to_real_when_key_arrives() {
        let store = Arc::new(MemorySecretStore::new());
        let cfg = Arc::new(ServiceConfig::with_defaults());
        let tr = MockFallbackTranslate::new(
            "deepl",
            SecretRef::store(Arc::clone(&store) as Arc<dyn crate::secrets::SecretStore>, "DEEPL_KEY"),
            Arc::new(FailingReal),
            cfg,
        );

        assert_eq!(tr.translate("hi", "tr", "en").await.unwrap(), "[tr->en] hi");
        store.insert("DEEPL_KEY", "k");
        let err = tr.translate("hi", "tr", "en").await.unwrap_err();
        assert!(matches!(err, TranslateError::Request(msg) if msg.contains("real provider called")));
    }

    #[tokio::test]
    async fn strict_mode_errors_without_key() {
        let cfg = Arc::new(ServiceConfig::with_defaults());
        cfg.set("providers.mock_fallback", "false");
        let tr = MockFallbackTranslate::new(
            "deepl",
            SecretRef::none(),
            Arc::new(FailingReal),
            cfg,
        );
        assert!(matches!(
            tr.translate("hi", "tr", "en").await,
            Err(TranslateError::Unavailable(_))
        ));
    }
}
