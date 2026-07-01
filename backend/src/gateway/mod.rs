//! API gateway — STT ve çeviri sağlayıcı yönlendirmesi.
//!
//! Aktif sağlayıcı `ServiceConfig`'ten okunur (`stt.provider`,
//! `translate.provider`). Hata olursa `*.fallback` devreye girer. Hedef diller
//! paralel çevrilir (Tokio task'ları).

pub mod deepgram;
pub mod deepl;
pub mod stt;
pub mod translate;

use std::collections::HashMap;
use std::sync::Arc;

use tokio::task::JoinSet;

use crate::config::ServiceConfig;
use crate::ws::protocol::Translation;
use stt::{SttError, SttProvider, Transcript};
use translate::{TranslateError, TranslateProvider};

/// Kayıtlı sağlayıcıları tutan ve config'e göre yönlendiren gateway.
pub struct Gateway {
    stt: HashMap<String, Arc<dyn SttProvider>>,
    translate: HashMap<String, Arc<dyn TranslateProvider>>,
    config: Arc<ServiceConfig>,
}

/// Gateway seviyesi hatalar.
#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    #[error("no provider configured for {0}")]
    NoProvider(&'static str),
    #[error("stt failed (primary and fallback): {0}")]
    Stt(#[from] SttError),
    #[error("translate failed (primary and fallback): {0}")]
    Translate(#[from] TranslateError),
}

impl Gateway {
    /// Boş bir gateway kurar.
    pub fn new(config: Arc<ServiceConfig>) -> Self {
        Gateway {
            stt: HashMap::new(),
            translate: HashMap::new(),
            config,
        }
    }

    /// Bir STT sağlayıcısı kaydeder.
    pub fn register_stt(&mut self, provider: Arc<dyn SttProvider>) {
        self.stt.insert(provider.name().to_string(), provider);
    }

    /// Bir çeviri sağlayıcısı kaydeder.
    pub fn register_translate(&mut self, provider: Arc<dyn TranslateProvider>) {
        self.translate
            .insert(provider.name().to_string(), provider);
    }

    /// Ses parçasını aktif STT sağlayıcısıyla (gerekirse fallback) transkribe eder.
    pub async fn transcribe(
        &self,
        audio: &[u8],
        source_lang: &str,
    ) -> Result<Transcript, GatewayError> {
        let primary_name = self
            .config
            .get("stt.provider")
            .ok_or(GatewayError::NoProvider("stt.provider"))?;

        if let Some(provider) = self.stt.get(&primary_name) {
            match provider.transcribe(audio, source_lang).await {
                Ok(t) => return Ok(t),
                Err(err) => {
                    tracing::warn!(provider = %primary_name, error = %err, "stt primary failed, trying fallback");
                }
            }
        }

        if let Some(fallback_name) = self.config.get("stt.fallback") {
            if let Some(provider) = self.stt.get(&fallback_name) {
                return Ok(provider.transcribe(audio, source_lang).await?);
            }
        }

        Err(GatewayError::NoProvider("stt.provider"))
    }

    /// Bir metni tüm hedef dillere paralel çevirir.
    ///
    /// Sonuç sırası `targets` ile aynıdır. Bir dil hata verirse o dil atlanır,
    /// diğerleri yine döner (best-effort altyazı).
    pub async fn translate_all(
        &self,
        text: &str,
        source: &str,
        targets: &[String],
    ) -> Vec<Translation> {
        let mut set: JoinSet<(usize, Option<String>)> = JoinSet::new();

        for (idx, target) in targets.iter().enumerate() {
            // Paralel task'lar için config'ten provider'ı çözüp Arc klonla.
            let provider = self
                .config
                .get("translate.provider")
                .and_then(|name| self.translate.get(&name).cloned());
            let fallback = self
                .config
                .get("translate.fallback")
                .and_then(|name| self.translate.get(&name).cloned());

            let text = text.to_string();
            let source = source.to_string();
            let target = target.to_string();

            set.spawn(async move {
                let out = run_with_fallback(provider, fallback, &text, &source, &target).await;
                (idx, out)
            });
        }

        // idx → text eşlemesini topla, sonra sırala.
        let mut indexed: Vec<(usize, String)> = Vec::with_capacity(targets.len());
        while let Some(joined) = set.join_next().await {
            if let Ok((idx, Some(text))) = joined {
                indexed.push((idx, text));
            }
        }
        indexed.sort_by_key(|(idx, _)| *idx);

        indexed
            .into_iter()
            .map(|(idx, text)| Translation {
                lang: targets[idx].clone(),
                text,
            })
            .collect()
    }
}

/// Primary → fallback sırasıyla tek çeviri dener. Hepsi başarısızsa `None`.
async fn run_with_fallback(
    primary: Option<Arc<dyn TranslateProvider>>,
    fallback: Option<Arc<dyn TranslateProvider>>,
    text: &str,
    source: &str,
    target: &str,
) -> Option<String> {
    if let Some(p) = primary {
        if let Ok(out) = p.translate(text, source, target).await {
            return Some(out);
        }
    }
    if let Some(f) = fallback {
        if let Ok(out) = f.translate(text, source, target).await {
            return Some(out);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use stt::MockStt;
    use translate::MockTranslate;

    fn gateway() -> Gateway {
        let config = Arc::new(ServiceConfig::with_defaults());
        let mut gw = Gateway::new(config);
        gw.register_stt(Arc::new(MockStt::new("deepgram")));
        gw.register_stt(Arc::new(MockStt::new("assemblyai")));
        gw.register_translate(Arc::new(MockTranslate::new("deepl")));
        gw.register_translate(Arc::new(MockTranslate::new("google")));
        gw
    }

    #[tokio::test]
    async fn transcribes_with_active_provider() {
        let gw = gateway();
        let t = gw.transcribe(&[1, 2, 3], "tr").await.expect("transcribe");
        assert!(t.is_final);
    }

    #[tokio::test]
    async fn translate_all_preserves_target_order() {
        let gw = gateway();
        let targets = vec!["en".to_string(), "de".to_string(), "fr".to_string()];
        let out = gw.translate_all("merhaba", "tr", &targets).await;
        let langs: Vec<&str> = out.iter().map(|t| t.lang.as_str()).collect();
        assert_eq!(langs, vec!["en", "de", "fr"]);
        assert_eq!(out[0].text, "[tr->en] merhaba");
    }

    #[tokio::test]
    async fn no_provider_when_config_points_to_unknown() {
        let config = Arc::new(ServiceConfig::with_defaults());
        config.set("stt.provider", "ghost");
        config.set("stt.fallback", "phantom");
        let gw = Gateway::new(config);
        assert!(matches!(
            gw.transcribe(&[1], "tr").await,
            Err(GatewayError::NoProvider(_))
        ));
    }
}
