# Browser Auth Workflow Hardening Plan

**Durum:** Faz 1–2 tamamlandı (locale-safe/opaque OAuth callback + ortak
HttpOnly cookie session); Faz 3 opaque 2FA challenge devralındı ve uygulanıyor,
Faz 4–6 bekliyor  
**Tarih:** 2026-07-12 (Faz 1–2: 2026-07-13)  
**Kapsam:** Frontend login/signup, Google/Twitch OAuth callback, browser session,
email 2FA, consent ve logout

## 1. Amaç

Tarayıcıdaki bütün kimlik doğrulama yollarını tek ve doğrulanabilir bir session
modelinde birleştirmek:

```text
password login ─┐
signup + 2FA ───┼─> HttpOnly browser session ─> GET /api/auth/session ─> role-aware UI
Google/Twitch ──┘
```

Bearer JWT desteği yalnız SDK, CLI ve server-to-server istemciler için kalır.
Browser token'ı URL fragment'ına veya `localStorage`'a yazılmaz.

Plan şu doğrulanmış sorunları kapatır:

1. `AuthGuard` başarısızlığının `?auth_error=session|network` ile URL'ye
   yazılması ve mesajın geçmiş/yenileme boyunca kalması.
2. OAuth callback'in locale içermeyen `/login/oauth-complete` ve
   `/dashboard/account` yollarına dönmesi; frontend gerçek yollarının
   `/[locale]/...` altında olması.
3. Şifre login'inin cookie, OAuth login'inin URL fragment + `localStorage`
   bearer kullanması; eski/süresi dolmuş bearer'ın geçerli cookie session'ın
   önüne geçmesi.
4. OAuth bearer'ının doğrulanmadan dashboard guard'ını geçmesi.
5. Admin 2FA'nın `/admin/login` tarafından işlenmemesi ve mevcut admin verify
   sayfasının backend çağrısı yapmayan stub olması.
6. Signup'ın hesap oluşturma ve login için iki ayrı istek kullanması.
7. Legal/marketing consent'in kalıcı, versiyonlu bir kayıt olarak tutulmaması
   ve OAuth ile otomatik hesap açmanın signup consent akışını atlaması.
8. Logout isteğinin beklenmeden navigasyon yapılması nedeniyle refresh
   session'ın sunucuda açık kalabilmesi.
9. OAuth callback'in ham provider/backend hata metnini query string'e koyması.

## 2. Mimari kararlar

### A-01 — Browser için tek session tipi

- Password, signup, 2FA ve OAuth başarıları
  `auth::session::create_browser_session` üzerinden aynı cookie setini üretir.
- `sd_access`: kısa ömürlü, HttpOnly access JWT.
- `sd_refresh`: dönen/replay korumalı, HttpOnly refresh token.
- `sd_csrf`: double-submit CSRF değeri.
- `userFetch` ve `adminFetch` aynı cookie-session çekirdeğini kullanır.
- `Authorization: Bearer` browser akışından kaldırılır; backend extractor'ında
  SDK/CLI uyumluluğu için kalır.

### A-02 — OAuth callback token taşımaz

- Başarılı callback cookie session kurar ve yalnız allowlist edilmiş frontend
  yoluna 302/303 döner.
- URL'de token, e-posta, challenge veya ham provider hatası bulunmaz.
- Callback sonucu yalnız sabit kodlardan biri olabilir:
  `success`, `account_not_found`, `email_exists`, `invalid_state`,
  `provider_unavailable`, `oauth_failed`.
- Kullanıcıya gösterilen metin frontend dictionary/config'inden gelir.

### A-03 — OAuth niyeti açıkça taşınır

OAuth `state` kaydı aşağıdaki server-side bağlamı taşır:

```text
provider
intent = login | signup | link
locale
return_to
legal_terms_version?   # yalnız signup
issued_at / expires_at
existing_user_id?      # yalnız link
```

- Login sayfasındaki OAuth butonu bilinmeyen kimlik için sessizce hesap açmaz;
  `account_not_found` döndürür ve signup'a yönlendirir.
