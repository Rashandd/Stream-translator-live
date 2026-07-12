# PostgreSQL Şeması — Faz 1/2 (güncellendi 2026-07-12)

## Prensipler
- UUID primary key (çoğu tabloda — `browser_sessions`/`user_oauth_links` gibi istisnalar kendi doğal anahtarını kullanır, aşağıda belirtilir)
- `created_at` / `updated_at` çoğu tabloda, otomatik
- Şifreli kolonlar `_encrypted` suffix'iyle işaretlenir — uygulama katmanında AES-256 ile şifrelenir, DB düz metin görmez
- Soft delete yok — GDPR silme talebi gelince satır gerçekten silinir, audit log'a düşer

---

## Kayıt Deposu (Store of Record) — Nerede Ne Saklanır

**Önemli:** Postgres bu proje için tek doğruluk kaynağı DEĞİL. Kullanıcı/secret/
relay/usage/voice gibi verilerin asıl kayıt deposu şifreli **dosyalardır**
(`*_FILE` env değişkenleri, `backend/src/main.rs`); Postgres varsa yalnızca
best-effort bir aynadır — DB kaybolursa dosya store'lar hâlâ çalışır, ama tersi
doğru değildir (DB `pg_dump`'ı yedeklemek `/data` volume'unu yedeklemenin
yerine geçmez).

| Varlık | Asıl kayıt deposu | Postgres tablosu (varsa) |
|---|---|---|
| Yayıncı/admin hesapları | `USERS_FILE` (AES-256-GCM) | `users` (best-effort yansıma) |
| Admin panel secret'ları | `SECRETS_FILE` (AES-256-GCM) | — (Postgres'e hiç yazılmaz) |
| OAuth token'ları (Streamlabs/YouTube/Twitch) | `TOKENS_FILE` (AES-256-GCM) | — |
| SDK relay hedefleri (stream key) | `RELAY_FILE` (AES-256-GCM) | — |
| Eşleşmiş cihaz presence/heartbeat | `SDK_SESSIONS_FILE` (AES-256-GCM) | `sdk_sessions` (best-effort yansıma) |
| Kullanım sayaçları | `USAGE_FILE` (JSONL) | `usage_records` (dakika-bazlı upsert, `DATABASE_URL` varsa) |
| Merkezi loglar | `LOG_FILE` (JSONL) | — |
| Yayıncı hedef dil seçimi | `STREAM_SETTINGS_FILE` (düz JSON) | — |
| Ses klonlama model kayıtları | `VOICE_MODELS_FILE` (düz JSON) | — |
| Ham ses örneği | `VOICE_SAMPLES_FILE` (AES-256-GCM) | — |
| Local STT/TTS modu + benchmark | `STT_SETTINGS_FILE` / `TTS_SETTINGS_FILE` (düz JSON) | — |
| Self-hosted GPU STT sunucuları | `STT_SERVERS_FILE` (AES-256-GCM) | — |
| Founding-beta başvuruları | `FOUNDING_APPS_FILE` (düz JSON) | — |
| Kanal eşleştirmeleri (Twitch/Kick) | Postgres | `channel_bindings` (DB varsa açılışta belleğe yüklenir) |
| Browser refresh session'ları | Postgres (tek kaynak) | `browser_sessions` |
| Runtime config (`service_config`) | Postgres (DB varsa) veya bellek içi seed | `service_config` |
| Plan tanımları | Postgres (DB varsa) veya kod-içi seed | `plans` |

