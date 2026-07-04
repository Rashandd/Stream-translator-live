-- Faz 2 hazırlığı (A2): platform kanal eşleştirmeleri.
-- Bir yayın platformu kanalını (Twitch, ileride Kick vb.) bir StreamDubber
-- hesabına bağlar. Twitch Extension EBS'i channel_id → user çözümlemesini
-- buradan yapar. Şema platform-bağımsızdır; Chrome Extension (roadmap 2.5)
-- aynı tabloyu kullanır. Prensipler 0001 ile aynı: UUID PK, created_at/
-- updated_at, soft delete yok (unbind satırı siler).

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
