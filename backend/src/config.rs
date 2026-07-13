//! Çalışma zamanı konfigürasyonu.
//!
//! İki katman:
//! - [`Config`] — başlangıç/dağıtım değerleri (adres, secret), env var'dan.
//! - [`ServiceConfig`] — sağlayıcı toggle'ları (`stt.provider` vb.). Runtime'da
//!   değişir, restart gerektirmez. Faz 1 iskeletinde bellek içi; ileride
//!   `service_config` tablosundan beslenir.

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::sync::RwLock;

/// Sunucu konfigürasyonu. Secret'lar burada DEĞİL — `secrets::SecretStore`'da.
#[derive(Debug, Clone)]
pub struct Config {
    /// WebSocket/HTTP sunucusunun bağlanacağı adres.
    pub bind_addr: SocketAddr,
    /// PostgreSQL bağlantı dizesi (varsa migration/pool için).
    pub database_url: Option<String>,
}

/// Konfigürasyon yükleme hataları.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("invalid {var}={value}: {source}")]
    InvalidAddr {
        var: &'static str,
        value: String,
        source: std::net::AddrParseError,
    },
}

impl Config {
    /// Konfigürasyonu ortam değişkenlerinden yükler.
    ///
    /// - `BIND_ADDR` — varsayılan `0.0.0.0:8080`
    /// - `DATABASE_URL` — opsiyonel
    pub fn from_env() -> Result<Self, ConfigError> {
        const BIND_VAR: &str = "BIND_ADDR";
        let raw = std::env::var(BIND_VAR).unwrap_or_else(|_| "0.0.0.0:8080".to_string());
        let bind_addr = raw.parse().map_err(|source| ConfigError::InvalidAddr {
            var: BIND_VAR,
            value: raw,
            source,
        })?;

        let database_url = std::env::var("DATABASE_URL").ok();

        Ok(Config {
            bind_addr,
            database_url,
        })
    }
}

/// Runtime sağlayıcı konfigürasyonu — `config::get("stt.provider")` deseni.
///
/// Değerler kod yerine bu store'dan okunur; hiçbir sağlayıcı adı hard-coded
/// değildir. Başlangıç değerleri `docs/schema.md`'deki `service_config`
/// seed'iyle birebirdir.
pub struct ServiceConfig {
    values: RwLock<HashMap<String, String>>,
}

