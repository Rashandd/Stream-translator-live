-- Browser refresh sessions. Raw refresh tokens never enter PostgreSQL;
-- token_hash is SHA-256 over a 256-bit random value. A replayed rotated token
-- revokes every row sharing the same family_id.
CREATE TABLE browser_sessions (
    token_hash  TEXT PRIMARY KEY,
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
