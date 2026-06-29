# PostgreSQL Şeması — Faz 1

## Prensipler
- UUID primary key (tüm tablolarda)
- `created_at` / `updated_at` her tabloda, otomatik
- Şifreli kolonlar `_encrypted` suffix'iyle işaretlenir — uygulama katmanında AES-256 ile şifrelenir, DB düz metin görmez
- Soft delete yok — GDPR silme talebi gelince satır gerçekten silinir, audit log'a düşer

---

## Tablolar

### `plans`
```sql
CREATE TABLE plans (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name            TEXT NOT NULL UNIQUE,          -- 'free', 'starter', 'pro', 'streamer'
    price_cents     INTEGER NOT NULL,              -- 0, 999, 2499, 4999
    monthly_hours   INTEGER NOT NULL,              -- 5, 20, 40, 100  (-1 = sınırsız)
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

Fiyat ve limitler buradan okunur — koda gömülmez.

---

### `users`
```sql
CREATE TABLE users (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email             TEXT NOT NULL UNIQUE,
    password_hash     TEXT NOT NULL,              -- bcrypt/argon2
    plan_id           UUID NOT NULL REFERENCES plans(id),
    role              TEXT NOT NULL DEFAULT 'broadcaster', -- 'broadcaster' | 'admin'
    email_verified    BOOLEAN NOT NULL DEFAULT FALSE,
    totp_secret_encrypted TEXT,                  -- 2FA secret (AES-256 şifreli)
    totp_enabled      BOOLEAN NOT NULL DEFAULT FALSE,
    stripe_customer_id TEXT,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_users_email ON users(email);
```

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

Web dashboard'dan pairing code gösterilir → SDK kodu girer → `paired_at` dolar, `pairing_code` NULL'lanır.

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

---

### `usage_records`
```sql
CREATE TABLE usage_records (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    stream_id       UUID REFERENCES streams(id),
    period          TEXT NOT NULL,               -- '2026-06' (YYYY-MM)
    seconds_used    INTEGER NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (user_id, period)
);

CREATE INDEX idx_usage_user_period ON usage_records(user_id, period);
```

Aylık limit kontrolü: `SELECT seconds_used FROM usage_records WHERE user_id = $1 AND period = $2`

---

### `platform_accounts`
```sql
CREATE TABLE platform_accounts (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id                 UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    platform                TEXT NOT NULL,     -- 'restream' | 'castr' | 'streamlabs' | 'youtube' | 'twitch' | 'kick'
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

-- Başlangıç değerleri
INSERT INTO service_config (key, value, description) VALUES
    ('stt.provider',        'deepgram',    'STT sağlayıcı: deepgram | assemblyai | whisper'),
    ('stt.fallback',        'assemblyai',  'STT fallback sağlayıcı'),
    ('translate.provider',  'deepl',       'Çeviri sağlayıcı: deepl | google'),
    ('translate.fallback',  'google',      'Çeviri fallback'),
    ('tts.provider',        'disabled',    'TTS sağlayıcı (Faz 2): elevenlabs | cartesia | xtts'),
    ('stream.max_bitrate',  '128000',      'Maksimum ses bitrate (bps)'),
    ('vad.enabled',         'true',        'VAD aktif mi');
```

Kod her zaman `config::get("stt.provider")` ile okur — değer değişince restart gerekmez.

---

### `auth_tokens`
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

JWT access token DB'de saklanmaz (stateless). Sadece refresh token ve tek kullanımlık tokenlar burada.

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

Vault key audit logu ayrı (Vault kendi tutar). Bu tablo uygulama seviyesi olaylar için.

---

## İlişki Özeti

```
plans ──< subscriptions >── users ──< sdk_sessions
                              │
                              ├──< streams
                              ├──< usage_records
                              ├──< platform_accounts
                              ├──< auth_tokens
                              └──< audit_logs

service_config  (kullanıcıdan bağımsız, global)
```

---

## Migration Stratejisi

- Her migration ayrı dosya: `migrations/0001_init.sql`, `0002_add_sdk_sessions.sql`, ...
- Tool: `sqlx migrate` (Rust backend ile entegre)
- Prod'da migration otomatik çalışır, geri alınamaz — her migration forward-only yaz
- Kolon silme: önce uygulama kodu güncelle, sonra kolonu düşür (ayrı migration)
