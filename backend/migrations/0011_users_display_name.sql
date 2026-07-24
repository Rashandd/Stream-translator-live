-- OAuth login profilinden çekilen görünen ad (Google `name`, Twitch
-- `display_name`). Şifreli signup'ta ve bu kolon eklenmeden önceki eski
-- kayıtlarda NULL kalır. Dosya deposu (UserStore) asıl kaynak — bu kolon
-- diğer alanlar gibi best-effort yansıma (bkz. backend/src/auth/oauth_login.rs).
ALTER TABLE users ADD COLUMN display_name TEXT;
