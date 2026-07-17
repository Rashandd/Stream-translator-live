---
segment: dublaj-ses-klonlama
plan: pro | streamer
dashboard_routes:
  - /dashboard/voice
  - /dashboard/distribution
api_endpoints:
  - POST /api/voice/clone, GET /api/voice/models
  - GET /api/tts/policy, PUT /api/tts/mode, POST /api/tts/benchmark
  - PUT/GET /api/relay/targets
---

# Segment 3 — Dublaj / Ses Klonlama

## Kim için

Kendi sesiyle **başka bir dilde "konuşmak"** isteyen yayıncı — izleyici,
yayıncının klonlanmış sesiyle dublajlı bir yayın izler.

**Plan:** Pro (dublör/ucuz-tier TTS ile ses klonlama) veya Streamer
(orijinal ses klonlama, üst-tier TTS). Bkz. `README.md` plan matrisi.

## Ön koşullar

- [`01`](./01-sadece-altyazi.md) ve [`02`](./02-altyazi-ceviri.md)'deki ön koşullar.
- **İkinci bir platform hesabı** — dublajlı yayın orijinalinden ayrı bir
  hesaptan gider (TR ses → hesap 1, klonlanmış ses → hesap 2).
- İki eşzamanlı yayın için yeterli upload bant genişliği.
- Bir ses örneği (kayıt veya dosya yükleme).
- **Ses onayı (consent) zorunlu** — kendi sesini klonlamak için açık rıza
  kaydı gerekir.
- **Ban riski uyarısı:** Twitch/Kick aynı IP'den bağlanan iki hesabı ban
  edebilir — bkz. [`05-platform-notlari.md`](./05-platform-notlari.md).

## Akış

---

### ADIM D-01 — Ses klonu oluşturma `[HAZIR]`

**Ne yapıyorsun:** Kendi sesinin bir örneğini yükleyip klonlanmış bir ses
modeli oluşturursun.
**Nerede:** `/dashboard/voice` → Ses Klonlama Stüdyosu.
**Nasıl:**
1. Mikrofonla kaydet veya ses dosyası yükle.
2. Dil modunu seç (`single_to_single` / `multi_to_single` / `single_to_multi`).
3. Gönder — model **onay bekliyor (pending_review)** durumuna geçer.
**Arka planda:** `POST /api/voice/clone` (ElevenLabs, plan `has_voice_clone`
şartı — Free/Starter'da 403), `GET /api/voice/models`.
**Sorun olursa:** Model onay kuyruğunda bekliyorsa admin onayını bekle
(otomatik değil — bilinçli insan-onaylı kapı, rıza/kötüye kullanım riski
için).

---

### ADIM D-02 — Ses onayı (consent) `[HAZIR]`

**Ne yapıyorsun:** Sesinin klonlanmasına açık onay verirsin.
**Nerede:** `/dashboard/voice` (klon oluşturma akışının parçası).
**Nasıl:** Onay metnini oku ve kabul et — bu olmadan klon oluşturulamaz.
**Arka planda:** Onay, kullanıcı kaydına kalıcı olarak işlenir.
**Sorun olursa:** Onayı geri çekmek istersen hesap ayarlarından iletişime geç.

---

### ADIM D-03 — TTS modunu seçme

**Ne yapıyorsun:** Dublaj sesinin bulutta mı yoksa kendi cihazında mı
sentezleneceğini seçersin.
**Nerede:** `/dashboard/voice` → Ses işleme kartı.
**Nasıl:** **Bulut (ElevenLabs)** `[HAZIR]` — varsayılan ve bugün tek
fiilen çalışan yol. **Cihazında (XTTS-v2)** `[YAKINDA]` — panelde seçenek
ve donanım testi akışı görünür, ama gerçek yerel sentez motoru henüz
kodlanmadı; seçilse bile bugün işlevsel bir fark yaratmaz.
**Arka planda:** `GET /api/tts/policy`, `PUT /api/tts/mode`,
`POST /api/tts/benchmark`.
**Sorun olursa:** Emin değilsen Bulut'ta kal — varsayılan ve tam
doğrulanmış yoldur.

---

### ADIM D-04 — Dublaj hedefini bağlama `[HAZIR]`

**Ne yapıyorsun:** Klonlanmış sesin hangi platform hesabına/dile
gönderileceğini belirlersin.
**Nerede:** `/dashboard/distribution` → relay hedefleri.
**Nasıl:** İkinci hesabının RTMP URL + stream key'ini ekle, o hedefe dublaj
dilini (`dub_language`) ata.
**Arka planda:** `PUT /api/relay/targets` (yalnız `rtmp://`/`rtmps://`,
anahtar şifreli saklanır).
**Sorun olursa:** Mux modu iki seçenekli görünür ama bugün yalnız
**cihazda mux (varsayılan)** `[HAZIR]` fiilen çalışıyor — video hiç buluta
çıkmaz, SDK kendi bilgisayarında birleştirir. **Buluttan mux** (zayıf
CPU/upload için, daha fazla kredi karşılığı) `[PLANLANDI]` — henüz
kodlanmadı.

---

### ADIM D-05 — Yayına çık `[HAZIR]`

[`00-ortak-adimlar.md`](./00-ortak-adimlar.md) ADIM O-05 → O-07'yi takip et.
**Arka planda:** Final çeviri → `subtitle.dubbed_audio` (ElevenLabs
`mp3_44100_128`, base64) → SDK cihazda mux → ikinci RTMP hedefi. Bu zincir
gerçek ffmpeg ile uçtan uca test edilmiştir; gerçek OBS + gerçek platform
hesabıyla canlı doğrulama henüz kullanıcı tarafından yapılmadı.

## Platforma özel notlar

Bkz. [`05-platform-notlari.md`](./05-platform-notlari.md). Twitch/Kick'te
ikinci hesap ban riski var; YouTube'da multi-audio (tek kanalda dil başına
ses parçası) `[PLANLANDI]` — bugün YouTube'da da ikinci kanal gerekir.

## Yükseltme yolu

- **Orijinal ses klonlama + üst-tier TTS + 200 saat** için → Streamer.
- **Dublajı çok platforma eşzamanlı dağıtmak** istersen →
  [`04-tam-paket.md`](./04-tam-paket.md).

## Sorun giderme

| Belirti | Aksiyon |
|---|---|
| Klon "onay bekliyor"da takılı kaldı | Admin onay kuyruğu manuel — biraz zaman alabilir |
| Dublaj sesi gecikmeli/senkronsuz | Cihazda mux gerçek zamanlı ffmpeg kullanır; CPU yükünü kontrol et |
| İkinci hesap RTMP hatası | Stream key'i ve `rtmp(s)://` URL formatını doğrula |
| Ses modelim reddedildi | Admin geri bildirimini kontrol et, yeni örnekle tekrar dene |

## İleri seviye

Kaynak dilin (altyazı) STT'sini kendi cihazında çalıştırmak, dublaj/TTS'ten
tamamen ayrı bir konudur ve her zaman en son düşünülür — bkz.
[`00-ortak-adimlar.md`](./00-ortak-adimlar.md) son bölüm, "İleri Seviye:
Kendi Cihazında STT".
