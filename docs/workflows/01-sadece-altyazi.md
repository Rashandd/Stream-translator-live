---
segment: sadece-altyazi
plan: free | starter
dashboard_routes:
  - /dashboard/captions
api_endpoints:
  - GET /api/overlay/token
  - GET /o/{token}
---

# Segment 1 — Sadece Altyazı

## Kim için

Kendi dilinde konuşan, izleyicisine **kendi dilinde** yazılı altyazı sunmak
isteyen yayıncı — erişilebilirlik (işitme engelli izleyici), sessiz-izleme
veya sadece netlik amacıyla. Çeviri veya dublaj istemiyor, en basit ve en
ucuz yol bu segment.

**Plan:** Free ($0, 5 saat/ay, 1 dil, 1 platform, **watermark var**) veya
Starter ($9.99, 30 saat/ay, watermark yok). Bkz. `README.md` plan matrisi.

## Ön koşullar

- Browser/Web Source destekleyen bir yayın yazılımı — OBS, Streamlabs
  Desktop, XSplit, vMix veya benzeri; konsol (PS5/Xbox) + capture-card ile
  PC'ye alıp bu yazılımlardan yayına giren kurulumlar da aynı şekilde çalışır.
- Tek platform hesabı yeterli (multistream/relay gerekmez, ek bant
  genişliği ihtiyacı yok).

## Akış

Önce ortak adımları tamamla: [`00-ortak-adimlar.md`](./00-ortak-adimlar.md)
ADIM O-01 → O-04 (keşif, kayıt, SDK eşleştirme, ses yakalama kontrolü).

---

### ADIM A-01 — Konuşma dilini seçme `[HAZIR]`

**Ne yapıyorsun:** Yayında hangi dilde konuştuğunu belirtirsin (altyazının
üretileceği kaynak dil).
**Nerede:** `/dashboard/captions`.
**Nasıl:** Açılır listeden konuşma dilini seç, ya da otomatik algılamayı
(`source_lang=auto`) aktif bırak.
**Arka planda:** Seçim `stream_settings`'e kaydedilir, `stream_start.source_lang`
alanına yansır.
**Sorun olursa:** Otomatik algılama yanlış dil buluyorsa dili elle sabitle.

---

### ADIM A-02 — Overlay Builder ile görünümü ayarla `[HAZIR]`

**Ne yapıyorsun:** Altyazının fontunu, boyutunu, rengini, arkaplanını ve
konumunu (üst/alt) seçip yayın için hazır bir URL üretirsin.
**Nerede:** `/dashboard/captions` → Overlay Builder paneli.
**Nasıl:**
1. Font/boyut/renk/arkaplan/pozisyon kontrollerini ayarla — canlı önizleme
   anında güncellenir (gerçek yayınla birebir aynı stil fonksiyonunu kullanır).
2. Üretilen overlay URL'ini **Kopyala**'ya bas.
**Arka planda:** `GET /api/overlay/token` (capability token — auth
gerektirmez, token'ın kendisi yetkidir) → `/o/{token}?lang=&font=&size=&color=&bg=&position=`.
**Sorun olursa:** Hiçbir kontrole dokunmasan da varsayılan değerler
bugünkü overlay davranışıyla birebir aynı sonucu verir.

---

### ADIM A-03 — Browser/Web Source olarak ekleme `[HAZIR]`

**Ne yapıyorsun:** Ürettiğin overlay URL'ini yayın yazılımına eklersin.
**Nerede:** Yayın yazılımın (OBS/Streamlabs Desktop/XSplit/vMix/vb.).
**Nasıl:** Yeni bir Browser/Web Source kaynağı oluştur, kopyaladığın URL'i
yapıştır, sahnene ekle.
**Arka planda:** Kaynak, `/o/{token}` WebSocket'ine bağlanır, altyazılar
geldikçe anlık render edilir.
**Sorun olursa:** Overlay boş görünüyorsa token'ın doğru kopyalandığından
ve yayının başlamış olduğundan emin ol (bkz. ADIM O-05).

> **Not:** SDK'nın yayın yazılımına otomatik Browser Source eklemesi
> (obs-websocket entegrasyonu) `[PLANLANDI]` — bugün manuel "kopyala →
> yapıştır" yeterli ve desteklenen tek yol.

---

### ADIM A-04 — Yayına çık

[`00-ortak-adimlar.md`](./00-ortak-adimlar.md) ADIM O-05 (Go-Live preflight)
→ O-06 (yayın sırasında) → O-07 (kullanım/fatura) adımlarını takip et.

## Platforma özel notlar

Twitch/YouTube/Kick'in üçünde de overlay yöntemi aynı şekilde çalışır —
detay için [`05-platform-notlari.md`](./05-platform-notlari.md).

## Yükseltme yolu

- **Watermark'ı kaldırmak** veya daha fazla saat için → Starter'a geç.
- **Birden fazla dilde altyazı / izleyicinin dil seçmesi** istersen →
  [`02-altyazi-ceviri.md`](./02-altyazi-ceviri.md).

## Sorun giderme

| Belirti | Aksiyon |
|---|---|
| Overlay hiç görünmüyor | URL'deki token'ı ve yayının başladığını kontrol et |
| Altyazı gecikmeli geliyor | Bugünkü STT VAD-kesiti başına REST çağrısıdır (gerçek zamanlı streaming değil) — birkaç saniyelik gecikme normaldir |
| Yanlış dilde algılanıyor | `source_lang=auto` yerine dili elle seç |

## İleri seviye

Kendi cihazının GPU'suyla STT çalıştırmak (bulut kotasını yarı hızda
tüketir) mümkün ama bu segment için de dahil, her zaman **en son** düşünülecek
bir seçenektir — bkz. [`00-ortak-adimlar.md`](./00-ortak-adimlar.md)
dosyasının son bölümü, "İleri Seviye: Kendi Cihazında STT".
