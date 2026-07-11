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
Kullanıcı isteği: "Vercel frontend + VPS backend + 1 GPU worker ile OBS Browser
Source overlay için ne eksik" analizi + admin panelde "Service routing" sayfasını
sil + "admin girişten ~5sn sonra session expired" regresyonunu bul ve düzelt.
Üçü de bitti:

1. **Service routing sayfası tamamen kaldırıldı** (commit `00a7c90`) — o sayfa
   `GET /config`/`PUT /admin/config`'in ince bir UI katmanıydı, provider
   ayarları artık doğrudan config API'den yapılıyor.
2. **Admin session regresyonu bulundu ve düzeltildi** (commit `fbcb424`,
   `ac2e3ef`) — kök neden koddaki gibi görünenden çok daha büyüktü: bkz.
   "Son Kararlar".
3. **3-makine deploy runbook'u eklendi** (commit `5194d34`,
   `infra/deploy.md` §8) — GPU worker provizyonu, `STT_SERVERS_FILE`
   kalıcılığı, `stt.provider=selfhosted`'a elle geçiş (config API curl
   örneğiyle), bilinen MVP sınırları.

## Son Kararlar
- **Kök neden (c76149d regresyonu):** `finish_principal_login`
  (`backend/src/users/mod.rs:915`) `Origin` header'ı olan (yani HER tarayıcı
  isteği) login'lerde artık bearer token değil, HttpOnly cookie session
  döndürüyordu (`auth::session::create_browser_session`) — ama bu değişiklik
  yapılırken (1) `auth::session::current_session/refresh/logout` handler'ları
  hiç router'a bağlanmamıştı (`app.rs`'te route yoktu), (2) `dashboard_cors`
  `allow_credentials(true)` içermiyordu, (3) frontend'in DÖRT ayrı yerinde
  (`admin-login-form.tsx`, `auth-client.ts`'in `login()`'ı, `verify-code-form.tsx`)
  hâlâ yanıt gövdesinden `data.token` okunup localStorage'a yazılıyordu —
  token alanı artık yoktu, `Bearer undefined` gönderiliyordu. Sonuç: admin
  panel "session expired" gösteriyordu, `/login`'den şifreyle giren HERKES
  (admin + broadcaster) "invalid credentials" görüyordu, 2FA doğrulama da
  aynı şekilde kırıktı. OAuth ile giren broadcaster'lar etkilenmedi (ayrı,
  Origin göndermeyen bir redirect akışı — `oauth_login.rs`, hâlâ bearer
  token kullanıyor, kasıtlı olarak dokunulmadı).
- **Backend fix** (`backend/src/app.rs`): üç route wire edildi
  (`GET /api/auth/session`, `POST /api/auth/refresh`, `POST /api/auth/logout`),
  `dashboard_cors`'a `.allow_credentials(true)` + `x-csrf-token` header'ı
  eklendi. `AuthUser`/`AdminUser` extractor'ları zaten cookie+CSRF
  destekliyordu (`authz.rs`), değişmedi. 4 yeni e2e test
  (`backend/tests/auth_api.rs`) regresyonu kilitliyor: browser login →
  cookie + token'sız gövde, cookie → `/api/auth/session` çalışır, cookie'siz
  401, CSRF header'sız mutasyon 403.
- **Frontend fix**: `adminFetch` artık tamamen cookie tabanlı
  (`credentials:"include"` + `x-csrf-token`, localStorage yok);
  `userFetch` HEM eski OAuth bearer token'ı (varsa öncelikli) HEM cookie
  session'ı destekliyor (broadcaster'lar iki farklı login yolundan gelebilir).
  Ortak `refreshInFlight` promise'ı eşzamanlı 401'lerin refresh cookie'sini
  yarıştırmasını (rotation race) önlüyor. `AuthGuard` artık localStorage
  yerine `GET /api/auth/session`'a bakıyor (`role` de admin konsolunda
  doğrulanıyor). `SignOutLink` artık `POST /api/auth/logout` da tetikliyor.
  `getAdminToken/setAdminToken/clearAdminToken` tamamen kaldırıldı.
- Tüm workspace (`cargo test/clippy --workspace`, frontend
  `test/lint/tsc/build`) her commit'ten önce temiz doğrulandı.
- Bu oturum sırasında Codex (aynı git user) paralel çalışıp
  `docs/twitch-extension-production-readiness-plan.md`'yi ekledi
  (commit `d9fbbfc`, benim işimle çakışmıyor, ayrı dosyalar).

## Sonraki Adım
- **Bloklanan (kullanıcı girdisi gerekiyor):** Gerçek Vercel deploy'u
  (proje link'i, env var'lar, `vercel login`) ve gerçek GPU worker
  provizyonu (SSH erişimi olan bir makine) bu ortamda credential'sız
  yapılamadı — `infra/deploy.md` §1 ve §8'de adım adım hazır, kullanıcıyla
  birlikte veya kullanıcı tarafından çalıştırılmalı.
- Sıradaki agent için: `docs/backend-hardening-captions-plan.md` hâlâ
  "Planlandı, implementasyon başlamadı" — P1/P2 fazına başlamak mantıklı
  bir sonraki Faz 1 işi (bkz. dosyanın kendi §8 uygulama sırası).
- Admin bootstrap 2FA sayfası (`/admin/login/verify`) hâlâ gerçek bir
  backend çağrısı yapmayan bir stub (form action doğrudan `/admin`'e GET
  atıyor) — bilerek dokunulmadı, "development stub" notu zaten dictionary'de
  var; gerçek admin 2FA gerekirse ayrı bir iş.

## Blokerlar / Dikkat
- `pairing::ClaimLimiter` ve `sdk_session::SdkSessionStore` per-IP/per-process bellek içi — çoklu backend instance'ı (horizontal scale) senaryosunda paylaşılmıyor; MVP tek-instance için yeterli.
- Panelden cihaz "kaldırma" fiziksel cihazın 30 günlük device JWT'sini iptal ETMEZ — sadece canlı görünürlüğü durdurur. Kayıp/çalıntı cihaz senaryosunda tam güvenlik için hâlâ backend'de JWT_SECRET rotasyonu (tüm cihazları etkiler) gerekir.
- Overlay token her stream'de yeniden üretilir, overlay/aktif-stream durumu bellek-içi/tek-instance — `infra/deploy.md` §8d'de detaylı.
- Self-hosted STT worker `Ready` olsa bile `stt.provider` elle `selfhosted`'a çevrilmedikçe backend onu kullanmaz (otomatik aktivasyon implemente değil).