impl ServiceConfig {
    /// `service_config` seed değerleriyle bir store kurar.
    pub fn with_defaults() -> Self {
        let defaults = [
            ("stt.provider", "deepgram"),
            ("stt.fallback", "assemblyai"),
            // Lokal STT (Faz 2.6b): hangi planlar `stream_start`'ta
            // stt_source=local bildirebilir (virgülle ayrılmış liste; lokal
            // STT bulut STT maliyetini düşürdüğünden varsayılan herkese açık)
            // + tek transcript satırı için karakter üst sınırı (çeviri API'si
            // karakter başına faturalandığından maliyet koruması).
            ("stt.local.plans", "free,starter,pro,streamer"),
            ("stt.local.max_chars", "500"),
            // Yerel STT kota teşviki (docs/local-stt-plan.md §3): transcript
            // yolundan işlenen dakikalar bu çarpanla kotadan düşer (0.5 =
            // "kotan yarı hızda tükenir"). Ses/fallback yolu her zaman 1.0 —
            // indirim yalnız gerçekten yerel işlenen dakikalara uygulanır.
            ("stt.local.usage_multiplier", "0.5"),
            // Kredi sistemi (roadmap "Fiyatlandırma — Kredi Sistemi"):
            // backend'in kendi işlediği (indirimsiz) STT saatinin kredi
            // ağırlığı. Admin `stt.provider`'ı self-hosted bir modelle
            // değiştirdikçe gerçek GPU $/saat maliyetine göre günceller.
            ("stt.cloud.credit_weight", "1.0"),
            // Yerel STT donanım/watchdog politikası (docs/local-stt-plan.md §2-3).
            // SDK bu eşikleri GET /api/stt/policy'den çeker — SDK güncellemeden
            // panelden ayarlanır. min_vram_mb: etkinleştirme şartı;
            // benchmark_rtf_max: testin geçer notu (15sn ses / süre);
            // watchdog_p95_ms: yayında bulut fallback eşiği;
            // retry_minutes: fallback sonrası yerel tekrar deneme (histerezis).
            ("stt.local.min_vram_mb", "6144"),
            ("stt.local.benchmark_rtf_max", "0.5"),
            ("stt.local.watchdog_p95_ms", "1500"),
            ("stt.local.retry_minutes", "5"),
            ("translate.provider", "deepl"),
            ("translate.fallback", "google"),
            // DeepL hedef dil matrisi (Faz 2.0): admin panelden hangi
            // kodların yayıncıya sunulacağını daraltır (virgülle ayrılmış,
            // translate_languages::DEEPL_TARGET_LANGUAGES katalogunun alt
            // kümesi). Varsayılan: tam katalog açık. Aşağıda `with_defaults`
            // sonunda gerçek değerle doldurulur (katalog çalışma zamanında
            // üretiliyor, literal değil).
            ("translate.deepl.enabled_targets", ""),
            // Ses klonlama/TTS (Faz 2.1): "disabled" iken Gateway::clone_voice
            // ve ::synthesize hep NoProvider döner — admin panelden
            // "elevenlabs" yapılınca devreye girer (PUT /admin/config;
            // ext.twitch.enabled ile aynı "kapalı varsayılan, admin açar"
            // deseni). Ayrıca yayıncı planında `has_voice_clone` şart
            // (PlanRegistry: free/starter=false, pro/streamer=true).
            ("tts.provider", "disabled"),
            ("tts.fallback", ""),
            // Yerel TTS (Faz 2.2b — stt.local.* ile birebir aynı desen):
            // hangi planlar `stream_start`'ta tts_source=local bildirebilir.
            // Ses klonlama zaten `has_voice_clone` ile pro/streamer'a kilitli
            // (PlanRegistry) — yerel mod bu kümenin bir alt kümesi/eşiti olur.
            ("tts.local.plans", "pro,streamer"),
            // Yerel TTS kota teşviki: transcript yolundaki 0.5 çarpanla aynı
            // gerekçe (ElevenLabs karakter başı ücretlendirir, yerelde bu maliyet yok).
            ("tts.local.usage_multiplier", "0.5"),
            // Kredi sistemi: backend'in kendi işlediği (indirimsiz) TTS/ses
            // klonlama saatinin kredi ağırlığı — stt.cloud.credit_weight ile
            // aynı gerekçe, farklı self-hosted TTS modellerinin (ör. ucuz
            // Chatterbox/VibeVoice vs. üst tier Qwen3-TTS/VoxCPM2) GPU
            // maliyeti farklıdır.
            ("tts.cloud.credit_weight", "1.0"),
            // Donanım/watchdog eşikleri — stt.local.* ile aynı alan adları,
            // XTTS-v2'ye özgü değerler. watchdog_p95_ms=2000: roadmap 2.2'nin
            // "metin → klonlanmış ses (2 sn altı)" hedefiyle aynı eşik.
            ("tts.local.min_vram_mb", "6144"),
            ("tts.local.benchmark_rtf_max", "0.5"),
            ("tts.local.watchdog_p95_ms", "2000"),
            ("tts.local.retry_minutes", "5"),
            ("stream.max_bitrate", "128000"),
            ("vad.enabled", "true"),
            // Dağıtım yöntemi → plan erişimi. Virgülle ayrılmış plan listesi.
            // Admin panel bunları değiştirir; yayıncı paneli okuyup kilitler.
            ("dist.service.plans", "free,pro"),
            ("dist.cloud.plans", "pro"),
            // Multistream OAuth (1.6, opsiyonel bağlantılar): sağlayıcı
            // kimlikleri yapılandırılmışsa her yayıncı planı bağlanabilir.
            (
                "integration.redirect_base",
                "http://localhost:8080/api/integrations",
            ),
            // OAuth bağlantı testleri: yayıncı dashboard'undaki "Test connection"
            // aksiyonu kayıtlı access token ile bu yan etkisiz uçlara çıkar.
            (
                "providers.streamlabs.test_url",
                "https://streamlabs.com/api/v1.0/socket/token",
            ),
            (
                "providers.youtube.test_url",
                "https://www.googleapis.com/youtube/v3/channels?part=id&mine=true",
            ),
            // OAuth callback tamamlandığında frontend'e geri yönlendirme tabanı.
            // Boş = geriye dönük uyumlu JSON yanıt (callback endpoint doğrudan
            // çağrılıyormuş gibi). Admin panelden ayarlanınca Production Setup
            // sayfasına 302 redirect ile (?provider=...&connected=1) döner.
            ("integration.frontend_base", ""),
            // Stripe (1.11): plan → Stripe Price ID eşlemesi. Boş = henüz Stripe'ta
            // ürün/fiyat oluşturulmadı; checkout o plan için 503 döner. Admin panelden
            // Stripe Dashboard'da oluşturulan gerçek price_id'lerle doldurulur.
            ("billing.plan.free.stripe_price_id", ""),
            ("billing.plan.starter.stripe_price_id", ""),
            ("billing.plan.pro.stripe_price_id", ""),
            ("billing.plan.streamer.stripe_price_id", ""),
            (
                "billing.success_url",
                "http://localhost:3000/dashboard/billing?checkout=success",
            ),
            (
                "billing.cancel_url",
                "http://localhost:3000/dashboard/billing?checkout=canceled",
            ),
            // Loglama (1.12): asgari seviye admin panelden değişir, restart gerekmez.
            ("log.level", "info"),
            // Overlay fan-out koruması (Faz 2 hazırlığı A1): token başına
            // eşzamanlı abone limiti + yeni bağlanana replay edilen son final
            // altyazı satırı sayısı. Restart gerektirmez.
            ("overlay.max_subscribers", "100"),
            ("overlay.snapshot_lines", "3"),
            // Twitch Extension EBS (Faz 2.4). enabled=false iken tüm
            // /ext/twitch/* ve /api/ext/twitch/* yüzeyi 503 döner — admin
            // panelden açılır, restart gerekmez. Limitler policy'nin zorunlu
            // kıldığı per-channel/per-viewer abuse korumasıdır.
            ("ext.twitch.enabled", "false"),
            ("ext.twitch.default_lang", "en"),
            ("ext.twitch.max_viewers_per_channel", "200"),
            ("ext.twitch.state_rate_per_min", "60"),
            // Sağlayıcı key'i yokken mock'a düşülsün mü? Dev/CI: true.
            // Üretimde false yapılır — key eksikse sessiz mock yerine açık hata.
            ("providers.mock_fallback", "true"),
            // SMTP TLS kipi: starttls (üretim) | none (test/dahili relay).
            ("mail.smtp.tls", "starttls"),
            // Yayıncı login 2FA kipi (1.4): auto (SMTP yapılandırıldıysa kod
            // maillenir, yoksa dev modunda atlanır) | required (SMTP yoksa
            // login 503 — üretim sıkılaştırması) | off.
            ("auth.email_2fa", "auto"),
            // Signup'ta atanan plan — admin panelden değiştirilebilir.
            ("auth.default_plan", "free"),
            // OAuth callback locale bilinemediğinde (state geçersiz/eksik ya da
            // start locale taşımadıysa) kullanılacak varsayılan frontend locale
            // segmenti — redirect `/{locale}/login/oauth-complete` biçiminde
            // kurulur (bkz. auth/oauth_login.rs::callback).
            ("auth.default_locale", "en"),
            // Login/signup brute-force koruması (Faz 1.13): istemci (IP)
            // başına dakikada izin verilen deneme sayısı — pairing::ClaimLimiter
            // ile aynı sabit-pencere deseni (bkz. `rate_limit::RateLimiter`).
            // Şifre denemeleri argon2 maliyetiyle zaten yavaşlar; bu limit
            // dağıtık/çoklu-kimlik brute-force'u önler. Restart gerekmez.
            ("auth.login.rate_limit_per_min", "10"),
            ("auth.signup.rate_limit_per_min", "20"),
            // Google/Twitch login (hesap linkleme dahil) authorize/callback
            // taban URL'i — integration.redirect_base ile aynı desen, ayrı
            // anahtar (bkz. auth/oauth_login.rs modül dokümanı: login kimliği
            // bağlama akışı ayrı bir OAuth registry). Prod'da admin panelden
            // `https://api.strovak.com/api/auth/oauth` yapılır.
            (
                "auth.oauth_redirect_base",
                "http://localhost:8080/api/auth/oauth",
            ),
            // "Founding 5" kapalı beta (Faz 1 ilk test — sadece STT altyazı,
            // tek GPU worker): kapalıyken (varsayılan) herkes normal plan
            // limitleriyle yayın açabilir; admin betayı başlatırken "true"
            // yapar — o andan sonra `ws::mod` StreamStart'ta onaylı bir
            // founding başvurusu (bkz. `founding` modülü) şart koşar.
            ("founding.gate.enabled", "false"),
            // Pricing sayfasında fiyatların üstünü çizip gösterilecek admin-
            // düzenlenebilir promosyon mesajı (`GET /config` public'ten okunur).
            ("promo.founding.enabled", "false"),
            (
                "promo.founding.message",
                "Founding testers stream free during the closed beta. Prices below apply after launch.",
            ),
            // Admin maliyet özeti (Faz 1.14): kullanım saati başına tahmini
            // $ maliyeti — self-hosted GPU STT + DeepL için roadmap'teki
            // kalibrasyon tahminleri (docs/audio-pipeline.md). Admin panelden
            // gerçek Vast.ai/Hetzner $/saat rakamlarıyla güncellenir.
            ("cost.stt.usd_per_hour", "0.05"),
            ("cost.translate.usd_per_hour", "0.04"),
            // Legal doküman HTML içerikleri. Boşsa frontend statik fallback
            // metinlerini gösterir; admin panelden doldurulunca public
            // /legal/{slug} sayfaları bu HTML'i render eder.
            ("legal.terms.html", ""),
            ("legal.terms.version", "2026-07-12"),
            ("legal.privacy.html", ""),
            ("legal.privacy.version", "2026-07-12"),
            ("legal.kvkk.html", ""),
            ("legal.kvkk.version", "2026-07-12"),
            ("legal.cookies.html", ""),
            ("legal.cookies.version", "2026-07-12"),
            ("legal.dpa.html", ""),
            ("legal.dpa.version", "2026-07-12"),
            ("legal.communications.html", ""),
            ("legal.communications.version", "2026-07-12"),
            // Tüzel kişilik bilgileri (Faz 1 — Litvanya şahıs kaydı henüz
            // tamamlanmadı). Boşken public /legal/{slug} sayfaları "kayıt
            // tamamlanınca yayınlanacak" notunu gösterir; admin panelden
            // dolduruldukça sayfalarda gerçek bilgi görünür — restart yok.
            ("company.legal_name", ""),
            ("company.registration_number", ""),
            ("company.registered_address", ""),
            // Dashboard/panel CORS izinli origin'leri (virgülle ayrılmış).
            // Frontend Vercel'de, API VPS'te — tarayıcı istekleri cross-origin.
            // İlk deploy CORS_ALLOWED_ORIGINS env'iyle tohumlanır, sonrası
            // admin panelden restart'sız değiştirilir.
            (
                "cors.allowed_origins",
                "http://localhost:3000,http://127.0.0.1:3000",
            ),
        ];
        let mut map: HashMap<String, String> = defaults
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        // Katalog çalışma zamanında üretiliyor (bkz. yukarısı) — varsayılan
        // olarak tam DeepL katalogu açık, admin panelden daraltılır.
        map.insert(
            "translate.deepl.enabled_targets".to_string(),
            crate::translate_languages::all_codes_csv(),
        );
        ServiceConfig {
            values: RwLock::new(map),
        }
    }

