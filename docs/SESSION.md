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
Yayıncı panelinde "Sign-in accounts" ve "Platforms" sayfaları tek "Account
details" sayfasında (`/dashboard/account`) birleştirildi (yalnızca frontend
kompozisyon değişikliği, backend'e dokunulmadı):

- `LinkedAccountsPanel` ve `PlatformAccountsPanel` değişmeden `SectionCard`
  içinde iki bölüm olarak aynı sayfada; Twitch Extension linking bölümü de
  altına taşındı.
- `/dashboard/platforms` artık `/dashboard/account`'a redirect eden ince bir
  sayfa — eski bookmark/linkler kırılmıyor.
- Nav'daki iki ayrı "Platforms"/"Sign-in accounts" item'ı tek "Account
  details" item'ına indirildi (`UserCog` ikonu eklendi).
- Kullanıcı önce e-posta/telefon/MFA yönetimini de aynı sayfaya istedi; bu üçü
  için backend'de hiçbir altyapı yok (email için `GET /api/auth/me` gibi bir
  uç yok, telefon için hiçbir DB alanı yok, MFA için sadece login-time email
  OTP var — `users.totp_secret_encrypted`/`totp_enabled` kolonları DB'de
  duruyor ama hiçbir Rust kodu dokunmuyor, ölü kolon). Kullanıcıya scope
  soruldu, "Merge UI only" seçildi — email/telefon/MFA backend'i bu turda
  YAPILMADI.
- Doğrulama: `npm run lint` + `npm run test` (267/267 yeşil) + `tsc --noEmit`
  temiz. Görsel doğrulama için AuthGuard bypass edilemedi (CSP nonce'ları
  inline script injection'ı engelliyor, backend koşmadığı için token da
  doğrulanamıyor) — geçici, dashboard layout'ı dışında bir preview route
  (`app/[locale]/preview-account-zz`, commit'lenmedi) ile ekran görüntüsü
  alınıp silindi; layout DESIGN.md kart/pill deseniyle uyumlu render oldu.
- Commit: `efbd750` — "feat(frontend): merge sign-in accounts and platforms
  into one account details page".

## Son Kararlar
- Email/telefon/telefon-doğrulama/gerçek kullanıcı-yönetilebilir MFA (TOTP)
  net yeni backend kapsamı — roadmap'te hiçbir yerde planlanmamış, kullanıcı
  onayı olmadan bu tur eklenmedi.

## Sonraki Adım
- Eğer kullanıcı ileride "email + phone + MFA" tam kapsamını isterse: önce
  `GET /api/auth/me` benzeri bir uç (email döndürsün), sonra telefon alanı +
  SMS sağlayıcı seçimi, sonra gerçek TOTP provisioning (secret/QR/backup
  codes) — üçü de ayrı backend işleri, tek bir PR'da değil.
- Auth workflow hardening planı (`docs/auth-workflow-hardening-plan.md`) hâlâ
  askıda — bu turla ilgisi yok, ayrı iş; onunla ilgili önceki "Neredeyim/Son
  Kararlar" notları git geçmişinde (bu dosyanın önceki hali) korunuyor.

## Blokerlar / Dikkat
- OAuth/2FA state bugün process memory'de; çoklu backend instance'a geçiş
  BH-04/BH-06 ile koordine edilmeli. Yeni state arayüzü Redis implementasyonuna
  açık tasarlanmalı.
- OAuth cookie geçişinde backend ve frontend ayrı deploy edildiği için planın
  uyumluluk sırası atlanmamalı; aksi halde eski frontend token fragment beklerken
  callback sonrası login kırılır.
- `pairing::ClaimLimiter` ve `sdk_session::SdkSessionStore` per-IP/per-process bellek içi — çoklu backend instance'ı (horizontal scale) senaryosunda paylaşılmıyor; MVP tek-instance için yeterli.
- Panelden cihaz "kaldırma" fiziksel cihazın 30 günlük device JWT'sini iptal ETMEZ — sadece canlı görünürlüğü durdurur. Kayıp/çalıntı cihaz senaryosunda tam güvenlik için hâlâ backend'de JWT_SECRET rotasyonu (tüm cihazları etkiler) gerekir.
- Overlay token her stream'de yeniden üretilir, overlay/aktif-stream durumu bellek-içi/tek-instance — `infra/deploy.md` §8d'de detaylı.
- Self-hosted STT worker `Ready` olsa bile `stt.provider` elle `selfhosted`'a çevrilmedikçe backend onu kullanmaz (otomatik aktivasyon implemente değil).
