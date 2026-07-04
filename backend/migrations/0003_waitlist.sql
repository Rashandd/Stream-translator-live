-- Faz 2/3 "coming soon" kataloğu için minimal lead-capture (bkz.
-- docs/overlay-builder-plan.md §6). Auth gerektirmeyen tek endpoint
-- (POST /api/waitlist) buraya yazar. Aynı kişi aynı özelliğe iki kez
-- kaydolamaz (email + interest unique). Operatör tarafı (admin'den
-- görüntüleme/export) kapsam dışı — bu tablo sadece toplama içindir.

CREATE TABLE waitlist_signups (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email      TEXT NOT NULL,
    interest   TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (email, interest)
);