- Signup sayfasındaki OAuth butonu ancak güncel legal metin sürümüyle hesap
  oluşturabilir.
- `return_to` yalnız aynı origin'de, locale altındaki izinli relative route
  olabilir. Mutlak URL ve `//host` biçimi reddedilir.
- Linkleme mevcut authenticated kullanıcıya bağlı kalır; login/signup ile
  karışmaz.

### A-04 — 2FA challenge kimliğe bağlı ve role-aware olur

- Frontend `sessionStorage` içinde e-posta saklamaz.
- Login/signup backend'i kısa ömürlü, opaque bir `challenge_id` üretir; tercih
  edilen taşıma HttpOnly `sd_auth_challenge` cookie'sidir.
- Challenge; subject, normalize e-posta, rol, amaç, expiry ve attempt sayısını
  server-side tutar.
- Verify yanıtındaki rol yönlendirmeyi belirler:
  broadcaster → `/[locale]/dashboard`, admin → `/[locale]/admin`.
- Admin ve broadcaster aynı gerçek verify bileşenini/endpoint'ini kullanır.
- Resend ayrı, rate-limit'li ve gerçek bir endpoint olur.

### A-05 — Auth bildirimi URL'den çıkarılır

- Guard, kullanıcıya gösterilecek tek kullanımlık kodu frontend origin'inin
  `sessionStorage` alanına yazar.
- `window.location.replace(loginHref)` kullanılır; geri tuşuyla korumalı
  sayfa/login döngüsü oluşturulmaz.
- Login sayfasındaki küçük client `AuthNotice` bileşeni kodu bir kez okuyup
  hemen siler.
- Doğrudan dashboard'a giden hiç-login-olmamış kullanıcı için `sign_in_required`,
  mevcut session işareti/başarısız refresh bulunan kullanıcı için
  `session_expired`, API erişilemiyorsa `session_unavailable` ayrılır.

### A-06 — Consent kanıtı kalıcı ve versiyonlu olur

Her kabul/değişiklik en az şu alanlarla saklanır:

```text
user_id
kind = terms | privacy | kvkk | cookies | dpa | marketing
document_version
accepted
recorded_at
source = password_signup | google_signup | twitch_signup | settings
```

- Legal kabul server timestamp'iyle ve gösterilen doküman versiyonuyla yazılır.
- Marketing consent ayrı ve opsiyoneldir; sessizce yutulmaz.
- Mevcut dosya-tabanlı çalışma modu korunacaksa consent kayıtları şifreli
  kullanıcı store snapshot'ında doğruluk kaynağı olarak kalıcı tutulur;
  PostgreSQL migration'ı mevcut mimariye uygun sorgulanabilir aynayı sağlar.
- Consent yazılamazsa yeni hesap başarıyla tamamlanmış sayılmaz.
- Consent ledger'ın tek doğruluk kaynağı bu plandır; hesap silme akışındaki
  retention/anonymization kararı backend-hardening planının BH-12'sinde ele
  alınır ve ledger'ın modelini yeniden tanımlamaz.

### A-07 — Signup tek auth sonucuyla biter

`POST /api/auth/signup` başarılı olduğunda ikinci bir login isteği gerekmez:

- 2FA kapalıysa browser session kurar.
- 2FA açıksa pending challenge döndürür.
- SMTP/challenge geçici olarak kullanılamıyorsa hesap durumunu açıkça bildirir;
  frontend bunu “network error” diye maskelemez.
- Aynı e-posta ile yarım kalan doğrulamayı güvenli biçimde devam ettirecek
  idempotent resend/recovery davranışı tanımlanır.

### A-08 — Logout tamamlanmadan çıkış gösterilmez

- Link'in varsayılan navigasyonu engellenir.
- CSRF korumalı `POST /api/auth/logout` beklenir.
- Başarıda legacy `sd.user.token` temizlenir ve `location.replace(loginHref)`
  çalışır.
- Ağ hatasında “çıkış tamamlanamadı” gösterilir; kullanıcı yanlış biçimde
  çıkmış sayılmaz.
