//! A relay destination: one platform's RTMP ingest + the broadcaster's own
//! stream key for it. The web panel is the source of truth for the list
//! (synced to the SDK encrypted, per CLAUDE.md); this module only owns
//! parsing/validating one entry.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayTarget {
    pub id: String,
    pub label: String,
    pub host: String,
    pub port: u16,
    pub app: String,
    pub tc_url: String,
    pub stream_key: String,
    /// Doluysa bu hedef bir dublaj hedefidir (bkz. `core::mux`) — relay
    /// motoru bu hedefi doğrudan RTMP-out yerine `run_dub_target`'a
    /// yönlendirir. `parse()` bunu her zaman `None` bırakır; backend'den
    /// gelen değeri taşımak isteyen çağıran `with_dub_language` kullanır
    /// (bkz. `relay_sync.rs`).
    pub dub_language: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TargetError {
    #[error("rtmp url must start with rtmp://")]
    MissingScheme,
    #[error("rtmp url is missing a host")]
    MissingHost,
    #[error("rtmp url is missing an app path")]
    MissingApp,
    #[error("stream key must not be empty")]
    EmptyStreamKey,
}

impl RelayTarget {
    /// Parses `rtmp://host[:port]/app[/...]` plus a separate stream key
    /// (never embedded in the URL — it's supplied by the platform per
    /// broadcaster and kept out of any logged/displayed URL).
    pub fn parse(
        id: impl Into<String>,
        label: impl Into<String>,
        rtmp_url: &str,
        stream_key: &str,
    ) -> Result<Self, TargetError> {
        let rest = rtmp_url
            .strip_prefix("rtmp://")
            .ok_or(TargetError::MissingScheme)?;
        let (authority, path) = rest.split_once('/').ok_or(TargetError::MissingApp)?;
        if authority.is_empty() {
            return Err(TargetError::MissingHost);
        }
        let app = path.trim_matches('/');
        if app.is_empty() {
            return Err(TargetError::MissingApp);
        }
        if stream_key.trim().is_empty() {
            return Err(TargetError::EmptyStreamKey);
        }

        let (host, port) = match authority.split_once(':') {
            Some((h, p)) if !h.is_empty() => (h.to_string(), p.parse::<u16>().unwrap_or(1935)),
            _ => (authority.to_string(), 1935),
        };

        Ok(RelayTarget {
            id: id.into(),
            label: label.into(),
            host,
            port,
            app: app.to_string(),
            tc_url: format!("rtmp://{authority}/{app}"),
            stream_key: stream_key.to_string(),
            dub_language: None,
        })
    }

    /// Sets the dub-target language (chainable — see the field doc).
    pub fn with_dub_language(mut self, dub_language: Option<String>) -> Self {
        self.dub_language = dub_language;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_url_with_explicit_port() {
        let t = RelayTarget::parse("t1", "Twitch", "rtmp://live.twitch.tv:1935/app", "sk-1")
            .expect("parse");
        assert_eq!(t.host, "live.twitch.tv");
        assert_eq!(t.port, 1935);
        assert_eq!(t.app, "app");
        assert_eq!(t.tc_url, "rtmp://live.twitch.tv:1935/app");
        assert_eq!(t.stream_key, "sk-1");
    }

    #[test]
    fn defaults_to_port_1935_without_explicit_port() {
        let t = RelayTarget::parse("t2", "YouTube", "rtmp://a.rtmp.youtube.com/live2", "sk-2")
            .expect("parse");
        assert_eq!(t.port, 1935);
        assert_eq!(t.host, "a.rtmp.youtube.com");
        assert_eq!(t.app, "live2");
    }

    #[test]
    fn rejects_missing_scheme() {
        assert_eq!(
            RelayTarget::parse("t", "l", "http://host/app", "k").unwrap_err(),
            TargetError::MissingScheme
        );
    }

    #[test]
    fn rejects_missing_app_path() {
        assert_eq!(
            RelayTarget::parse("t", "l", "rtmp://host", "k").unwrap_err(),
            TargetError::MissingApp
        );
        assert_eq!(
            RelayTarget::parse("t", "l", "rtmp://host/", "k").unwrap_err(),
            TargetError::MissingApp
        );
    }

    #[test]
    fn parse_defaults_dub_language_to_none_and_with_dub_language_sets_it() {
        let t = RelayTarget::parse("t1", "Twitch", "rtmp://live.twitch.tv/app", "sk-1").expect("parse");
        assert_eq!(t.dub_language, None);

        let dub = t.with_dub_language(Some("en".to_string()));
        assert_eq!(dub.dub_language.as_deref(), Some("en"));
    }

    #[test]
    fn rejects_empty_stream_key() {
        assert_eq!(
            RelayTarget::parse("t", "l", "rtmp://host/app", "  ").unwrap_err(),
            TargetError::EmptyStreamKey
        );
    }
}
