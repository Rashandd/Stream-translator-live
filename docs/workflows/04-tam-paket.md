---
segment: tam-paket
plan: streamer
dashboard_routes:
  - /dashboard/distribution
  - /dashboard/go-live
api_endpoints:
  - GET /api/integrations, /api/integrations/streamlabs/authorize|test
  - PUT/GET /api/relay/targets
---

# Segment 4 — Tam Paket (Dublaj + Multistream)

## Kim için

Profesyonel yayıncı/ajans — dublaj **ve** çok platforma eşzamanlı dağıtım
istiyor: tek yayından N hesaba (Twitch + YouTube + Kick gibi), gerekirse
dublaj hedefleriyle birlikte.

**Plan:** Streamer ($99.99/ay, 200 saat, sınırsız dil/platform).

## Ön koşullar

- [`03-dublaj-ses-klonlama.md`](./03-dublaj-ses-klonlama.md)'deki tüm ön
  koşullar.
- **≥25 Mbps upload** — çoklu platforma SDK relay ile dağıtım için önerilen
  eşik (hedef sayısı arttıkça upload ihtiyacı da artar, aşağıya bkz.).
- N platform hesabı + her biri için RTMP stream key.
- Görece güçlü CPU — relay ve mux işlemi yayıncının kendi bilgisayarında
  çalışır.

## Akış

---

### ADIM T-01 — Dağıtım modu seçimi

**Ne yapıyorsun:** Yayının video/sesinin hangi yoldan platformlara
gideceğini seçersin.
**Nerede:** `/dashboard/distribution`.
**Nasıl:**
- **"OBS'ten yönet" (Mod A — SDK relay)** `[HAZIR]` — varsayılan/önerilen.
  Yayıncının kendi internetinden N platforma dağıtım; video hiç buluta
  çıkmaz.
- **Streamlabs (Mod A')** `[HAZIR]`, opsiyonel — mevcut Streamlabs
  hesabını OAuth ile bağlayıp onun üzerinden dağıtabilirsin.
- **"Strovak yönlendirsin" (Mod C — bulut multistream)** `[PLANLANDI]` —
  Faz 3, yalnızca interneti zayıf yayıncılar için düşünülen premium bir
  seçenek; bugün kodlanmadı.
**Arka planda:** `GET /api/integrations`, `/api/integrations/streamlabs/authorize`.
**Sorun olursa:** Streamlabs bağlantı testi başarısızsa
`POST /api/integrations/streamlabs/test` ile tekrar dene.

---

### ADIM T-02 — Relay hedeflerini ekleme `[HAZIR]`

**Ne yapıyorsun:** Yayının gideceği tüm hesapları (RTMP URL + stream key)
tanımlarsın.
**Nerede:** `/dashboard/distribution` (veya `/dashboard/sdk` stream key
paneli).
**Nasıl:** Her platform hesabı için ayrı bir hedef ekle; dublaj gerekiyorsa
o hedefe `dub_language` ata (bkz. `03-dublaj-ses-klonlama.md` ADIM D-04).
**Arka planda:** `PUT /api/relay/targets` (yalnız `rtmp(s)://`, anahtarlar
AES-256-GCM şifreli saklanır), `GET /api/relay/targets`.
**Sorun olursa:** URL şeması `rtmp://`/`rtmps://` değilse kayıt reddedilir.

---

### ADIM T-03 — Dublaj kurulumu (opsiyonel)

Dublajlı hedef(ler) istiyorsan [`03-dublaj-ses-klonlama.md`](./03-dublaj-ses-klonlama.md)
ADIM D-01 → D-04'ü tamamla.

---

### ADIM T-04 — Preflight ve yayına çık `[HAZIR]`

**Ne yapıyorsun:** Çoklu hedefli yayın için son kontrolü yaparsın.
**Nerede:** `/dashboard/go-live`.
**Nasıl:** Hedef sayına göre upload uyarısını oku, yayın yazılımını başlat.
**Arka planda:** [`00-ortak-adimlar.md`](./00-ortak-adimlar.md) ADIM O-05
ile aynı WS akışı; `RelayEngine` her hedefe bağımsız bağlantı/reconnect
(2 sn backoff) yönetir — biri düşerse diğerleri etkilenmez.
**Sorun olursa:** Upload yetersizse önce en düşük öncelikli hedefi
(ör. dublaj hedefi) geçici kapat.

---

### ADIM T-05 — Yayın sırasında hedef izleme `[HAZIR]`

**Ne yapıyorsun:** Her hedefin durumunu ayrı ayrı izlersin.
**Nerede:** `/dashboard` (canlı durum).
**Nasıl:** Bir hedef koparsa yalnız o hedef yeniden bağlanır, diğerleri
kesintisiz devam eder.
**Arka planda:** `POST /api/sdk/commands` (panelden mod/hedef değişikliği).
**Sorun olursa:** Sürekli düşen bir hedef varsa o hesabın stream key'ini
ve kendi platform tarafındaki durumunu (ban/kısıtlama) kontrol et.

## Platforma özel notlar

Bkz. [`05-platform-notlari.md`](./05-platform-notlari.md). Hedef sayısı
arttıkça upload ihtiyacı da artar — kaba örnek: tek hedef ~6 Mbps ise, TR +
2 dublaj hedefi ~18-25 Mbps civarına çıkabilir (gerçek değer bitrate/çözünürlük
ayarına göre değişir). Twitch/Kick'te birden fazla hesap aynı IP'den
bağlanıyorsa ban riskini göz önünde bulundur.

## Sorun giderme

| Belirti | Aksiyon |
|---|---|
| Bir hedef sürekli düşüyor, diğerleri sorunsuz | O hedefin key'ini/limitini kontrol et — `RelayEngine` diğerlerini etkilemez |
| Genel yayın kalitesi düşük | Toplam hedef sayısına göre upload yetersiz olabilir, hedef sayısını azalt |
| Streamlabs bağlantısı koptu | `/dashboard/distribution`'da yeniden authorize et (token yenileme) |

## Sonraki adım

Bu segment zaten en üst pakettir — büyüme yolu daha çok saat/özel destek
için Streamer planında kalmak veya (ileride) Founding-5/kurumsal
görüşmelerdir.
