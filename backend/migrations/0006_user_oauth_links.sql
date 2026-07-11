-- Bir hesaba, birincil OAuth login kimliği (users.oauth_provider/
-- oauth_external_id) dışında sonradan bağlanan ek sağlayıcılar — bkz.
-- backend/src/users/mod.rs UserStore::link_identity. Dosya deposu
-- (UserStore) asıl kaynak, bu tablo diğerleri gibi best-effort yansıma.
CREATE TABLE user_oauth_links (
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider    TEXT NOT NULL,
    external_id TEXT NOT NULL,
    linked_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, provider)
);

CREATE UNIQUE INDEX idx_user_oauth_links_identity ON user_oauth_links(provider, external_id);
