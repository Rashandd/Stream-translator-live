-- Versioned legal and marketing consent mirror. USERS_FILE remains the
-- encrypted source of truth; this table supports operational queries and
-- audits when PostgreSQL is configured.
CREATE TABLE user_consents (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id          UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind             TEXT NOT NULL CHECK (kind IN ('terms', 'privacy', 'kvkk', 'cookies', 'dpa', 'marketing')),
    document_version TEXT NOT NULL,
    accepted         BOOLEAN NOT NULL,
    recorded_at      TIMESTAMPTZ NOT NULL,
    source           TEXT NOT NULL CHECK (source IN ('password_signup', 'google_signup', 'twitch_signup', 'settings')),
    UNIQUE (user_id, kind, document_version, recorded_at)
);

CREATE INDEX idx_user_consents_user_kind ON user_consents(user_id, kind);
