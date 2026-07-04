//! Platform kanal eşleştirmeleri (Faz 2 hazırlığı A2).
//!
//! Bir yayın platformu kanalını (`platform` + `platform_channel_id`) bir
//! StreamDubber hesabına bağlar. Twitch Extension EBS'i (`ext_twitch`)
//! viewer isteklerinde `channel_id → user` çözümlemesini buradan yapar;
//! model platform-bağımsızdır — Chrome Extension (roadmap 2.5) aynı depoyu
//! kullanacak.
//!
//! Depo, pipeline sıcak yolunda okunacağı için bellek içidir; `DATABASE_URL`
//! verilmişse her değişiklik `channel_bindings` tablosuna best-effort yazılır
//! ve açılışta tablo belleğe yüklenir (pairing'in `sdk_sessions` desenine
//! paralel — DB hatası akışı durdurmaz, warn loglanır).

use std::collections::HashMap;
use std::sync::RwLock;

use serde::Serialize;

use crate::logging::{LogLevel, Logger};

/// Tek bir kanal eşleştirmesi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChannelBinding {
    pub user_id: String,
    pub platform: String,
    pub channel_id: String,
}

/// (platform, channel_id) → binding deposu.
pub struct ChannelBindingStore {
    entries: RwLock<HashMap<(String, String), ChannelBinding>>,
}

impl ChannelBindingStore {
    pub fn new() -> Self {
        ChannelBindingStore {
            entries: RwLock::new(HashMap::new()),
        }
    }

    /// Bir kanalı kullanıcıya bağlar. Aynı kanal zaten bağlıysa üzerine
    /// yazılır (kod claim'i hesap sahipliğini kanıtlar — yeniden bağlama
    /// bilinçli bir işlemdir).
    pub fn bind(&self, platform: &str, channel_id: &str, user_id: &str) -> ChannelBinding {
        let binding = ChannelBinding {
            user_id: user_id.to_string(),
            platform: platform.to_string(),
            channel_id: channel_id.to_string(),
        };
        if let Ok(mut map) = self.entries.write() {
            map.insert(
                (platform.to_string(), channel_id.to_string()),
                binding.clone(),
            );
        }
        binding
    }

    /// Kanalın bağlı olduğu hesabı çözer.
    pub fn lookup(&self, platform: &str, channel_id: &str) -> Option<ChannelBinding> {
        self.entries
            .read()
            .ok()
            .and_then(|map| map.get(&(platform.to_string(), channel_id.to_string())).cloned())
    }

    /// Bir kullanıcının bir platformdaki eşleştirmeleri (dashboard görünümü).
    pub fn for_user(&self, platform: &str, user_id: &str) -> Vec<ChannelBinding> {
        self.entries
            .read()
            .map(|map| {
                let mut out: Vec<ChannelBinding> = map
                    .values()
                    .filter(|b| b.platform == platform && b.user_id == user_id)
                    .cloned()
                    .collect();
                out.sort_by(|a, b| a.channel_id.cmp(&b.channel_id));
                out
            })
            .unwrap_or_default()
    }

    /// Bir kanalın eşleştirmesini kaldırır. Kayıt yoksa `false`.
    pub fn unbind(&self, platform: &str, channel_id: &str) -> bool {
        self.entries
            .write()
            .map(|mut map| {
                map.remove(&(platform.to_string(), channel_id.to_string()))
                    .is_some()
            })
            .unwrap_or(false)
    }

    /// Bir kullanıcının bir platformdaki tüm eşleştirmelerini kaldırır
    /// (dashboard "unlink"). Kaldırılan kayıt sayısı döner.
    pub fn unbind_user(&self, platform: &str, user_id: &str) -> usize {
        self.entries
            .write()
            .map(|mut map| {
                let before = map.len();
                map.retain(|_, b| !(b.platform == platform && b.user_id == user_id));
                before - map.len()
            })
            .unwrap_or(0)
    }

    /// DB'den yüklenen satırları belleğe alır (açılışta çağrılır).
    pub fn load(&self, bindings: impl IntoIterator<Item = ChannelBinding>) {
        if let Ok(mut map) = self.entries.write() {
            for binding in bindings {
                map.insert(
                    (binding.platform.clone(), binding.channel_id.clone()),
                    binding,
                );
            }
        }
    }
}

impl Default for ChannelBindingStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Bir binding'i `channel_bindings` tablosuna best-effort yazar (upsert).
/// Her hata warn'dur — bellek içi durum zaten günceldir, akış durmaz.
pub async fn persist_binding(pool: &sqlx::PgPool, binding: &ChannelBinding, logger: &Logger) {
    let Ok(user_uuid) = uuid::Uuid::parse_str(&binding.user_id) else {
        return;
    };
    let result = sqlx::query(
        "INSERT INTO channel_bindings (user_id, platform, platform_channel_id, status, updated_at) \
         VALUES ($1, $2, $3, 'active', NOW()) \
         ON CONFLICT (platform, platform_channel_id) \
         DO UPDATE SET user_id = EXCLUDED.user_id, status = 'active', updated_at = NOW()",
    )
    .bind(user_uuid)
    .bind(&binding.platform)
    .bind(&binding.channel_id)
    .execute(pool)
    .await;
    if let Err(err) = result {
        logger.log(
            LogLevel::Warn,
            "bindings",
            &format!("channel_bindings persist failed: {err}"),
            Some(&binding.user_id),
            None,
        );
    }
}

