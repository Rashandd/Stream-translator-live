# Aktif Multi-Agent Session Durumu

<!-- Ana koordinasyon dosyasi. Agent bazli session dosyasi kullanilmaz. -->

## Genel Proje Durumu
Repo blueprint/dokumantasyon asamasinda. Backend/frontend/sdk/infra kod klasorleri henuz olusturulmamis.

## Aktif Agentlar
- Codex ve Claude: ayni merkezi session panosunu (`docs/SESSION.md`) okur ve gunceller.

## Cakisma Kurali
- Agentlar calisma sonunda sadece `docs/SESSION.md` dosyasini gunceller.
- Agent bazli session dosyasi kullanilmaz.
- Ayni anda ayni klasor veya dosya uzerinde calisilmadan once `git status --short` kontrol edilir.

## Neredeyim?
Frontend public navbar duzenlendi: Security linki `en.nav.items` listesinden kaldirildi. Desktop header ve mobile menu ayni listeyi kullandigi icin Security iki navigasyondan da cikti; Security sayfasi ve footer linki korunuyor.

## Son Kararlar
- Public header nav ana urun linkleri Product, Solutions ve Pricing olarak kalacak; Security header'da gosterilmeyecek.
- Bu davranis `frontend/src/i18n/dictionaries/public-pages.test.ts` icinde sozlesme testiyle sabitlendi.
- Commit akışı: Testler ve lint temizse anlamlı mesajla commit oluşturulur; `git push` kullanıcı açıkça istemedikçe yapılmaz.
- Session akışı: Codex ve Claude `docs/SESSION.md` üzerinden aynı bağlamı paylaşır; agent bazlı session dosyası kullanılmaz.
- Issue akışı: `docs/commands/fix-issue.md` commit atar, push/PR yalnizca kullanici acikca isterse yapilir.
- Faz 1 kapsamı: Deepgram + DeepL + kendi SDK gömülü RTMP relay'iyle (+ opsiyonel Streamlabs) ücretsiz altyazı; Restream/Castr desteği kaldırıldı. Ses klonlama, Twitch/Chrome extension ve local AI sonraki fazlarda.
- Transit güvenlikte TLS 1.3 + SDK-backend mTLS kullanılacak; ses akışına ayrıca AES eklenmeyecek. AES-256 sadece at-rest veriler, lokal stream key'ler ve ileride ses modelleri için.
- Konfigürasyonlar DB'deki service_config benzeri katmandan okunacak; provider/fallback/plan limitleri koda gömülmeyecek.
- `docs/websocket-protocol.md` mevcut fakat git'e eklenmemiş; SDK-backend protokolü için referans alınmalı.

## Sonraki Adım
Siradaki frontend veya Faz 1 task'i icin ilgili referans dokumani okuyup regression-first devam et.

## Blokerlar / Dikkat
- Roadmap'te bu kucuk navbar istegine denk acik task bulunamadi; tik atilmadi.
- Frontend dogrulama temiz: hedefli Vitest, tum `npm run test` ve `npm run lint` gecti.
- Dokümanlarda küçük tutarsızlık var: roadmap ve `docs/commands/test-pipeline.md` bazı yerlerde ses akışı için AES ifadesi kullanıyor; güncel güvenlik kararı TLS 1.3 + mTLS, AES yalnızca at-rest.
