# Codex Session

## Agent
Codex

## Neredeyim?
Dashboard distribution dili OBS hub ve Strovak managed routing olarak genisletildi.

## Degistirdigim Alanlar
- `frontend/src/i18n/dictionaries/en.json`
- `frontend/src/i18n/dictionaries/marketing.test.ts`
- `frontend/app/[locale]/dashboard/distribution/page.tsx`
- `frontend/app/[locale]/dashboard/go-live/page.tsx`
- `docs/sessions/codex.md`

## Son Kararlar
- Restream/Castr/Streamlabs ayri caption veya multi-dub modu degil; OBS'den cikan yayini tasiyan harici servisler olarak kalmali.
- Distribution kartlari: `Control outputs from OBS` ve `Let Strovak handle routing`.
- OBS modu sadece caption overlay gibi anlatilmiyor; captions now + enabled language/dub outputs icin buyuyebilecek ana yayin hub'i olarak konumlandi.
- Strovak routing modu upload workaround degil; destinations, language outputs, captions ve delivery'nin managed edilmesi olarak anlatiliyor.
- Distribution sayfasindaki connected services bolumu kaldirildi.
- Go-live sayfasinda platform hedef secimi yalnizca Strovak routing/cloud modunda gosteriliyor.
- `marketing.test.ts` distribution modlarinda Restream/Castr/Streamlabs'in geri gelmemesini ve cloud routing'in upload bahanesi gibi anlatilmamasini kontrol ediyor.

## Sonraki Adim
Frontend sonraki adim: roadmap/admin metinlerinde Restream/Castr entegrasyon beklentisini bu yeni urun karariyla uyumlu hale getirmek.

## Blokerlar / Dikkat
- `npm run test` ve `npm run lint` frontend icin gecti.
- `scripts/session-start.sh` backend testlerinde gateway/Vault local socket bind izni nedeniyle sandbox kaynakli basarisizlik gosterdi.
- Calisma agacinda bu turdan once de backend, dashboard ve roadmap dosyalarinda commit edilmemis degisiklikler vardi; bunlara dokunulmadi.