`*_FILE` env'i boş bırakılan her varlık bellek içi çalışır — backend restart'ında
sıfırlanır (startup'ta `tracing::warn!` ile bildirilir). Production deploy
kontrol listesi için `infra/deploy.md`'deki env tablosuna bak.

---

## Tablolar

### `plans`
```sql
CREATE TABLE plans (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name            TEXT NOT NULL UNIQUE,          -- 'free', 'starter', 'pro', 'streamer', 'founding'
    price_cents     INTEGER NOT NULL,              -- 0, 999, 2499, 9999 (kredi kararı 2026-07-09)
    monthly_hours   INTEGER NOT NULL,              -- 5, 30, 80, 200  (-1 = sınırsız)
    max_languages   INTEGER NOT NULL,              -- 1, 3, 10, -1
    max_platforms   INTEGER NOT NULL,              -- 1, 3, -1, -1
    has_watermark   BOOLEAN NOT NULL DEFAULT TRUE,
    has_voice_clone BOOLEAN NOT NULL DEFAULT FALSE,
    stripe_price_id TEXT,
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

Fiyat ve limitler koda gömülmez — kod-içi seed `backend/src/plans/mod.rs`'te
(`PlanRegistry::with_defaults`), DB varsa açılışta önceki admin düzenlemeleri
seed'in üzerine yazılır (`PUT /admin/plans` ile restart'sız değiştirilir).
Güncel seed: free $0/5sa, starter $9.99/30sa, pro $24.99/80sa, streamer
$99.99/200sa, founding $0/50sa (kapalı beta — bkz. roadmap "Founding 5").

---

### `users`
```sql
CREATE TABLE users (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email             TEXT NOT NULL UNIQUE,
    password_hash     TEXT,                        -- argon2id; OAuth-only hesaplarda NULL (0004)
    plan_id           UUID NOT NULL REFERENCES plans(id),
    role              TEXT NOT NULL DEFAULT 'broadcaster', -- 'broadcaster' | 'admin'
    email_verified    BOOLEAN NOT NULL DEFAULT FALSE,
    totp_secret_encrypted TEXT,                  -- 2FA secret (AES-256 şifreli)
    totp_enabled      BOOLEAN NOT NULL DEFAULT FALSE,
    stripe_customer_id TEXT,
    oauth_provider    TEXT,                       -- 'google' | 'twitch' | NULL (0004)
    oauth_external_id TEXT,                        -- sağlayıcının kullanıcı ID'si (0004)
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_users_email ON users(email);
CREATE UNIQUE INDEX idx_users_oauth_identity ON users(oauth_provider, oauth_external_id)
    WHERE oauth_provider IS NOT NULL;
```

Asıl kaynak `USERS_FILE` (bkz. yukarıdaki "Kayıt Deposu" tablosu) — bu satırlar
best-effort yansıma. `password_hash` 0004 migration'ından sonra NULL olabilir
(şifresiz OAuth-only hesap); o durumda `oauth_provider`/`oauth_external_id`
doludur.

---

### `subscriptions`
```sql
CREATE TABLE subscriptions (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id                 UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    plan_id                 UUID NOT NULL REFERENCES plans(id),
    stripe_subscription_id  TEXT UNIQUE,
    status                  TEXT NOT NULL,   -- 'active' | 'past_due' | 'canceled' | 'trialing'
    current_period_start    TIMESTAMPTZ NOT NULL,
    current_period_end      TIMESTAMPTZ NOT NULL,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_subscriptions_user ON subscriptions(user_id);
```

Stripe webhook handler'ı (`backend/src/billing/persist.rs`) bu tabloyu
`checkout.session.completed`/`customer.subscription.*` event'lerinde upsert eder.

---

### `sdk_sessions`
```sql
CREATE TABLE sdk_sessions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_name     TEXT,                        -- "Windows PC", "Mac Studio"
    pairing_code    TEXT UNIQUE,                 -- 6 haneli kod, 10 dakika geçerli
    paired_at       TIMESTAMPTZ,
    last_seen_at    TIMESTAMPTZ,
    sdk_version     TEXT,
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_sdk_sessions_user ON sdk_sessions(user_id);
CREATE INDEX idx_sdk_sessions_pairing ON sdk_sessions(pairing_code) WHERE pairing_code IS NOT NULL;
```

Web dashboard'dan pairing code gösterilir → SDK kodu girer → `paired_at` dolar,
`pairing_code` NULL'lanır. Asıl kaynak `SDK_SESSIONS_FILE` — bu tablo canlı
presence/heartbeat'in best-effort yansımasıdır (`backend/src/sdk_session`).

---

### `streams`
```sql
CREATE TABLE streams (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id             UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    sdk_session_id      UUID REFERENCES sdk_sessions(id),
    status              TEXT NOT NULL DEFAULT 'idle', -- 'idle' | 'active' | 'ended' | 'error'
    source_lang         TEXT NOT NULL,               -- 'tr', 'en', ...
    target_langs        TEXT[] NOT NULL DEFAULT '{}', -- ['en', 'de']
    distribution_mode   TEXT NOT NULL DEFAULT 'external', -- 'external' (Mod A) | 'cloud' (Mod C)
    started_at          TIMESTAMPTZ,
    ended_at            TIMESTAMPTZ,
    duration_seconds    INTEGER,                     -- ended_at - started_at (hesaplanmış)
    error_message       TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_streams_user ON streams(user_id);
CREATE INDEX idx_streams_status ON streams(status) WHERE status = 'active';
```

**Not:** Bu tablo şema olarak planlı ama aktif yayın durumu bugün Postgres'te
değil, backend'in bellek içi `ActiveStreams`'inde tutuluyor (tek-instance,
restart'a dayanıksız — bkz. `docs/backend-hardening-captions-plan.md` BH-03/06).

---

### `usage_records`
```sql
CREATE TABLE usage_records (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    stream_id       UUID REFERENCES streams(id),
    period          TEXT NOT NULL,               -- '2026-07' (YYYY-MM)
    seconds_used    INTEGER NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (user_id, period)
);

CREATE INDEX idx_usage_user_period ON usage_records(user_id, period);
```

Asıl kaynak `USAGE_FILE` (JSONL, boot'ta geri yüklenir); `DATABASE_URL` varsa
ayrıca bu tabloya dakika sınırlarında write-behind upsert edilir (fire-and-forget,
FK ihlali pipeline'ı düşürmez).

Aylık limit kontrolü: `SELECT seconds_used FROM usage_records WHERE user_id = $1 AND period = $2`

---

### `platform_accounts`
```sql
CREATE TABLE platform_accounts (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id                 UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    platform                TEXT NOT NULL,     -- 'streamlabs' | 'youtube' | 'twitch' | 'kick'
    platform_user_id        TEXT,
    platform_username       TEXT,
    access_token_encrypted  TEXT NOT NULL,     -- AES-256
    refresh_token_encrypted TEXT,              -- AES-256
    token_expires_at        TIMESTAMPTZ,
    scopes                  TEXT[],
    is_active               BOOLEAN NOT NULL DEFAULT TRUE,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (user_id, platform)
);

CREATE INDEX idx_platform_accounts_user ON platform_accounts(user_id);
```

**Not:** Bu tablo şema olarak planlı; bugünkü OAuth token deposu `TOKENS_FILE`
(`backend/src/integrations/token_store.rs::TokenVault`, AES-256-GCM tek blob) —
`platform_accounts`'a henüz yazılmıyor. Kick platformu backend'de hiç OAuth
entegrasyonuna sahip değil (PKCE gerektiriyor, bilinçli kapsam dışı).

---

### `service_config`
```sql
CREATE TABLE service_config (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    description TEXT,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_by  UUID REFERENCES users(id)
);
```

Kod-içi seed `backend/src/config.rs`'te (`ServiceConfig::with_defaults`) —
`stt.provider`, `stt.fallback`, `translate.provider`, `tts.provider`,
`cors.allowed_origins`, `founding.gate.enabled`, `legal.*.html` gibi ~40 anahtar.
DB varsa açılışta önceki admin düzenlemeleri seed'in üzerine yazılır. Kod her
zaman `config::get("stt.provider")` ile okur — değer değişince restart gerekmez.

---

### `channel_bindings` *(migration 0002, Faz 2 hazırlığı)*
```sql
CREATE TABLE channel_bindings (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id             UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    platform            TEXT NOT NULL,             -- 'twitch' | 'kick' | ...
    platform_channel_id TEXT NOT NULL,
    status              TEXT NOT NULL DEFAULT 'active',
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (platform, platform_channel_id)
);

CREATE INDEX idx_channel_bindings_user ON channel_bindings(user_id);
```

Twitch Extension EBS'in `channel_id → user` çözümlemesi buradan yapılır
(`backend/src/bindings`); Kick/Chrome Extension (Faz 2.5) aynı tabloyu
platform-bağımsız olarak kullanacak şekilde tasarlandı. DB varsa açılışta
belleğe yüklenir (`ChannelBindingStore`).

---

### `waitlist_signups` *(migration 0003)*
```sql
CREATE TABLE waitlist_signups (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email      TEXT NOT NULL,
    interest   TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (email, interest)
);
```

"Coming soon" katalog sayfalarındaki (`/solutions`, ses klonlama vb.)
`POST /api/waitlist` lead-capture uçlarının deposu. Auth gerektirmez. Admin
tarafı (görüntüleme/export) kapsam dışı — yalnız toplama içindir.

---

### `browser_sessions` *(migration 0005)*
```sql
CREATE TABLE browser_sessions (
    token_hash  TEXT PRIMARY KEY,                 -- SHA-256(refresh token) — düz token asla saklanmaz
    family_id   UUID NOT NULL,
    subject     TEXT NOT NULL,
    role        TEXT NOT NULL CHECK (role IN ('broadcaster', 'admin')),
    plan        TEXT NOT NULL,
    status      TEXT NOT NULL CHECK (status IN ('active', 'rotated', 'revoked')),
    expires_at  TIMESTAMPTZ NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    rotated_at  TIMESTAMPTZ,
    revoked_at  TIMESTAMPTZ
);

CREATE INDEX idx_browser_sessions_family ON browser_sessions(family_id);
CREATE INDEX idx_browser_sessions_expiry ON browser_sessions(expires_at);
```

HttpOnly cookie tabanlı dashboard session'larının refresh-token rotasyonu
(`backend/src/auth/session.rs`). Bu tablo Postgres'te **tek kaynak** — dosya
store karşılığı yok; `DATABASE_URL` yoksa bellek içi çalışır (restart'ta tüm
oturumlar düşer). Rotasyonlu bir token replay edilirse aynı `family_id`'deki
tüm satırlar iptal edilir.

---

### `user_oauth_links` *(migration 0006)*
```sql
CREATE TABLE user_oauth_links (
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider    TEXT NOT NULL,
    external_id TEXT NOT NULL,
    linked_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, provider)
);

CREATE UNIQUE INDEX idx_user_oauth_links_identity ON user_oauth_links(provider, external_id);
```

Bir hesaba, `users.oauth_provider`/`oauth_external_id`'deki birincil login
kimliği DIŞINDA sonradan bağlanan ek sağlayıcılar (`/dashboard/account` —
"ikinci sağlayıcı bağla" akışı). Asıl kaynak yine `USERS_FILE`'daki
`UserStore::link_identity`; bu tablo best-effort yansıma.

---

### `auth_tokens` *(şemada var, kod tarafından KULLANILMIYOR)*
```sql
CREATE TABLE auth_tokens (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash  TEXT NOT NULL UNIQUE,    -- SHA-256 hash, düz token saklanmaz
    type        TEXT NOT NULL,           -- 'refresh' | 'email_verify' | 'password_reset'
    expires_at  TIMESTAMPTZ NOT NULL,
    used_at     TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_auth_tokens_hash ON auth_tokens(token_hash);
CREATE INDEX idx_auth_tokens_user ON auth_tokens(user_id);
```

**Bu tablonun `backend/migrations/`'ta karşılığı YOK ve `backend/src`'te sıfır
referansı var** — orijinal Faz 1 tasarımından kalma, hiç implemente edilmedi.
Refresh-token rotasyonu bunun yerine `browser_sessions`'ta yapılıyor;
email-verify/password-reset akışları kendi bellek içi store'larını kullanıyor
(`users::TwoFactorStore`). Bu tabloyu ya gerçekten implemente et ya da bu
dosyadan sil — şu haliyle "gelecek plan" notu olarak duruyor.

---

### `audit_logs`
```sql
CREATE TABLE audit_logs (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID REFERENCES users(id) ON DELETE SET NULL,
    action      TEXT NOT NULL,     -- 'login' | 'stream_start' | 'plan_change' | 'key_used' | ...
    resource    TEXT,              -- 'stream:uuid' | 'config:stt.provider' | ...
    metadata    JSONB,             -- ek detay (IP, değişiklik öncesi/sonrası)
    ip_address  INET,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_audit_logs_user ON audit_logs(user_id);
CREATE INDEX idx_audit_logs_action ON audit_logs(action);
CREATE INDEX idx_audit_logs_created ON audit_logs(created_at DESC);
```

**Not:** Bugünkü secret-değişikliği ve admin-login audit izleri bu tabloya
değil, merkezi log deposuna (`LOG_FILE` / `Logger`, `backend/src/logging`)
yazılıyor — `redact()` ile key/token/password değerleri `[REDACTED]`'e çevrilir.
Bu tablo Postgres tabanlı ayrı bir audit trail için planlı, henüz bağlanmadı.

---

## İlişki Özeti

```
plans ──< subscriptions >── users ──< sdk_sessions
                              │
                              ├──< streams
                              ├──< usage_records
                              ├──< platform_accounts
                              ├──< channel_bindings
                              ├──< user_oauth_links
                              └──< audit_logs (planlı, henüz bağlanmadı)

browser_sessions   (users.id'ye FK değil — subject TEXT olarak tutulur)
waitlist_signups   (kullanıcıdan bağımsız, anonim lead-capture)
service_config     (kullanıcıdan bağımsız, global)
```

---

## Migration Stratejisi

- Her migration ayrı dosya: `backend/migrations/0001_init.sql`, `0002_channel_bindings.sql`, ...
  (bugüne kadar: 0001–0006, bkz. yukarıdaki tablo listesi)
- Tool: `sqlx::migrate!("./migrations")` — backend açılışında (`backend/src/db.rs::run_migrations`)
  otomatik uygulanır, derleme zamanında binary'ye gömülür (ayrıca dosya kopyalamaya gerek yok)
- Prod'da migration otomatik çalışır, geri alınamaz — her migration forward-only yaz
- Kolon silme: önce uygulama kodu güncelle, sonra kolonu düşür (ayrı migration)
- `DATABASE_URL` set edilmezse migration hiç çalışmaz, backend dosya-tabanlı store'larla
  (yukarıdaki "Kayıt Deposu" tablosu) tam işlevsel çalışmaya devam eder
