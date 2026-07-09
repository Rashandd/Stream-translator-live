# Aktif Multi-Agent Session Durumu

<!-- Ana koordinasyon dosyasi. Agent bazli session dosyasi kullanilmaz. -->

## Genel Proje Durumu
Backend (Rust/Axum) + Frontend (Next.js) + SDK (Tauri/Rust) kod klasorleri mevcut ve calisir durumda. Faz 1 kapsaminin buyuk bolumu tamamlandi (bkz. `streamdubber_roadmap_final.md`).

## Aktif Agentlar
- Codex ve Claude: ayni merkezi session panosunu (`docs/SESSION.md`) okur ve gunceller.

## Cakisma Kurali
- Agentlar calisma sonunda sadece `docs/SESSION.md` dosyasini gunceller.
- Agent bazli session dosyasi kullanilmaz.
- Ayni anda ayni klasor veya dosya uzerinde calisilmadan once `git status --short` kontrol edilir.

## Neredeyim?
SDK ↔ hesap pairing workflow'unun güvenliği iki aşamada sertleştirildi:
1. `POST /api/pairing/claim` (auth'suz, kod capability) rate-limit'siz brute-force'a açıktı → `ClaimLimiter` eklendi.
2. Frontend'de eşleşmiş masaüstü uygulamanın gerçekten bağlı olup olmadığını gösteren hiçbir yüzey yoktu ("başka cihazla karışma" riski yoktu ama görünürlük de yoktu) → yeni `sdk_session` modülü: imzalı (HMAC-SHA256 + timestamp toleransı, Stripe webhook doğrulamasıyla birebir aynı şema) heartbeat + panelde canlı "bağlı cihazlar" listesi + kaldırma.

## Son Kararlar
- `backend/src/pairing/mod.rs::ClaimLimiter` — per-IP, dakikada 10 deneme, `waitlist::WaitlistLimiter` ile aynı desen.
- `backend/src/sdk_session/mod.rs` (yeni modül): `SdkSessionStore` — pairing claim'de `heartbeat_secret` (32 byte, hex) üretir, SDK'ya BİR KEZ döner (`ClaimResponse.sdk_session_id`/`heartbeat_secret`). `POST /api/sdk/heartbeat`: `{session_id, ts, sig}` — `sig=HMAC-SHA256(secret,"{id}.{ts}")`, ±60sn tolerans, `hmac::Mac::verify_slice` (sabit zamanlı). `GET /api/sdk/devices` (panel, kendi cihazları + `connected` bayrağı) + `DELETE /api/sdk/devices/{id}` (kaldırma — device JWT'yi iptal etmez, sadece heartbeat/liste'yi durdurur, NOT bu bilinen bir kapsam sınırı). Kalıcılık `RELAY_FILE` ile aynı desen: `SDK_SESSIONS_FILE` (AES-256-GCM), `last_seen_at` diske yazılmaz (sık I/O'dan kaçınmak için, zararsız).
- SDK: `core::heartbeat` (imzalama + 20sn'lik loop, `stt_control::run_polling_loop` ile aynı fire-and-forget desen), `LocalSession` yeni alanlar `#[serde(default)]` ile eski format'ı bozmuyor.
- Frontend: `sdk-devices-panel.tsx` — `/dashboard/sdk`'da yeni "Connected devices" kartı, 15sn poll, bağlı/çevrimdışı rozet, kaldır butonu. Tek dictionary `en.json` (locales=["en"] tek dil).
- Testler: backend 9 unit + 4 e2e (`tests/sdk_session_api.rs`), SDK 2 unit (`core::heartbeat`), frontend 3 yeni wiring testi. Tüm workspace (`cargo test/clippy --workspace`, `npm run test/lint`, `tsc --noEmit`, `npm run build`) temiz.
- Roadmap'e V9 altına iki tarihli not eklendi (satır ~798-799).
- Commit akışı: Testler ve lint temizse anlamlı mesajla commit oluşturulur; `git push` kullanıcı açıkça istemedikçe yapılmaz.
- Faz 1 kapsamı: Deepgram + DeepL + kendi SDK gömülü RTMP relay'iyle (+ opsiyonel Streamlabs) ücretsiz altyazı; Restream/Castr desteği kaldırıldı. Ses klonlama, Twitch/Chrome extension ve local AI sonraki fazlarda.
- Transit güvenlikte TLS 1.3 + SDK-backend mTLS kullanılacak (backend/src/tls.rs — WebPkiClientVerifier ile kurulu, `TLS_CLIENT_CA_PATH` verilirse zorunlu); ses akışına ayrıca AES eklenmeyecek. AES-256 sadece at-rest veriler (pairing token, stream key, sdk_session heartbeat secret, ses örnekleri) için.
- Konfigürasyonlar DB'deki service_config benzeri katmandan okunacak; provider/fallback/plan limitleri koda gömülmeyecek.

## Sonraki Adım
Panelden "kaldır" ile device JWT'nin kendisinin de iptal edilmesi (tam revocation — relay targets GET / WS auth gibi diğer SDK-authenticated uçların da session store'a bakması) bilinçli olarak kapsam dışı bırakıldı; istenirse ayrı bir iş olarak ele alınabilir. Bunun dışında bir sonraki agent yeni bir Faz 1 task'i icin ilgili referans dokumani okuyup regression-first devam etsin.

## Blokerlar / Dikkat
- `pairing::ClaimLimiter` ve `sdk_session::SdkSessionStore` per-IP/per-process bellek içi — çoklu backend instance'ı (horizontal scale) senaryosunda paylaşılmıyor; MVP tek-instance için yeterli.
- Panelden cihaz "kaldırma" fiziksel cihazın 30 günlük device JWT'sini iptal ETMEZ (yukarıdaki not) — sadece canlı görünürlüğü durdurur. Kayıp/çalıntı cihaz senaryosunda tam güvenlik için hâlâ backend'de JWT_SECRET rotasyonu (tüm cihazları etkiler) gerekir.
- Dokümanlarda küçük tutarsızlık var: roadmap ve `docs/commands/test-pipeline.md` bazı yerlerde ses akışı için AES ifadesi kullanıyor; güncel güvenlik kararı TLS 1.3 + mTLS, AES yalnızca at-rest.
