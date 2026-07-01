-- StreamDubber Faz 1 — başlangıç şeması.
-- Bkz. docs/schema.md. Prensipler: UUID PK, created_at/updated_at her tabloda,
-- şifreli kolonlar _encrypted suffix'i (uygulama katmanında AES-256), soft delete yok.

-- plans -----------------------------------------------------------------------
CREATE TABLE plans (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name            TEXT NOT NULL UNIQUE,          -- 'free', 'starter', 'pro', 'streamer'
    price_cents     INTEGER NOT NULL,
    monthly_hours   INTEGER NOT NULL,              -- -1 = sınırsız
    max_languages   INTEGER NOT NULL,
    max_platforms   INTEGER NOT NULL,
    has_watermark   BOOLEAN NOT NULL DEFAULT TRUE,
    has_voice_clone BOOLEAN NOT NULL DEFAULT FALSE,
    stripe_price_id TEXT,
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- users -----------------------------------------------------------------------
CREATE TABLE users (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email                 TEXT NOT NULL UNIQUE,
    password_hash         TEXT NOT NULL,
    plan_id               UUID NOT NULL REFERENCES plans(id),
    role                  TEXT NOT NULL DEFAULT 'broadcaster',  -- 'broadcaster' | 'admin'
    email_verified        BOOLEAN NOT NULL DEFAULT FALSE,
    totp_secret_encrypted TEXT,
    totp_enabled          BOOLEAN NOT NULL DEFAULT FALSE,
    stripe_customer_id    TEXT,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_users_email ON users(email);

-- subscriptions ---------------------------------------------------------------
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

-- sdk_sessions ----------------------------------------------------------------
CREATE TABLE sdk_sessions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_name     TEXT,
    pairing_code    TEXT UNIQUE,
    paired_at       TIMESTAMPTZ,
    last_seen_at    TIMESTAMPTZ,
    sdk_version     TEXT,
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_sdk_sessions_user ON sdk_sessions(user_id);
CREATE INDEX idx_sdk_sessions_pairing ON sdk_sessions(pairing_code) WHERE pairing_code IS NOT NULL;

-- streams ---------------------------------------------------------------------
CREATE TABLE streams (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id             UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    sdk_session_id      UUID REFERENCES sdk_sessions(id),
    status              TEXT NOT NULL DEFAULT 'idle',      -- 'idle' | 'active' | 'ended' | 'error'
    source_lang         TEXT NOT NULL,
    target_langs        TEXT[] NOT NULL DEFAULT '{}',
    distribution_mode   TEXT NOT NULL DEFAULT 'external',  -- 'external' (Mod A) | 'cloud' (Mod C)
    started_at          TIMESTAMPTZ,
    ended_at            TIMESTAMPTZ,
    duration_seconds    INTEGER,
    error_message       TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_streams_user ON streams(user_id);
CREATE INDEX idx_streams_status ON streams(status) WHERE status = 'active';

-- usage_records ---------------------------------------------------------------
CREATE TABLE usage_records (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    stream_id       UUID REFERENCES streams(id),
    period          TEXT NOT NULL,               -- 'YYYY-MM'
    seconds_used    INTEGER NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (user_id, period)
);

CREATE INDEX idx_usage_user_period ON usage_records(user_id, period);

-- platform_accounts -----------------------------------------------------------
CREATE TABLE platform_accounts (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id                 UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    platform                TEXT NOT NULL,   -- 'streamlabs' | 'youtube' | 'twitch' | 'kick'
    platform_user_id        TEXT,
    platform_username       TEXT,
    access_token_encrypted  TEXT NOT NULL,
    refresh_token_encrypted TEXT,
    token_expires_at        TIMESTAMPTZ,
    scopes                  TEXT[],
    is_active               BOOLEAN NOT NULL DEFAULT TRUE,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (user_id, platform)
);

CREATE INDEX idx_platform_accounts_user ON platform_accounts(user_id);

-- service_config --------------------------------------------------------------
CREATE TABLE service_config (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    description TEXT,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_by  UUID REFERENCES users(id)
);

INSERT INTO service_config (key, value, description) VALUES
    ('stt.provider',        'deepgram',    'STT sağlayıcı: deepgram | assemblyai | whisper'),
    ('stt.fallback',        'assemblyai',  'STT fallback sağlayıcı'),
    ('translate.provider',  'deepl',       'Çeviri sağlayıcı: deepl | google'),
    ('translate.fallback',  'google',      'Çeviri fallback'),
    ('tts.provider',        'disabled',    'TTS sağlayıcı (Faz 2): elevenlabs | cartesia | xtts'),
    ('stream.max_bitrate',  '128000',      'Maksimum ses bitrate (bps)'),
    ('vad.enabled',         'true',        'VAD aktif mi');

-- auth_tokens -----------------------------------------------------------------
CREATE TABLE auth_tokens (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash  TEXT NOT NULL UNIQUE,    -- SHA-256 hash
    type        TEXT NOT NULL,           -- 'refresh' | 'email_verify' | 'password_reset'
    expires_at  TIMESTAMPTZ NOT NULL,
    used_at     TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_auth_tokens_hash ON auth_tokens(token_hash);
CREATE INDEX idx_auth_tokens_user ON auth_tokens(user_id);

-- audit_logs ------------------------------------------------------------------
CREATE TABLE audit_logs (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID REFERENCES users(id) ON DELETE SET NULL,
    action      TEXT NOT NULL,
    resource    TEXT,
    metadata    JSONB,
    ip_address  INET,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_audit_logs_user ON audit_logs(user_id);
CREATE INDEX idx_audit_logs_action ON audit_logs(action);
CREATE INDEX idx_audit_logs_created ON audit_logs(created_at DESC);
