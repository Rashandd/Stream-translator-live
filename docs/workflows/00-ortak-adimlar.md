---
segment: ortak (tüm segmentler)
plan: free | starter | pro | streamer | founding
dashboard_routes:
  - /pricing
  - /solutions
  - /signup, /login, /login/verify, /login/reset
  - /dashboard/sdk
  - /dashboard/go-live
  - /dashboard/billing
api_endpoints:
  - POST /api/auth/signup|login|2fa|2fa/resend|reset/request|reset/confirm
  - GET /api/auth/oauth/{provider}/start, /callback
  - POST /api/pairing/code, POST /api/pairing/claim
  - GET /ws/stream (auth → session_ready → stream_start → stream_ack)
  - POST /api/sdk/commands, GET /api/sdk/commands, POST /api/sdk/heartbeat
  - GET /api/usage/summary, POST /api/billing/checkout
---

# Ortak Adımlar — Tüm Segmentlerin Paylaştığı Çekirdek Akış

Bu doküman, 4 segmentin (`01`–`04`) **hepsinde aynı şekilde geçen** adımları
tek yerde tanımlar. Segment dosyaları bu adımları tekrar etmez, yalnız
kendi farkını (ADIM `A-*`/`C-*`/`D-*`/`T-*`) ekler.

```
Keşif → Kayıt/Giriş → SDK indir+eşleştir → Ses yakalama kontrolü
   → (segment-özel yapılandırma) → Go-Live preflight → Yayın sırasında
   → Kullanım takibi/fatura → (yükseltme veya yenileme)
```

---

### ADIM O-01 — Keşif ve plan seçimi `[HAZIR]`

**Ne yapıyorsun:** Ürünü tanır, hangi plana ihtiyacın olduğuna karar verirsin.
**Nerede:** `/pricing`, `/solutions` (15 çözüm kartı + `coming_soon`/`waitlist`
durumları), `/how-it-works`.
**Nasıl:**
1. `/pricing`'de plan tablosunu incele (bkz. `README.md` plan matrisi).
2. `/solutions`'ta henüz canlı olmayan bir özellik ilgini çekiyorsa
   "Yayınlandığında haber ver" ile e-posta bırakabilirsin.
3. İlk 5 testerden biri olmak istersen Founding-5 kapalı beta başvurusu
   yapabilirsin (yayın platformu + kanal linki; admin onayı gerekir).
**Arka planda:** `POST /api/waitlist` (waitlist), `POST /api/founding/application`
+ `GET /api/founding/gate` (Founding-5).
**Sorun olursa:** Founding-5 dolu/kapalıysa normal plana kayıt olabilirsin.

---

### ADIM O-02 — Hesap açma ve giriş `[HAZIR]`

**Ne yapıyorsun:** Hesabını oluşturur, e-posta veya sosyal hesapla giriş yaparsın.
**Nerede:** `/signup`, `/login`, `/login/verify`, `/login/reset`.
**Nasıl:**
1. E-posta + şifreyle kaydol **veya** Google/Twitch ile tek tıkla giriş yap.
2. E-posta 2FA açıksa (`auth.email_2fa` config: `auto`/`required`/`off`)
   sana gelen kodu `/login/verify`'da gir.
3. Legal (kullanım şartları) onayı + opsiyonel pazarlama izni işaretle.
**Arka planda:** `POST /api/auth/signup|login|2fa`, `GET /api/auth/oauth/{provider}/start`
→ `/callback` (Google/Twitch), HttpOnly `sd_access`/`sd_refresh`/`sd_csrf` çerezleri.
**Sorun olursa:** 2FA e-postası gelmediyse `POST /api/auth/2fa/resend` (30 sn
bekleme kuralı var); şifreni unuttuysan `/login/reset`.

> **Not — Kick ile giriş yok `[YAKINDA]`:** Kick, OAuth 2.1 + PKCE
> gerektiriyor; bugünkü `exchange_code` yalnız `client_secret` akışını
> destekliyor. Kick yayıncıları e-posta veya Google/Twitch hesabıyla
> kaydolup platform bağlantısını ayrıca `/dashboard/platforms`'tan
> yapabilir (bkz. `05-platform-notlari.md`).

---

### ADIM O-03 — SDK indirme ve eşleştirme `[HAZIR]` (Windows/Linux) · `[KOD HAZIR — CANLI DOĞRULAMA BEKLİYOR]` (macOS)

**Ne yapıyorsun:** Masaüstü uygulamasını indirir, dashboard'daki kodla eşleştirirsin.
**Nerede:** `/dashboard/sdk`.
**Nasıl:**
1. Platformuna uygun installer'ı indir (Windows/macOS/Linux — üçü de gerçek
   indirme linkleriyle çalışır, "yakında" durumunda değil).