- Çoklu sekme senkronu için `BroadcastChannel` veya `storage` olayı kullanılır.

## 3. Uygulama fazları

### Faz 0 — Davranış sözleşmesini testlerle kilitle

Önce başarısız testler eklenir; implementasyon bu testleri geçirir.

Backend:

- OAuth callback'in cookie üretmesi ve URL'de token bulunmaması.
- Locale/return path doğrulaması ve open-redirect reddi.
- Login intent'inin yeni hesap açmaması; signup intent'inin consent olmadan
  hesap açmaması.
- OAuth/signup sonrası cookie ile `/api/auth/session` erişimi.
- Admin ve broadcaster 2FA role sonucu.
- Logout sonrası refresh token'ın yeniden kullanılamaması.
- Callback hata yanıtının ham `reqwest`, provider body veya e-posta içermemesi.

Frontend:

- Kaynak metni arayan testler yerine Vitest + DOM davranış testleri.
- Stale legacy bearer varken cookie session'ın çalışması.
- Guard'ın her browser auth türünde `/api/auth/session` doğrulaması yapması.
- Auth notice'ın URL query üretmeden bir kere gösterilip silinmesi.
- Admin 2FA'nın admin'e, broadcaster 2FA'nın dashboard'a yönlenmesi.
- Logout'un istek tamamlanmadan navigasyon yapmaması.

E2E:

- Ayrı origin'lerde çalışan frontend (`localhost:3000`) ve backend
  (`localhost:8080`) ile Playwright auth smoke paketi eklenir.
- Cookie/CORS/CSRF davranışı gerçek browser'da doğrulanır; yalnız mock fetch
  yeterli kabul edilmez.

### Faz 1 — Callback route ve hata kontratını düzelt

1. Auth OAuth state'ini `intent`, `locale`, allowlist `return_to` taşıyacak
   biçimde genişlet.
2. Login/signup/link butonlarının doğru intent ve locale ile start endpoint'ine
   gitmesini sağla.
3. Backend redirect'lerini `/[locale]/login/oauth-complete` ve
   `/[locale]/dashboard/account` gerçek route'larıyla eşleştir.
4. Ham `?error=<message>` yerine sabit hata kodu kullan.
5. `integration.frontend_base` yalnız origin olarak normalize/validate edilsin;
   path birleştirme tek yardımcı fonksiyonda yapılsın.

**Çıkış kriteri:** Mock-provider callback'i hiçbir locale için 404 üretmez,
open redirect mümkün değildir ve URL/loglarda teknik hata ayrıntısı yoktur.

### Faz 2 — OAuth'u cookie session'a taşı

1. `OAuthOutcome::LoggedIn(String)` token sonucu yerine session principal/user
   sonucu döndürür.
2. Callback `create_browser_session` ile Set-Cookie başlıklarını üretip redirect
   yanıtına taşır.
3. OAuth tamamlanma sayfası token okumaz; cookie session'ı doğrulayıp role göre
   devam eder.
4. `AuthGuard` local token kısa yolunu kaldırır ve daima session endpoint'ini
   doğrular.
5. `userFetch` bearer önceliğini kaldırıp browser'da yalnız cookie kullanır.

**Çıkış kriteri:** Google/Twitch mock login sonrasında `localStorage` ve URL'de
JWT yoktur; refresh rotation password ve OAuth girişlerinde aynı çalışır.

### Faz 3 — Password signup ve 2FA'yı birleştir

1. Signup handler'ı session/challenge üreten ortak auth completion servisini
   kullanır; frontend'deki ikinci `login()` çağrısı silinir.
2. Opaque challenge store/cookie eklenir; email tabanlı frontend pending state
   kaldırılır.
3. Admin verify stub gerçek ortak verify formuyla değiştirilir.
4. Login, signup ve verify sonuçları ortak typed response şemasına geçirilir.
5. 429, 409, 422, 502 ve 503 durumları kullanıcıya doğru, ayrı mesajlarla
   gösterilir.

