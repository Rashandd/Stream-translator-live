-- BH-03C SDK sessions: PostgreSQL becomes the durable presence owner.
-- The HMAC key is an authentication secret, therefore it is encrypted by the
-- application before storage and never returned from device-list endpoints.
ALTER TABLE sdk_sessions
    ADD COLUMN IF NOT EXISTS heartbeat_secret_encrypted TEXT;

CREATE INDEX IF NOT EXISTS idx_sdk_sessions_active_user_seen
    ON sdk_sessions (user_id, last_seen_at DESC)
    WHERE is_active;