2. SDK'yı aç; kendi penceresinde 6 haneli bir kod ister.
3. Dashboard'da **"Eşleştirme kodu üret"** butonuna bas, çıkan kodu SDK'ya gir.
4. SDK arka planda (tray) çalışmaya başlar — kontrolün tamamı web panelden.
**Arka planda:** `POST /api/pairing/code` (panel, tek kullanımlık kod üretir)
→ SDK `POST /api/pairing/claim` (30 günlük cihaz JWT, yerel diskte
AES-256-GCM şifreli saklanır).
**Sorun olursa:**
- Windows'ta SmartScreen uyarısı çıkabilir (gerçek Authenticode sertifikası
  henüz alınmadı) — "Yine de çalıştır" ile devam edilebilir.
- macOS'ta Gatekeeper engelleyebilir (notarization henüz yok) — kullanıcı
  manuel "yine de aç" yapmalı; gerçek Mac donanımında uçtan uca doğrulama
  henüz yapılmadı (Linux mikrofon yakalaması gerçek donanımla doğrulandı).
- Kod süresi dolduysa yeni kod üret.

---

### ADIM O-04 — Ses yakalama kontrolü `[HAZIR]`

**Ne yapıyorsun:** SDK'nın hangi mikrofonu dinleyeceğini onaylarsın.
**Nerede:** SDK tray uygulaması (ayrıntılı ayar yok — otomatik algılar).
**Nasıl:** SDK, işletim sistemine göre WASAPI (Windows) veya cpal
(macOS/Linux) ile mikrofonu 16kHz mono yakalar; sessizlik VAD ile atlanır,
DC-offset/pre-emphasis ön işlemeden geçer.
**Arka planda:** Yerel işlem — hiçbir şey buluta gitmeden önce burada biter.
**Sorun olursa:** Yalnız ses (~50-128 Kbps) buluta gider, **video hiçbir
zaman göndermez** — gizlilik/bant genişliği endişesi varsa bu güvenceyi
paylaş.

---

### ADIM O-05 — Go-Live preflight `[HAZIR]`

**Ne yapıyorsun:** Yayına çıkmadan önce son kontrolü yaparsın.
**Nerede:** `/dashboard/go-live` (dağıtım moduna göre iki farklı adım listesi
gösterir — bkz. segment/`04-tam-paket.md`).
**Nasıl:** Sayfadaki checklist'i tamamla, yayın yazılımını (OBS, Streamlabs
Desktop, XSplit, vMix veya Browser/Web Source destekleyen herhangi bir
yazılım — konsol + capture-card kombinasyonu dahil) her zamanki gibi başlat.
**Arka planda:** SDK `GET /ws/stream`'e bağlanır → `auth` → `session_ready`
→ `stream_start` → `stream_ack` (yanıtta `overlay_token` gelir).
**Sorun olursa:** `auth` 5 saniye içinde gelmezse bağlantı kapanır; JWT süresi
dolduysa (`error 4001`) SDK otomatik token yeniler ve yeniden bağlanır.

---

### ADIM O-06 — Yayın sırasında `[HAZIR]`

**Ne yapıyorsun:** Canlı durumunu ve altyazı akışını izlersin.
**Nerede:** `/dashboard` (canlı durum), overlay senin yayın yazılımında.
**Nasıl:** Altyazılar `subtitle` mesajlarıyla anlık gelir (`is_final: false`
= geçici/italik, `true` = kesin); panelden SDK'ya komut göndermek istersen
(mod değişikliği vb.) panel bunu arka planda yapar.
**Arka planda:** `POST /api/sdk/commands` (panel yazar) → SDK `GET` ile
yoklar (15 sn ping/pong ile bağlantı canlı tutulur, 30 sn cevapsızlıkta
SDK yeniden bağlanır).
**Sorun olursa:** STT/çeviri sağlayıcı geçici hata verirse (`4005`/`4006`)
otomatik fallback devreye girer (Deepgram→AssemblyAI, DeepL→Google), altyazı
akışı kesintiye uğramaz.

> **Bilinen sınır:** `usage_update` mesajı bugün **periyodik gönderilmiyor**
> — kullanım arka planda takip edilir ama saat limitine ulaşana kadar
> (`error 4003`, session kapanır) ara bildirim yoktur. Kalan sürenin
> gerçek zamanlı takibi `GET /api/usage/summary`'den yapılabilir.

---

### ADIM O-07 — Kullanım takibi ve fatura `[HAZIR]`

