-- BH-03B: PostgreSQL is the durable owner for production domain state.
-- This migration is forward-only.  Seed rows deliberately use DO NOTHING so
-- an administrator's existing plan edits are never overwritten at startup.

-- Canonical plan catalogue ----------------------------------------------------
INSERT INTO plans (name, price_cents, monthly_hours, max_languages, max_platforms, has_watermark, has_voice_clone)
VALUES
    ('free', 0, 5, 1, 1, TRUE, FALSE),
    ('starter', 999, 30, 3, 3, FALSE, FALSE),
    ('pro', 2499, 80, 10, -1, FALSE, TRUE),
    ('streamer', 9999, 200, -1, -1, FALSE, TRUE),
    ('founding', 0, 50, 3, 3, FALSE, FALSE)
ON CONFLICT (name) DO NOTHING;

-- User/security fields -------------------------------------------------------
-- `users` already owns the core account fields.  These fields complete the
-- durable account profile without moving short-lived challenges into Postgres.
ALTER TABLE users ADD COLUMN IF NOT EXISTS last_login_at TIMESTAMPTZ;
ALTER TABLE users ADD COLUMN IF NOT EXISTS password_changed_at TIMESTAMPTZ;
ALTER TABLE users ADD COLUMN IF NOT EXISTS disabled_at TIMESTAMPTZ;
CREATE INDEX IF NOT EXISTS idx_users_plan_id ON users(plan_id);
CREATE INDEX IF NOT EXISTS idx_users_active_email ON users(email) WHERE disabled_at IS NULL;

-- Relay targets --------------------------------------------------------------
CREATE TABLE relay_targets (
    id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id              UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    platform             TEXT NOT NULL,
    rtmp_url             TEXT NOT NULL,
    stream_key_encrypted TEXT NOT NULL,
    dub_language         TEXT,
    position             SMALLINT NOT NULL DEFAULT 0 CHECK (position >= 0),
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (user_id, platform)
);
CREATE INDEX idx_relay_targets_user_position ON relay_targets(user_id, position);

-- Per-user stream / STT / TTS settings --------------------------------------
CREATE TABLE user_stream_settings (
    user_id      UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    source_lang  TEXT NOT NULL DEFAULT 'auto',
    target_langs TEXT[] NOT NULL DEFAULT '{}',
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE user_stt_settings (
    user_id       UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    mode          TEXT NOT NULL DEFAULT 'cloud' CHECK (mode IN ('cloud', 'local')),
    benchmark     JSONB,
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE user_tts_settings (
    user_id       UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    settings      JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Founding beta applications -------------------------------------------------
CREATE TABLE founding_applications (
    user_id                UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    email                  TEXT NOT NULL,
    platform               TEXT NOT NULL,
    channel_url            TEXT NOT NULL,
    message                TEXT NOT NULL DEFAULT '',
    status                 TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved', 'rejected')),
    auto_created_via_oauth BOOLEAN NOT NULL DEFAULT FALSE,
    created_at             TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at             TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_founding_applications_status_created ON founding_applications(status, created_at DESC);

-- Voice model metadata and encrypted sample references ----------------------
CREATE TABLE voice_models (
    id                    TEXT PRIMARY KEY,
    user_id               UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name                  TEXT NOT NULL,
    provider_voice_id     TEXT NOT NULL,
    status                TEXT NOT NULL,
    language_mode         TEXT NOT NULL,
    source_languages      TEXT[] NOT NULL DEFAULT '{}',
    target_languages      TEXT[] NOT NULL DEFAULT '{}',
    mode                  TEXT NOT NULL DEFAULT 'dubbing',
    source                TEXT NOT NULL DEFAULT 'user_cloned',
    has_stored_sample     BOOLEAN NOT NULL DEFAULT FALSE,
    consent               JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_voice_models_user_created ON voice_models(user_id, created_at);
CREATE INDEX idx_voice_models_status ON voice_models(status);

CREATE TABLE voice_sample_references (
    voice_model_id            TEXT PRIMARY KEY REFERENCES voice_models(id) ON DELETE CASCADE,
    user_id                   UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    sample_encrypted          TEXT NOT NULL,
    content_type              TEXT,
    byte_length               INTEGER NOT NULL CHECK (byte_length >= 0),
    created_at                TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_voice_sample_references_user ON voice_sample_references(user_id);

-- Self-hosted STT server metadata -------------------------------------------
CREATE TABLE stt_servers (
    id                            TEXT PRIMARY KEY,
    name                          TEXT NOT NULL UNIQUE,
    hostname                      TEXT NOT NULL,
    ssh_port                      INTEGER NOT NULL CHECK (ssh_port BETWEEN 1 AND 65535),
    ssh_user                      TEXT NOT NULL,
    ssh_public_key                TEXT NOT NULL,
    ssh_private_key_encrypted     TEXT NOT NULL,
    host_fingerprint              TEXT,
    public_endpoint_override      TEXT,
    status                        TEXT NOT NULL,
    status_detail                 TEXT,
    endpoint_url                  TEXT,
    api_token_encrypted           TEXT,
    gpu_name                      TEXT,
    vram_mb                       BIGINT CHECK (vram_mb >= 0),
    last_health_check_at          TIMESTAMPTZ,
    created_at                    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_stt_servers_status ON stt_servers(status);

-- Persistent overlay credential.  Live overlay membership/snapshots belong
-- to Redis (BH-04/BH-06), never to this table.
CREATE TABLE overlay_tokens (
    user_id    UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    token      TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