/// Bir kullanıcının bir platformdaki binding satırlarını best-effort siler.
pub async fn delete_bindings_for_user(
    pool: &sqlx::PgPool,
    platform: &str,
    user_id: &str,
    logger: &Logger,
) {
    let Ok(user_uuid) = uuid::Uuid::parse_str(user_id) else {
        return;
    };
    let result = sqlx::query("DELETE FROM channel_bindings WHERE platform = $1 AND user_id = $2")
        .bind(platform)
        .bind(user_uuid)
        .execute(pool)
        .await;
    if let Err(err) = result {
        logger.log(
            LogLevel::Warn,
            "bindings",
            &format!("channel_bindings delete failed: {err}"),
            Some(user_id),
            None,
        );
    }
}

/// `channel_bindings` tablosunu okur (açılış yüklemesi). Hata warn'dur —
/// tablo boş kabul edilir, servis yine ayağa kalkar.
pub async fn load_bindings(pool: &sqlx::PgPool, logger: &Logger) -> Vec<ChannelBinding> {
    let rows: Result<Vec<(uuid::Uuid, String, String)>, sqlx::Error> = sqlx::query_as(
        "SELECT user_id, platform, platform_channel_id FROM channel_bindings WHERE status = 'active'",
    )
    .fetch_all(pool)
    .await;
    match rows {
        Ok(rows) => rows
            .into_iter()
            .map(|(user_id, platform, channel_id)| ChannelBinding {
                user_id: user_id.to_string(),
                platform,
                channel_id,
            })
            .collect(),
        Err(err) => {
            logger.log(
                LogLevel::Warn,
                "bindings",
                &format!("channel_bindings load failed: {err}"),
                None,
                None,
            );
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_and_lookup_roundtrip() {
        let store = ChannelBindingStore::new();
        store.bind("twitch", "ch-1", "u-1");
        let found = store.lookup("twitch", "ch-1").expect("binding");
        assert_eq!(found.user_id, "u-1");
        assert_eq!(found.platform, "twitch");
        assert_eq!(found.channel_id, "ch-1");
    }

    #[test]
    fn unknown_channel_is_none() {
        let store = ChannelBindingStore::new();
        assert_eq!(store.lookup("twitch", "nope"), None);
    }

    #[test]
    fn rebind_replaces_user() {
        let store = ChannelBindingStore::new();
        store.bind("twitch", "ch-1", "u-1");
        store.bind("twitch", "ch-1", "u-2");
        assert_eq!(store.lookup("twitch", "ch-1").map(|b| b.user_id), Some("u-2".into()));
        // Eski kullanıcı artık listede görmez.
        assert!(store.for_user("twitch", "u-1").is_empty());
    }

    #[test]
    fn platforms_are_isolated() {
        let store = ChannelBindingStore::new();
        store.bind("twitch", "ch-1", "u-1");
        assert_eq!(store.lookup("kick", "ch-1"), None);
        assert!(store.for_user("kick", "u-1").is_empty());
    }

    #[test]
    fn for_user_lists_sorted_channels() {
        let store = ChannelBindingStore::new();
        store.bind("twitch", "ch-b", "u-1");
        store.bind("twitch", "ch-a", "u-1");
        store.bind("twitch", "ch-x", "u-2");
        let channels: Vec<String> = store
            .for_user("twitch", "u-1")
            .into_iter()
            .map(|b| b.channel_id)
            .collect();
        assert_eq!(channels, vec!["ch-a", "ch-b"]);
    }

    #[test]
    fn unbind_removes_only_target() {
        let store = ChannelBindingStore::new();
        store.bind("twitch", "ch-1", "u-1");
        store.bind("twitch", "ch-2", "u-1");
        assert!(store.unbind("twitch", "ch-1"));
        assert!(!store.unbind("twitch", "ch-1"));
        assert_eq!(store.for_user("twitch", "u-1").len(), 1);
    }

    #[test]
    fn unbind_user_clears_all_platform_bindings() {
        let store = ChannelBindingStore::new();
        store.bind("twitch", "ch-1", "u-1");
        store.bind("twitch", "ch-2", "u-1");
        store.bind("kick", "ch-3", "u-1");
        assert_eq!(store.unbind_user("twitch", "u-1"), 2);
        assert!(store.for_user("twitch", "u-1").is_empty());
        // Diğer platform korunur.
        assert_eq!(store.for_user("kick", "u-1").len(), 1);
    }

    #[test]
    fn load_populates_store() {
        let store = ChannelBindingStore::new();
        store.load([ChannelBinding {
            user_id: "u-1".into(),
            platform: "twitch".into(),
            channel_id: "ch-1".into(),
        }]);
        assert!(store.lookup("twitch", "ch-1").is_some());
    }
}
