//! Çeviri sağlayıcı soyutlaması.
//!
//! Gerçek DeepL/Google entegrasyonu Faz 1.8'de. Burada trait + mock.

use async_trait::async_trait;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