**Ne yapıyorsun:** Bu ayki kullanımını görür, gerekirse plan yükseltirsin.
**Nerede:** `/dashboard` (özet), `/dashboard/billing` (detay + yükseltme).
**Nasıl:** Kalan saatin azaldığını gördüğünde yükseltmek için
**"Planı yükselt"**'e tıkla → Stripe Checkout'a yönlenirsin.
**Arka planda:** `GET /api/usage/summary`, `POST /api/billing/checkout`
(plan → Stripe price ID eşlemesi admin config'inden), `POST /webhooks/stripe`
(imza doğrulanır, abonelik güncellenir).
**Sorun olursa:** Saat limitin dolarsa yayın otomatik durur (`error 4003`);
plan yükseltip yeniden `stream_start` göndermen yeterli.

---

## Ortak Sorun Giderme

| Belirti | Aksiyon |
|---|---|
| WS bağlantısı sık kopuyor | SDK 15 sn'de bir `ping` atar, 30 sn cevapsızlıkta kendini yeniden bağlar — internetini kontrol et |
| Eşleştirme kodu "geçersiz" diyor | Kod tek kullanımlık ve süreli — panelden yeni kod üret |
| 2FA e-postası gelmiyor | `/login/verify`'daki "Yeniden gönder" (30 sn bekleme kuralı var); spam klasörünü kontrol et |
| Yayın aniden kesildi | `error` mesajının `code` alanına bak (`docs/websocket-protocol.md` tablosu) — `4003` plan limiti, `4001` token süresi |

---

## İleri Seviye: Kendi Cihazında STT `[YAKINDA]`

> Bu, **yalnızca STT (altyazı üretimi) için** geçerli, tamamen opsiyonel bir
> özelliktir — dublaj/ses klonlama (TTS) ile karıştırılmamalıdır (TTS'in
> kendi cihaz seçeneği `03-dublaj-ses-klonlama.md`'de ayrıca anlatılır).
> Her segment dokümanı bu bölüme link verir; anlatım tek yerde toplanır.

**Bugün varsayılan yol her zaman bulut STT'dir** (Deepgram + AssemblyAI
fallback). Kaynak dil kapsamının referansı
[`Systran/faster-whisper-large-v3`](https://huggingface.co/Systran/faster-whisper-large-v3)
modelidir (Whisper large-v3 ailesi, ~99 dil) — roadmap'in Faz 2 self-hosted
STT hedefiyle aynı model ailesi (`streamdubber_roadmap_final.md` §"STT (Ses
→ Metin)").

**"Cihazında (Yerel)" seçeneği ne sunar:** `/dashboard/captions` üstündeki
"Altyazı işleme" kartında Bulut/Cihazında arasında seçim yapılabilir.
Cihazda çalışınca kota **yarı hızda tükenir** (bizim GPU maliyetimiz
sıfırlandığı için) — karşılığında donanım şartı vardır:

| | Minimum | Önerilen |
|---|---|---|
| GPU | NVIDIA GTX 1660 / RTX 2060 (6 GB VRAM) | RTX 3060+ (8+ GB VRAM) |
| Sürücü | CUDA 12 destekli güncel sürücü | aynı |
| RAM | 16 GB | 16 GB |
| Disk | 2 GB (model dosyası) | aynı |

Seçim, **[Donanımımı test et]** butonundan geçer not almadan kilitli kalır
(gerçek 15 saniyelik ses parçasıyla ölçüm: RTF ≤ 0.5 ve p95 ≤ 1.2 sn şartı).
Yayın sırasında donanım yetişemezse (son 30 sn p95 > 1.5 sn veya art arda 3
segment > 2 sn) SDK **otomatik ve sessizce buluta geçer** — bu dakikalar
normal (1×) tarifeden sayılır, yayın kesilmez.

> **Dürüst durum notu:** Bu özelliğin panel arayüzü, sunucu-taraflı politika
> (`GET /api/stt/policy`), donanım testi/watchdog mantığı ve SDK'nın
> yoklama/komut altyapısı kodlanmış ve testlidir. Ancak **gerçek yerel
> transkripsiyon motoru (faster-whisper'ın SDK'ya gömülü native bağlantısı)
> henüz kodlanmadı** — bugün SDK bu motoru fiilen başlatmıyor. Yani panelde
> seçenek görünse bile bugün seçilmesi işlevsel bir fark yaratmaz; motor
> tamamlanınca bu not güncellenecektir. Model ailesi olarak SDK tarafında
> hedeflenen varyant `faster-whisper large-v3-turbo`dur (aynı dil kapsamı,
> daha düşük VRAM için budanmış decoder) — bkz. `docs/local-stt-plan.md`.
