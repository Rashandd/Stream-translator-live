-- OAuth ile giriş (Google/Twitch) — bkz. backend/src/auth/oauth_login.rs.
-- Şifresiz hesaplar için password_hash artık NULL olabilir; sağlayıcı kimliği
-- (oauth_provider, oauth_external_id) ile kullanıcı eşlenir. Dosya deposu
-- (UserStore) asıl kaynak — bu tablo diğer alanlar gibi best-effort yansıma.
ALTER TABLE users ALTER COLUMN password_hash DROP NOT NULL;
ALTER TABLE users ADD COLUMN oauth_provider TEXT;
ALTER TABLE users ADD COLUMN oauth_external_id TEXT;
CREATE UNIQUE INDEX idx_users_oauth_identity ON users(oauth_provider, oauth_external_id)
    WHERE oauth_provider IS NOT NULL;