**Çıkış kriteri:** Password signup tek submit ile session veya 2FA challenge'a
ulaşır; admin 2FA production modunda çalışır.

### Faz 4 — Consent ledger

1. Consent veri modeli, şifreli file snapshot uyumluluğu ve PostgreSQL migration
   eklenir.
2. Güncel legal doküman version anahtarları runtime config'ten okunur;
   backend-hardening planının BH-02 `ConfigSpec` registry'si mevcutsa key'ler o
   formatta (tip + validator + visibility) tanımlanır (bkz.
   `docs/backend-hardening-captions-plan.md` §13).
3. Password ve OAuth signup aynı consent servisini kullanır.
4. Marketing seçimi kaydedilir; daha sonra settings'ten değiştirilebilir.
5. Audit log yalnız consent türü/version/zaman/source içerir; IP/e-posta gibi
   alanlar mevcut privacy politikasına göre minimize edilir.

**Çıkış kriteri:** Her yeni hesabın versiyonlu zorunlu consent kaydı vardır;
marketing tercihi okunabilir ve değiştirilebilir.

### Faz 5 — Guard bildirimi ve güvenilir logout

1. `auth_error` query üretimi ve login page `searchParams` bağımlılığı kaldırılır.
2. Tek-kullanımlık `AuthNotice` eklenir; `replace` ile yönlendirilir.
3. Logout await edilir, başarısızlık UI'ı ve multi-tab senkronu eklenir.
4. Geçişten kalan `sd.user.token`, `getUserToken`, `setUserToken`,
   `clearUserToken` ve ilgili testler tamamen kaldırılır.

**Çıkış kriteri:** Login URL'sinde auth/session nedeni bulunmaz; logout sonrası
eski refresh token ile session yenilenemez.

### Faz 6 — Canlı doğrulama ve doküman kapatma

- Gerçek Google hesabıyla login ve signup ayrı ayrı denenir.
- Gerçek Twitch hesabıyla login ve signup ayrı ayrı denenir.
- Password signup/login; 2FA off/auto/required matrisinde denenir.
- Admin 2FA required modu denenir.
- 15 dakikalık access expiry ve refresh rotation gerçek zaman/clock kontrollü
  test edilir.
- Vercel production domain + VPS API domain üzerinde cookie, CORS, CSRF ve
  logout doğrulanır.
- `docs/security.md`, `docs/schema.md`, `docs/dashboard.md`,
  `docs/production-operator-guide.md`, roadmap ve `docs/SESSION.md`
  güncellenir.

## 4. Kesintisiz deploy sırası

Frontend ve backend ayrı deploy edildiği için tek committe protokol kırılmaz:

1. **Uyumlu frontend:** OAuth-complete hem mevcut fragment yanıtını hem yeni
   cookie-session başarı yanıtını anlayabilir; guard legacy token'ı körlemesine
   kabul etmez.
2. **Uyumlu backend:** OAuth cookie session kurar. Gerekirse kısa süreli
   `auth.oauth_legacy_fragment` flag'iyle eski frontend için fragment da üretir;
   varsayılan production hedefi `false` olur.
3. **Yeni frontend zorunlu cookie:** Fragment/localStorage yazımı kaldırılır.
4. **Backend cleanup:** Legacy fragment flag'i ve token üretim kolu silinir.
5. **Legacy client cleanup:** İlk yeni frontend açılışında eski
   `sd.user.token` bir kez temizlenir.

Legacy mod bir release penceresinden uzun tutulmaz ve kaldırılma tarihi aynı
deploy notunda yazılır.

## 5. Dosya/değişiklik haritası

Backend ana dosyaları:

- `backend/src/auth/oauth_login.rs`
- `backend/src/auth/session.rs`
- `backend/src/auth/authz.rs`
- `backend/src/users/mod.rs`
- `backend/src/integrations/oauth.rs` veya auth'a özel yeni state store
- `backend/src/app.rs`
- `backend/src/config.rs`
- `backend/migrations/*_auth_consent.sql`
- `backend/tests/auth_api.rs`
- yeni `backend/tests/oauth_browser_session_api.rs`