    /// Bir anahtarın değerini okur.
    pub fn get(&self, key: &str) -> Option<String> {
        self.values.read().ok().and_then(|m| {
            m.get(key)
                .map(|value| crate::legal::sanitize_config_value(key, value))
        })
    }

    /// Bir anahtarı günceller (admin toggle). Restart gerekmez.
    pub fn set(&self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let value = value.into();
        if let Ok(mut m) = self.values.write() {
            m.insert(
                key.clone(),
                crate::legal::sanitize_config_value(&key, &value),
            );
        }
    }

    /// Yalnızca seed'de tanımlı (bilinen) bir anahtarı günceller.
    ///
    /// Admin API'sinin gelişigüzel anahtar enjekte etmesini engeller —
    /// şema seed ile sabittir. Anahtar yoksa `false` döner, hiçbir şey yazılmaz.
    pub fn set_existing(&self, key: &str, value: impl Into<String>) -> bool {
        let value = value.into();
        match self.values.write() {
            Ok(mut m) if m.contains_key(key) => {
                m.insert(
                    key.to_string(),
                    crate::legal::sanitize_config_value(key, &value),
                );
                true
            }
            _ => false,
        }
    }

    /// Tüm config'in sıralı bir kopyasını döndürür (deterministik JSON için).
    pub fn snapshot(&self) -> BTreeMap<String, String> {
        self.values
            .read()
            .map(|m| {
                m.iter()
                    .map(|(key, value)| {
                        (key.clone(), crate::legal::sanitize_config_value(key, value))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// `service_config` tablosundan okunan satırları üzerine yazar.
    ///
    /// `set_existing` ile aynı whitelist kuralı geçerli — DB'de kalmış eski/
    /// bilinmeyen bir anahtar sessizce yok sayılır, seed şemasını bozmaz.
    pub fn apply_db_rows(&self, rows: Vec<(String, String)>) {
        for (key, value) in rows {
            self.set_existing(&key, value);
        }
    }
}

/// `service_config` tablosundaki tüm satırları okur (başlangıçta, admin'in
/// önceki oturumlarda kaydettiği override'ları belleğe yüklemek için).
pub async fn load_from_db(pool: &sqlx::PgPool) -> Result<Vec<(String, String)>, sqlx::Error> {
    sqlx::query_as("SELECT key, value FROM service_config")
        .fetch_all(pool)
        .await
}

/// Bir anahtarı `service_config` tablosuna yazar (upsert) — admin panelden
/// `PUT /admin/config` çağrıldığında bellek içi değerle birlikte kalıcı hale
/// gelir, restart'ta kaybolmaz.
pub async fn persist_to_db(pool: &sqlx::PgPool, key: &str, value: &str) -> Result<(), sqlx::Error> {
    let value = crate::legal::sanitize_config_value(key, value);
    sqlx::query(
        "INSERT INTO service_config (key, value) VALUES ($1, $2) \
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()",
    )
    .bind(key)
    .bind(&value)
    .execute(pool)
    .await?;
    Ok(())
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_provider_defaults() {
        let cfg = ServiceConfig::with_defaults();
        assert_eq!(cfg.get("stt.provider").as_deref(), Some("deepgram"));
        assert_eq!(cfg.get("translate.provider").as_deref(), Some("deepl"));
        assert_eq!(cfg.get("tts.provider").as_deref(), Some("disabled"));
    }

    #[test]
    fn seeds_local_stt_defaults() {
        let cfg = ServiceConfig::with_defaults();
        // Lokal STT bulut maliyetini düşürür → varsayılan tüm planlara açık.
        assert_eq!(
            cfg.get("stt.local.plans").as_deref(),
            Some("free,starter,pro,streamer")
        );
        assert_eq!(cfg.get("stt.local.max_chars").as_deref(), Some("500"));
        assert_eq!(
            cfg.get("stt.local.usage_multiplier").as_deref(),
            Some("0.5")
        );
    }

    #[test]
    fn set_overrides_without_restart() {
        let cfg = ServiceConfig::with_defaults();
        cfg.set("stt.provider", "whisper");
        assert_eq!(cfg.get("stt.provider").as_deref(), Some("whisper"));
    }

    #[test]
    fn missing_key_is_none() {
        let cfg = ServiceConfig::with_defaults();
        assert_eq!(cfg.get("nonexistent"), None);
    }

    #[test]
    fn set_existing_updates_known_key() {
        let cfg = ServiceConfig::with_defaults();
        assert!(cfg.set_existing("stt.provider", "whisper"));
        assert_eq!(cfg.get("stt.provider").as_deref(), Some("whisper"));
    }

    #[test]
    fn oauth_login_redirect_base_is_seeded_and_admin_settable() {
        // Regression: this key backs auth/oauth_login.rs's authorize URL
        // construction. If it's missing from the seed, PUT /admin/config
        // 404s on it (set_existing only writes pre-seeded keys) and
        // production can never point Google/Twitch login callbacks at
        // api.strovak.com instead of the localhost:8080 dev default.
        let cfg = ServiceConfig::with_defaults();
        assert_eq!(
            cfg.get("auth.oauth_redirect_base").as_deref(),
            Some("http://localhost:8080/api/auth/oauth")
        );
        assert!(cfg.set_existing(
            "auth.oauth_redirect_base",
            "https://api.strovak.com/api/auth/oauth"
        ));
    }

    #[test]
    fn set_existing_rejects_unknown_key() {
        let cfg = ServiceConfig::with_defaults();
        assert!(!cfg.set_existing("attacker.key", "x"));
        assert_eq!(cfg.get("attacker.key"), None);
    }

    #[test]
    fn apply_db_rows_overrides_known_keys() {
        let cfg = ServiceConfig::with_defaults();
        cfg.apply_db_rows(vec![(
            "legal.terms.html".to_string(),
            "<h1>Terms</h1>".to_string(),
        )]);
        assert_eq!(
            cfg.get("legal.terms.html").as_deref(),
            Some("<h1>Terms</h1>")
        );
    }

    #[test]
    fn apply_db_rows_sanitizes_legacy_legal_html() {
        let cfg = ServiceConfig::with_defaults();
        cfg.apply_db_rows(vec![(
            "legal.privacy.html".to_string(),
            "<h2>Privacy</h2><img src=x onerror=alert(1)><style>body{display:none}</style>"
                .to_string(),
        )]);

        assert_eq!(
            cfg.get("legal.privacy.html").as_deref(),
            Some("<h2>Privacy</h2>")
        );
    }

    #[test]
    fn apply_db_rows_ignores_unknown_keys() {
        let cfg = ServiceConfig::with_defaults();
        cfg.apply_db_rows(vec![("attacker.key".to_string(), "x".to_string())]);
        assert_eq!(cfg.get("attacker.key"), None);
    }

    #[test]
    fn snapshot_is_sorted_and_complete() {
        let cfg = ServiceConfig::with_defaults();
        let snap = cfg.snapshot();
        assert_eq!(
            snap.get("dist.service.plans").map(String::as_str),
            Some("free,pro")
        );
        assert_eq!(
            snap.get("dist.cloud.plans").map(String::as_str),
            Some("pro")
        );
        // BTreeMap garantili sıralı.
        let keys: Vec<&String> = snap.keys().collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted);
    }
}
