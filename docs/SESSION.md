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
Kullanıcının login/signup akışı analizi sonucunda ortaya çıkan browser auth
borçları için uygulama planı tamamlandı:

- Yeni plan: `docs/auth-workflow-hardening-plan.md`.
- Roadmap 2.9 altında plan tiklendi; implementasyon fazları ayrı açık checkbox
  olarak eklendi.
- Bu tur yalnız plan/dokümantasyon kapsamındadır; auth implementasyonu
  değiştirilmedi.

## Son Kararlar
- Tarayıcıdaki password, signup, 2FA ve Google/Twitch OAuth başarıları tek
  HttpOnly cookie session modelinde birleşecek. Bearer JWT yalnız SDK/CLI için
  kalacak.
- OAuth `state`; `intent=login|signup|link`, locale, allowlist `return_to` ve
  signup consent version bağlamını server-side taşıyacak. Login intent'i sessiz
  hesap açmayacak.
- `?auth_error=session|network` güvenlik sırrı değil, fakat URL/history'den
  kaldırılacak; tek kullanımlık `sessionStorage` notice + `location.replace`
  kullanılacak.
- Admin/broadcaster 2FA ortak opaque challenge ile role-aware olacak; admin
  verify stub kaldırılacak.
- Signup ikinci bir login çağrısı yapmayacak; session veya pending challenge'ı
  doğrudan signup yanıtı üretecek.
- Legal ve marketing consent versiyon/timestamp/source ile kalıcı tutulmadan
  yeni hesap tamamlanmış kabul edilmeyecek.
- Frontend/backend ayrı deploy edildiği için legacy OAuth fragment kaldırma işi
  uyumluluk pencereli rollout ile yapılacak.

## Sonraki Adım
- `docs/auth-workflow-hardening-plan.md` Faz 0 ile başla: kaynak-string testleri
  yerine davranış testlerini ve cross-origin browser auth E2E iskeletini ekle.
- Ardından Faz 1 callback locale/opaque-error kontratı ve Faz 2 OAuth cookie
  session taşıması birlikte ele alınmalı.
- Gerçek Google/Twitch ve Vercel/VPS testleri kullanıcı credential/deploy
  erişimi gerektiren son canlı doğrulama kapısıdır.

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