Frontend ana dosyaları:

- `frontend/src/lib/api.ts`
- `frontend/src/lib/auth-client.ts`
- `frontend/src/components/auth-guard.tsx`
- `frontend/src/components/oauth-buttons.tsx`
- `frontend/src/components/oauth-complete.tsx`
- `frontend/src/components/login-form.tsx`
- `frontend/src/components/signup-form.tsx`
- `frontend/src/components/verify-code-form.tsx`
- `frontend/src/components/admin-login-form.tsx`
- `frontend/src/components/sign-out-link.tsx`
- `frontend/app/[locale]/login/**`
- `frontend/app/[locale]/admin/login/**`

## 6. Test matrisi

| Akış | Beklenen sonuç |
|---|---|
| Password login, 2FA off | Cookie session + role yönlendirmesi |
| Password login, 2FA required | Opaque challenge; doğrulama sonrası cookie |
| Admin login, 2FA required | Verify sonrası yalnız admin console |
| Password signup | Tek request zinciri; session veya challenge |
| OAuth login, bilinen hesap | Cookie; yeni hesap yok |
| OAuth login, bilinmeyen hesap | `account_not_found`; signup önerisi |
| OAuth signup, consent var | Hesap + consent + cookie |
| OAuth signup, consent yok/eski | Hesap oluşturulmaz |
| OAuth email çakışması | Otomatik link yok; sabit güvenli hata |
| Account link callback | Aynı kullanıcıya link; locale-aware dönüş |
| Access token expired | Tek refresh rotation ve başarılı retry |
| Refresh replay | Tüm family revoke |
| Stale legacy local token | Cookie'yi gölgelemez; token temizlenir |
| Logout | Revoke tamamlanır, sonra navigation |
| API network error | Tek kullanımlık notice; URL temiz |
| Kötü `return_to` | 400/fallback; external redirect yok |

## 7. Tamamlanma kapısı

Bir faz ancak aşağıdakiler sağlanınca roadmap'te tamamlandı işaretlenir:

```bash
cargo test
cargo clippy --workspace --all-targets
cd frontend && npm run test
cd frontend && npm run lint
cd frontend && npx tsc --noEmit
cd frontend && npm run build
```

Auth/session kritik path coverage hedefi en az `%80`'dir. Kaynak dosyada belirli
string arayan testler coverage sayılmaz; handler/component davranışı ve gerçek
browser cookie akışı test edilmelidir.

Canlı Google/Twitch, Vercel/VPS cookie ve admin 2FA doğrulamaları tamamlanmadan
bu çalışma “production-ready” sayılmaz.

## 8. Kapsam dışı

- Kick OAuth 2.1 + PKCE.
- SDK/device JWT protokolünün değiştirilmesi.
- Şifreli hesapla OAuth e-posta çakışmasını otomatik bağlama. Mevcut explicit
  settings linkleme akışı korunur; otomatik linkleme yapılmaz.
- Passkey/WebAuthn.
- Çoklu backend instance için OAuth/2FA state'in Redis'e taşınması; bu konu
  backend hardening BH-04/BH-06 ile koordine edilir. Ancak bu planın state
  arayüzü daha sonra Redis implementasyonuna geçirilebilir olmalıdır.
  (BH-04'ün Redis taşıma listesine bu planın OAuth-login state'i ve 2FA challenge
  store'u 2026-07-12'de açıkça eklendi — bkz.
  `docs/backend-hardening-captions-plan.md` §15.)

## 9. Önerilen atomik commit sırası

```text
test: cover browser auth workflow regressions
fix: make oauth callback locale-safe and opaque
feat: issue browser sessions from oauth callbacks
fix: unify browser auth fetch and guard behavior
feat: make signup and 2fa role-aware
feat: persist versioned auth consent
fix: consume auth notices without query parameters
fix: await and synchronize browser logout
test: add cross-origin browser auth e2e coverage
docs: document unified browser authentication
```
