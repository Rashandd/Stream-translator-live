---
segment: altyazi-ceviri
plan: starter | pro
dashboard_routes:
  - /dashboard/captions
api_endpoints:
  - GET /api/translate/catalog
  - GET/PUT /api/translate/languages
  - GET /api/overlay/token
---

# Segment 2 — Altyazı + Çeviri

## Kim için

Uluslararası kitleye ulaşmak isteyen, izleyicisinin **kendi dilinde**
altyazı görmesini isteyen yayıncı.

**Plan:** Starter (3 eşzamanlı hedef dil) veya Pro (10 eşzamanlı hedef dil).
Bkz. `README.md` plan matrisi.

## Ön koşullar

[`01-sadece-altyazi.md`](./01-sadece-altyazi.md)'deki ön koşullar + hangi
hedef dillere ihtiyacın olduğuna dair bir karar (izleyici kitlenin
konuştuğu diller).

## Akış

Önce ortak adımları ([`00-ortak-adimlar.md`](./00-ortak-adimlar.md) O-01→O-04)
ve Segment 1'deki ADIM A-01/A-02'yi (konuşma dili + overlay görünümü)
tamamla.

---

### ADIM C-01 — Hedef dilleri seçme `[HAZIR]`

**Ne yapıyorsun:** Altyazının hangi dillere çevrileceğini seçersin.
**Nerede:** `/dashboard/captions` → hedef dil seçici.
**Nasıl:** Çeviri kataloğundaki **herhangi bir dili** çoklu-seçimle
işaretle — katalog DeepL (birincil) + Google (fallback) destekli ~30 dili
kapsar, güncel liste `GET /api/translate/catalog`'tan gelir. Planının izin
verdiği **eşzamanlı** sayıyı aşarsan seçim reddedilir.
**Arka planda:** `GET/PUT /api/translate/languages`.
**Sorun olursa:** `error 4008` (`target_langs` planın `max_languages`
sınırını aşıyor) → dil sayısını azalt veya plan yükselt; `error 4009`
(katalogda olmayan/admin'in kapattığı bir kod) → güncel kataloğu tekrar çek.
Bazı dillerde DeepL'in "formal/informal üslup" desteği yoktur — seçim
ekranında bu dillerde uyarı görürsün, kalite beklentini buna göre ayarla.

---

### ADIM C-02 — Dil başına overlay URL `[HAZIR]`

**Ne yapıyorsun:** Her hedef dil için ayrı bir overlay URL'i üretirsin.
**Nerede:** `/dashboard/captions` → Overlay Builder.
**Nasıl:** Aynı token'ı kullanıp yalnız `?lang=` parametresini değiştirerek
dil başına bir URL üret (ör. izleyicilerine birden fazla dil seçeneği
sunmak istiyorsan, her biri ayrı bir Browser Source/paylaşılan link olarak).
**Arka planda:** `GET /api/overlay/token` → `/o/{token}?lang={kod}`.
**Sorun olursa:** Kaynak dilin kendisini (çevirisiz) göstermek istersen
`lang` parametresini boş bırak.

---

### ADIM C-03 — İzleyicinin kendi dilini seçmesi `[YAKINDA]`

**Ne yapıyorsun:** İzleyicinin, senin sabit bir dil seçmene gerek kalmadan
kendi dilini seçmesini sağlarsın.
**Nerede:** Platforma göre değişir — bkz.
[`05-platform-notlari.md`](./05-platform-notlari.md).
**Nasıl:** Bugün için platforma özel durum farklı: Twitch'te backend/viewer
arayüzü kodlanmış ama Twitch'in kendi incelemesi bekleniyor; YouTube'da
native CC istemcisi canlı yayına henüz bağlı değil; Kick'te henüz
başlanmadı. O zamana kadar ADIM C-02'deki dil-başına URL yöntemiyle devam
edilir.
**Arka planda:** Twitch Extension için `POST /api/ext/twitch/code`,
`GET /ext/twitch/captions/{channel_id}`.
**Sorun olursa:** Detay ve güncel durum için `05-platform-notlari.md`.

---

### ADIM C-04 — Yayına çık

[`00-ortak-adimlar.md`](./00-ortak-adimlar.md) ADIM O-05 → O-07.

## Platforma özel notlar

Bkz. [`05-platform-notlari.md`](./05-platform-notlari.md) — özet: Twitch
Extension `[YAKINDA]` (yalnız Twitch review kaldı), YouTube native CC
`[YAKINDA]` (kod var, canlı döngüye bağlı değil), Kick Chrome Extension
`[PLANLANDI]`.

## Yükseltme yolu

- **3'ten fazla eşzamanlı dil** istersen → Pro (10 dil).
- **Kendi sesinle başka dilde "konuşmak"** (dublaj) istersen →
  [`03-dublaj-ses-klonlama.md`](./03-dublaj-ses-klonlama.md).

## Sorun giderme

| Belirti | Aksiyon |
|---|---|
| Dil seçimi reddediliyor (`4008`) | Eşzamanlı hedef dil sayını azalt veya plan yükselt |
| Dil kodu reddediliyor (`4009`) | `GET /api/translate/catalog`'tan güncel listeyi çek |
| Çeviri kalitesi düşük görünüyor | DeepL desteklemiyorsa Google fallback'e düşmüş olabilir; dilin `supports_formality` durumunu kontrol et |

## İleri seviye

Kaynak dilin (konuşma dilin) STT'sini kendi cihazında çalıştırmak — çeviri
adımını etkilemez, ayrı ve en son düşünülecek bir seçenektir. Bkz.
[`00-ortak-adimlar.md`](./00-ortak-adimlar.md) son bölüm, "İleri Seviye:
Kendi Cihazında STT".
