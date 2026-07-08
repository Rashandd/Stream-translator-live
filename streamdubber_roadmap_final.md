# StreamDubber — Ürün Yol Haritası v4.0

**Website:** strovak.com

## Proje Özeti

**Vizyon:** Yayıncılara altyazı, çeviri, ses klonlama ve multistream hizmetini tek pakette sunan, platform bağımsız SaaS.  
**Rakip:** StreamTranslate (sadece yakılı altyazı, ses yok, izleyici seçemiyor)  
**Temel fark:** İzleyici dil seçer + klonlanmış ses + multistream + hepsi tek pakette  
**Hedef platformlar:** YouTube, Twitch, Kick  
**Müşteri:** Yayıncı (kullanıcı değil) — yayıncı öder, izleyici bedava kullanır  
**Büyüme stratejisi:** Önce ücretsiz/ucuz girişle kitle çek → sonra tam pakete geçir  
**Bağımlılık stratejisi:** Baştan kendi SDK multistream altyapınla ilerle → yayıncı isterse Streamlabs gibi mevcut bir servisi de bağlayabilir

---

## Büyüme Stratejisi — Aşamalı Geçiş

```
AŞAMA 1 — Kitle Çek
  Kendi SDK multistream'i (gömülü RTMP relay) ile çıkış
  Ücretsiz plan (altyazı, 1 dil, 5 saat/ay)
  Kurulum 60 saniye, OBS'e URL yapıştır bitti
  → Yayıncılar StreamTranslate yerine StreamDubber'ı dener

AŞAMA 2 — Bağla
  Ses klonlama ekle (premium özellik)
  İzleyici dil seçimi (Twitch Extension + Chrome Extension)
  → Rakipte olmayan özellikler devreye girer, geçiş maliyeti artar

AŞAMA 3 — Tek Paket
  Bulut multistream (premium, zayıf internet için) devreye girer
  Streamlabs gibi harici servisler hâlâ opsiyonel bağlanabilir
  "Tek abonelik, her şey dahil" mesajı
  → Yayıncı başka servise ihtiyaç duymaz
```

---

## Rakip ve Entegrasyon Platformları

### Rakipler (Bunlardan daha iyi ol)

| Platform | Ne yapıyor | Fiyat | Eksikliği |
|---|---|---|---|
| StreamTranslate | Altyazı overlay | $15–35/ay | Ses yok, izleyici seçemiyor |
| Lingo Echo | Chat çevirisi | Ücretsiz | Ses/altyazı yok |
| Maestra | İzleyici taraflı çeviri | $29+/ay | Yayıncı kontrolü yok |

### Multistream Dağıtım Seçenekleri

StreamDubber kendi SDK relay'iyle multistream sunar; yayıncı isterse ayrıca Streamlabs gibi mevcut bir servisi de bağlayabilir. Asıl fark: video yayıncının kendi internetinden dağıtılır, senin Gbit sunucuna gerek kalmaz.

| Yöntem | Nasıl çalışır | Öncelik | Senin maliyetin |
|---|---|---|---|
| **SDK gömülü RTMP relay** | Yayıncının kendi interneti N platforma gönderir | Faz 1 — birincil | Sıfır (video buluta çıkmaz) |
| **Streamlabs** | Yayıncı kendi Streamlabs hesabını bağlar (OAuth), isteğe bağlı | Faz 1 — opsiyonel | Sıfır (yayıncının aboneliği) |
| **Bulut multistream** | Video buluta çıkar, bulut dağıtır | Premium — opsiyonel | Gbit sunucu (~$65/ay) |

> SDK'nın kendi RTMP relay'i Faz 1'de gelir ve asıl dağıtım yöntemidir. Streamlabs bağlantısı, zaten o servisi kullanan yayıncılar için opsiyonel bir alternatiftir. Bulut multistream sadece interneti zayıf yayıncılar için premium opsiyon.

### Entegrasyon Mantığı — İki Çalışma Modu

StreamDubber'ın işleme katmanı (STT + çeviri + ses klonlama) her zaman aradadır. Fark, çıktının nasıl dağıtıldığında:

**Mod A — SDK Yerel Multistream (Faz 1, birincil)**
```
Yayıncı PC (SDK) → AES-256 → bulut işleme (sadece ses) → klonlanmış ses + altyazı → SDK
SDK içinde gömülü RTMP relay → yayıncının kendi internetinden → N platforma dağıtım
```
SDK'nın içinde bir RTMP relay var. Yayıncının kendi interneti N platforma gönderir. **Senin Gbit sunucuya ihtiyacın kalmaz** — video hiç buluta uğramaz.

**Mod A' — Mevcut Servis Pass-Through (Faz 1, opsiyonel — Streamlabs)**
```
Yayıncı PC (SDK) → AES-256 → bulut işleme (sadece ses) → klonlanmış ses + altyazı → SDK
SDK → video + ses birleştir → yayıncının Streamlabs hesabı → platformlar
```
Yayıncı zaten Streamlabs kullanıyorsa hesabını bağlayabilir. Video hiç buluta çıkmaz; sadece ses gidip gelir.

**Mod C — Bulut Multistream (opsiyonel, premium)**
```
Yayıncı PC (SDK) → video + ses buluta → bulutta birleştir + dağıt → platformlar
```
Yayıncının interneti zayıfsa veya ikili yayın için upload yetmiyorsa bulut devreye girer. Gbit sunucu gerektirir, bu yüzden premium/opsiyonel.

### İşlem ve Bant Genişliği Dağılımı

| Ne | Nerede | Boyut |
|---|---|---|
| Ses yakalama, VAD, ön-işleme | Yayıncı PC (SDK) | — |
| STT, çeviri, ses klonlama | Bulut (GPU) | ses: ~50–128 Kbps |
| Video + ses birleştirme | Yayıncı PC (Mod A/A') | lokal |
| Multistream dağıtım | Yayıncı interneti (Mod A/A') | video: 6 Mbps/platform |

**Kritik avantaj:** Mod A ve A''de video hiç buluta çıkmaz. Sadece ses (çok küçük) gidip gelir. Bu yüzden Gbit RTMP sunucusu gerekmez; sadece GPU sunucu (ses klonlama için) yeterli.

### Yayıncıya Sunum — Dashboard Akışı

Yayıncı "Yayın Kurulumu" sayfasında şu seçeneği görür:

```
Dağıtım yöntemini seç:

  ○ Doğrudan platformlara gönder (StreamDubber RTMP relay — önerilen)
      → Ekstra servise gerek yok, kendi internetinden biz dağıtırız

  ○ Mevcut servisimi kullan (Streamlabs)
      → Hesabını bağla, StreamDubber sadece çeviri/ses ekler

  ○ Sadece OBS stream key ver
      → Manuel kurulum, ileri düzey kullanıcı
```

---

## Mimari Özet

```
╔═══════════════ YAYINCI BİLGİSAYARI (Win/Mac/Linux) ═════════╗
║                                                              ║
║  StreamDubber SDK (Tauri + Rust — çok platformlu)            ║
║    ├── Ses yakalama (mikrofon)                               ║
║    ├── VAD (konuşma var mı) — hafif, lokal                   ║
║    ├── Ses ön-işleme (gürültü temizleme) — hafif, lokal      ║
║    ├── Video + ses birleştirme — lokal                       ║
║    ├── Gömülü RTMP relay (Mod A) — lokal multistream         ║
║    ├── OBS / stream key yönetimi — lokal                     ║
║    └── AES-256: sadece SES'i buluta gönderir                 ║
║                                                              ║
║  Video burada kalır, buluta ÇIKMAZ ──┐                       ║
╚══════════════════════════╤═══════════│══════════════════════╝
              AES-256 şifreli│ (sadece ses, ~50-128 Kbps)
                            ▼          │
╔══════════ STREAMDUBBER BULUT (ağır AI) ═══════╗      │
║                                                ║      │
║  ├── STT          [Deepgram → faster-whisper]  ║      │
║  ├── Çeviri       [DeepL → HY-MT1.5]            ║      │
║  └── Ses Klonlama [ElevenLabs → XTTS-v2]        ║      │
║         └── klonlanmış ses + altyazı → SDK'ya  ║      │
║                                                ║      │
║  Web Panel ←── tüm kontrol (dil, başlat, ayar) ║      │
╚════════════════════════════════════════════════╝      │
                                                         │
        ┌──────────── DAĞITIM (yayıncı interneti) ───────┘
        │  Mod A: SDK gömülü RTMP relay (kendi internet, birincil)
        │  Mod A': Yayıncının Streamlabs hesabı (opsiyonel)
        │  Mod C: Bulut multistream (premium, opsiyonel)
        └────────────────┬───────────────┘
                        ▼
            YouTube + Twitch + Kick
                        ▲
        İzleyici dil seçer: YouTube CC / Twitch Ext / Chrome Ext
```

**İşlem dağılımı (hibrit, temelde çok platformlu):**
- **Lokalde (yayıncı PC):** ses yakalama, VAD, gürültü temizleme, video+ses birleştirme, RTMP relay (Mod A), OBS yönetimi
- **Bulutta:** sadece STT, çeviri, ses klonlama — ağır AI işleri, GPU gerektirir
- **Web panel:** tüm kontrol — SDK'ya komut gönderir
- **Video hiç buluta çıkmaz** (Mod A/A') → Gbit sunucu gerekmez, sadece GPU sunucu yeterli

---

## Teknoloji Stack — Net Karar Özeti

Aşağıdaki tablo "hangi katmanda ne kullanılacak" sorusunun net cevabı. Detaylı alternatifler altta.

| Katman | Faz 1 (Başlangıç) | Faz 3 (Hedef) | Neden hibrit? |
|---|---|---|---|
| **Frontend** | Next.js + Tailwind + shadcn/ui | (aynı) | Hızlı, modern, SSR/SEO |
| **Backend** | Rust + Axum | (aynı) | Düşük latency, ses akışı için ideal |
| **Lokal SDK** | Tauri + Rust (Win öncelik, Mac/Linux temelde hazır) | Mac/Linux build aktif | Çok platformlu temel, hafif işlem + RTMP relay |
| **Realtime** | WebSocket (Axum) + Tokio | (aynı) | Canlı ses akışı |
| **DB** | PostgreSQL + Redis (cache) | (aynı) | İlişkisel + hızlı önbellek |
| **Şifreleme** | AES-256-GCM + TLS 1.3 | (aynı) | Ses akışı + key güvenliği |
| **Key yönetimi** | HashiCorp Vault / AWS KMS | (aynı) | API key + ses modeli koruma |
| **STT** | Deepgram API | faster-whisper (local) | API → local geçiş, maliyet %92 ↓ |
| **Çeviri** | DeepL API | HY-MT1.5 (local) + DeepL fallback | Yaygın diller local, nadir diller API |
| **TTS / Ses klonlama** | ElevenLabs API | XTTS-v2 (local) + ElevenLabs premium | Kalite → maliyet dengesi |
| **Multistream** | SDK gömülü RTMP relay (yayıncı interneti) | + bulut multistream (premium) | Video buluta çıkmaz, Gbit sunucu gerekmez |
| **Altyapı** | BAsic Vps (MVP) | Hetzner GPU + Cloudflare edge | Geliştirme → üretim ölçeği |
| **Ödeme** | Stripe | (aynı) | Standart |
| **Twitch izleyici** | — | Twitch Extension (React) | İzleyici dil seçimi |
| **Kick izleyici** | — | Chrome Extension (Manifest V3) | İzleyici dil seçimi |
| **Monitoring** | Sentry + Grafana | (aynı) | Hata + metrik izleme |

**Hibrit prensibi:** Hiçbir katman tek sağlayıcıya kilitli değil. Admin panelinden her katman API ↔ local arasında anlık değiştirilebilir. Bir sağlayıcı çökerse veya fiyat artırırsa, fallback devreye girer.

---

## Teknoloji Stack — Detaylı Alternatifler

### STT (Ses → Metin)

| Seçenek | Tür | Kalite | Maliyet (1000 kullanıcı×4saat) | Latency | Faz |
|---|---|---|---|---|---|
| **Deepgram Nova-2** | API | ⭐⭐⭐⭐⭐ | ~$1,032/ay | <300ms | Faz 1 başlangıç |
| **faster-whisper large-v3** | Local GPU | ⭐⭐⭐⭐⭐ | ~$80/ay sabit | ~500ms | Faz 2 geçiş |
| **Voxtral Realtime** | Local GPU | ⭐⭐⭐⭐ | ~$80/ay sabit | <200ms | Faz 3 araştırma (13 dil) |
| **AssemblyAI** | API | ⭐⭐⭐⭐ | ~$1,200/ay | ~400ms | Deepgram fallback |
| **whisper.cpp** | Local CPU | ⭐⭐⭐ | ~$20/ay | ~800ms | Düşük maliyet yedek |

### Çeviri

| Seçenek | Tür | Kalite | Maliyet (1000 kullanıcı/ay) | Dil | Faz |
|---|---|---|---|---|---|
| **DeepL API** | API | ⭐⭐⭐⭐⭐ | ~$150/ay | 100+ | Faz 1 başlangıç |
| **HY-MT1.5-7B (Tencent)** | Local GPU | ⭐⭐⭐⭐ | ~$0 (GPU paylaşımlı) | 33 | Faz 2 geçiş |
| **LibreTranslate** | Local | ⭐⭐⭐ | ~$0 | 30+ | Faz 2 yedek |
| **Google Cloud Translate** | API | ⭐⭐⭐⭐ | ~$120/ay | 130+ | DeepL fallback |
| **OPUS-MT** | Local | ⭐⭐⭐ | ~$0 | 140+ çift | Nadir diller |

### Ses Klonlama + TTS

| Seçenek | Tür | Kalite | Maliyet (1000 kullanıcı/ay) | Latency | Faz |
|---|---|---|---|---|---|
| **ElevenLabs** | API | ⭐⭐⭐⭐⭐ | ~$600/ay | ~800ms | Faz 2 başlangıç |
| **Cartesia** | API | ⭐⭐⭐⭐ | ~$240/ay | ~200ms | Hız alternatifi |
| **PlayHT** | API | ⭐⭐⭐⭐ | ~$200/ay | ~500ms | Ucuz API |
| **XTTS-v2 (Coqui)** | Local GPU | ⭐⭐⭐⭐ | ~$80/ay sabit | ~600ms | Faz 3 geçiş |
| **StyleTTS2** | Local GPU | ⭐⭐⭐⭐ | ~$80/ay sabit | ~400ms | Faz 3 araştırma |

### Multistream / RTMP

Asıl strateji: dağıtım yayıncının kendi internetinden (SDK relay) yapılır, senin sunucun video taşımaz.

| Seçenek | Nerede çalışır | Senin maliyetin | Faz |
|---|---|---|---|
| **SDK gömülü RTMP relay** | Yayıncı PC (kendi internet) | Sıfır | Faz 1 — birincil |
| **Streamlabs (yayıncının)** | Yayıncının kendi aboneliği | Sıfır | Faz 1 — opsiyonel |
| **Bulut multistream (MediaMTX)** | Senin Gbit sunucun | ~$65/ay (Hetzner 1Gbps) | Premium opsiyon |
| **SRS / Nginx-RTMP** | Senin sunucun (alternatif) | ~$40/ay | Bulut modu yedeği |

> SDK relay sayesinde video hiç buluta çıkmaz. Bulut multistream sadece interneti zayıf yayıncılar için premium opsiyon olarak kalır.

### Altyapı

| Seçenek | Güçlü yan | Maliyet | Kullanım |
|---|---|---|---|
| **Hetzner** | En ucuz GPU, Avrupa | RTX 4090: ~$200/ay | Üretim: STT+TTS+RTMP |
| **Vercel + VPS** | Frontend Vercel (Next.js native), backend VPS/Docker — KARAR 2026-07-05, bkz. `infra/deploy.md` | VPS ~$10–20/ay + Vercel hobby/pro | MVP üretim |
| **Railway** | Kolay deploy | ~$20–50/ay | (eski plan — arşiv: `infra/railway.md`) |
| **Fly.io** | Global edge | ~$20–40/ay | Overlay WebSocket |
| **Cloudflare Workers** | Edge, latency-free | ~$5/ay | WebSocket proxy |

---

## Lokal SDK — Yayıncı Agent'ı (Tauri + Rust, Çok Platformlu)

Yayıncı bir kez SDK indirir, sonra her şeyi web panelden yönetir. SDK arka planda çalışır. **Temelden çok platformlu tasarlanır** — Windows önce çıkar, ama kod tabanı Mac ve Linux'u baştan destekleyecek şekilde yazılır, böylece sonradan eklemek zorlanmadan olur.

### Çok Platformlu Temel Prensibi

```
Tek Rust kod tabanı
  ├── Platform-bağımsız çekirdek (ses işleme, RTMP relay, şifreleme, ağ)
  └── Platform-özel katman (ince adaptör)
        ├── Windows → WASAPI (ses), önce çıkar
        ├── macOS → CoreAudio (ses), aktive edilmeye hazır
        └── Linux → PulseAudio/PipeWire (ses), aktive edilmeye hazır
```

Ses yakalama dışındaki her şey (VAD, ön-işleme, RTMP relay, şifreleme, web panel bağlantısı) zaten platform-bağımsız. Sadece ses giriş katmanı platforma özel — o da küçük bir adaptör. Bu yüzden Mac/Linux eklemek temelde hazır olduğu sürece kolaydır.

### SDK Ne Yapar?

**Lokalde (hafif + orta işler, GPU gerektirmez):**
- Mikrofon sesi yakalama (platform adaptörü)
- VAD — konuşma var mı tespiti (boş sesi buluta göndermez, maliyet düşürür)
- Gürültü temizleme / ses ön-işleme
- Video + klonlanmış ses birleştirme
- Gömülü RTMP relay — yayıncının kendi internetinden N platforma dağıtım (Mod B)
- OBS entegrasyonu (OBS WebSocket ile sahne, ses kanalı, altyazı)
- Stream key ve platform konfigürasyonu (lokal, şifreli saklanır)

**Bulutla ilişki:**
- SDK sadece sesi şifreli buluta yollar (çok küçük, ~50-128 Kbps)
- Bulut klonlanmış ses + altyazıyı geri yollar
- Video hiç buluta çıkmaz → Gbit sunucu gereksiz

**Web panel ile ilişki:**
- SDK web panele bağlanır (yayıncı hesabıyla eşleşir)
- Tüm ayarlar web panelden (dil, ses profili, başlat/durdur, dağıtım modu)
- Web panel komut gönderir → SDK lokalde uygular
- Yayıncı SDK arayüzüne neredeyse hiç dokunmaz



### SDK Dağıtım ve Güvenlik
- [ ] Code signing (Windows SmartScreen + macOS notarization)
- [ ] Otomatik güncelleme mekanizması (tüm platformlar)
- [ ] SDK ↔ bulut mTLS (karşılıklı sertifika doğrulama)
- [ ] Lokal stream key'ler AES-256 ile şifreli
- [ ] Derlenmiş binary (tersine mühendislik zorluğu)

### Avantajları
- **Maliyet:** Video buluta çıkmaz + VAD boş sesi engeller → hem Gbit sunucu gereksiz hem STT maliyeti %30-40 ↓
- **Latency:** Ön-işleme ve birleştirme lokalde
- **Bağımsızlık:** Multistream yayıncının kendi internetinden, dış servise bağımlılık yok
- **Gizlilik:** Ham ses ve video yayıncı makinesinde, buluta sadece şifreli ses gider

---

## Güvenlik ve Şifreleme

### Şifreleme Katmanları

| Katman | Yöntem | Ne korur |
|---|---|---|
| **Veri aktarımı (transit)** | TLS 1.3 | SDK ↔ bulut, web panel ↔ bulut — ses akışı dahil |
| **Saklanan veri (at rest)** | AES-256 | Ses modelleri, kullanıcı verisi, DB |
| **Stream key'ler** | AES-256 + Vault | Platform stream key'leri, OAuth token'ları (lokal) |
| **API key'ler** | Vault / KMS | Deepgram, DeepL, ElevenLabs key'leri |

> Ses akışı transit şifreleme için TLS 1.3 yeterli.

### Key Yönetimi

| Seçenek | Güçlü yan | Maliyet | Öncelik |
|---|---|---|---|
| **HashiCorp Vault** | Self-hosted, tam kontrol, audit log | Sunucu maliyeti | ⭐ Önerilen (bağımsızlık) |
| **AWS KMS** | Yönetilen, kolay | ~$1/key/ay | Bulut-native isteniyorsa |
| **Infisical** | Açık kaynak, modern, ucuz | Düşük | Alternatif |

**Öneri:** HashiCorp Vault — bağımsızlık stratejisine uygun, key'ler kendi kontrolünde.

### Güvenlik Prensipleri
- [ ] Ses modelleri asla düz metin saklanmaz (AES-256 şifreli)
- [ ] API key'ler koda gömülmez, Vault'tan runtime'da çekilir
- [ ] Key rotasyonu (periyodik otomatik key değişimi)
- [ ] Her yayıncının ses modeli kendi anahtarıyla şifreli (izolasyon)
- [ ] Audit log: hangi key ne zaman kullanıldı
- [ ] Yayıncı hesabı silindiğinde ses modeli kalıcı silinir (GDPR/KVKK)
- [ ] SDK ↔ bulut mTLS karşılıklı doğrulama
- [ ] Veritabanı şifreleme (PostgreSQL TDE veya disk-level)

---

## Fiyatlandırma — Yayıncı Öder

```
İzleyici → Bedava (hiç ödeme yok)
Yayıncı  → Aylık abonelik
```

| Plan | Fiyat | İçerik |
|---|---|---|
| **Free** | $0/ay | Altyazı, 1 dil, 5 saat/ay, StreamDubber watermark |
| **Starter** | $9.99/ay | Altyazı, 3 dil, 3 platform, 20 saat, watermark yok |
| **Pro** | $24.99/ay | Altyazı + ses klonlama, 10 dil, tüm platformlar, 40 saat |
| **Streamer** | $49.99/ay | Her şey, sınırsız dil, öncelikli destek, 100 saat |

Admin panelden ayarlaanbilir olmali kesin degil fiaytlar.



---

## Dashboard Mimarisi

### A) Yayıncı Dashboard'u

#### Ana Sayfa
- [ ] Canlı yayın durumu (aktif mi, izleyici sayısı, platformlar)
- [ ] İzleyici dil dağılımı (EN: 680 / TR: 412 / ES: 148)
- [ ] Altyazı latency göstergesi
- [ ] Bu ayki kullanım / plan limiti
- [ ] Hızlı başlat (stream key kopyala)

#### Yayın Kurulumu — Dağıtım Yöntemi
- [ ] Dağıtım yöntemi seçimi (web panelden, SDK uygular)
- [ ] **SDK relay (önerilen):** Kendi internetinden dağıt, ekstra servis yok (Faz 1)
- [ ] **Mevcut servis:** Streamlabs OAuth bağlama (Faz 1, opsiyonel)
- [ ] **Bulut multistream:** İnternetin zayıfsa biz dağıtırız (premium, opsiyonel)
- [ ] Bağlı her servis/platform için durum göstergesi
- [ ] Plana göre hangi yöntemlerin açık olduğu
- [ ] İkili yayın için upload bant genişliği uyarısı (≥25 Mbps)

#### Platform Hesapları
- [ ] YouTube OAuth (native CC + multi-audio için)
- [ ] Twitch hesap 1 (TR) ve hesap 2 (EN) bağlama
- [ ] Kick hesap 1 (TR) ve hesap 2 (EN) bağlama
- [ ] Her platform için bağlantı durumu

#### Ses Klonlama Stüdyosu
- [ ] Tarayıcıdan ses kaydı (1–2 dk)
- [ ] Ses örneği yükleme (MP3, WAV)
- [ ] Model durumu: İşleniyor / Hazır / Onay Bekliyor
- [ ] Birden fazla ses profili (dil başına)
- [ ] Test: metin yaz → klonlanmış sesle dinle

#### Dil ve Altyazı Ayarları
- [ ] Konuşma dili seçimi
- [ ] Hedef çeviri dilleri (çoklu)
- [ ] Altyazı görünümü: font, boyut, renk, konum
- [ ] Overlay URL üretimi ve kopyalama
- [ ] Canlı önizleme

#### Yayın Başlat
- [ ] Stream key (StreamDubber RTMP veya bağlı platform)
- [ ] Hangi platformlara gönderileceği seçimi
- [ ] Başlatma öncesi kontrol listesi
- [ ] OBS'e yapıştırılacak tek URL / stream key

#### Analitik
- [ ] Yayın başına izleyici dil dağılımı
- [ ] Platform bazlı izleyici karşılaştırması
- [ ] Toplam yayın süresi
- [ ] Haftalık email raporu

#### Abonelik & Fatura
- [ ] Mevcut plan ve kullanım
- [ ] Plan yükseltme
- [ ] Fatura geçmişi

---

### B) Admin Dashboard'u

#### Genel Bakış (Canlı)
- [ ] Aktif yayın sayısı
- [ ] Toplam aktif kullanıcı
- [ ] STT / TTS ortalama latency
- [ ] RTMP sunucu yük yüzdesi
- [ ] Son 1 saatteki hata oranı

#### Kullanıcı Yönetimi
- [ ] Tüm yayıncılar (filtre: plan, ülke, aktif/pasif)
- [ ] Kullanıcı detayı (kullanım, ses modelleri, yayın geçmişi)
- [ ] Plan değiştirme, kredi ekleme, ban

#### Altyapı Durumu
- [ ] Her servisin canlı durumu (✅ / ⚠️ / ❌)
- [ ] Sunucu CPU / GPU / RAM / bant genişliği
- [ ] Servis başlatma / durdurma

#### Hibrit Servis Yönlendirme
- [ ] STT toggle: Deepgram ↔ faster-whisper ↔ Hibrit
- [ ] Çeviri toggle: DeepL ↔ HY-MT1.5 ↔ Hibrit
- [ ] TTS toggle: ElevenLabs ↔ Cartesia ↔ XTTS-v2
- [ ] Otomatik fallback kuralları (örn: XTTS meşgulse Cartesia'ya geç)
- [ ] Değişiklik geçmişi

#### Multistream Dağıtım Yönetimi
- [ ] Dağıtım yöntemi dağılımı (SDK relay / Streamlabs / bulut)
- [ ] SDK relay kullanan yayıncı sayısı ve durumu
- [ ] Yayıncının bağladığı dış servis (Streamlabs) durumu
- [ ] Bulut multistream (MediaMTX) yük dengeleme — sadece premium kullanıcılar
- [ ] Hangi planda hangi dağıtım yönteminin açık olduğu
- [ ] Bulut multistream maliyet takibi (sadece premium video taşıyanlar)
- [ ] SDK relay vs bulut multistream kullanım oranı

#### Maliyet Takibi
- [ ] Toplam API harcaması (anlık + aylık)
- [ ] Servis bazlı dağılım
- [ ] Local vs API tasarruf göstergesi
- [ ] Kullanıcı başına maliyet
- [ ] Gelir vs maliyet marjı

#### Ses Model Onay Kuyruğu
- [ ] Bekleyen modeller
- [ ] Ses örneği dinleme
- [ ] Onay / red / düzeltme

#### SDK Yönetimi
- [ ] Aktif SDK kurulumları (kaç yayıncıda, hangi versiyon)
- [ ] SDK versiyon dağılımı (güncel mi, eski mi)
- [ ] Zorunlu güncelleme yayınlama
- [ ] SDK hata/crash raporları
- [ ] SDK ↔ bulut bağlantı durumu

#### Güvenlik İzleme
- [ ] Key kullanım audit log'u (hangi key ne zaman)
- [ ] Key rotasyon durumu ve zamanlaması
- [ ] Başarısız mTLS/auth denemeleri
- [ ] Şüpheli erişim uyarıları
- [ ] GDPR/KVKK silme talepleri kuyruğu

#### Gelir & Abonelik
- [ ] MRR, churn, büyüme
- [ ] Plan bazlı dağılım
- [ ] Ülke bazlı kullanıcı haritası

#### Log ve Hata İzleme
- [ ] Son 100 hata
- [ ] Yayın başarısızlıkları
- [ ] API timeout logları

---

## Faz 1 — MVP: Kendi Multistream'i ile Altyazı

**Hedef:** Ücretsiz plan ile kitle çek, rakipten daha iyi altyazı sun  
**Mesaj:** "StreamTranslate'ten daha iyi, üstelik ücretsiz başlayabilirsin"  
**Stack:** Deepgram + DeepL + SDK gömülü RTMP relay (+ opsiyonel Streamlabs)

### Görevler

#### 1.1 Backend — Rust (Axum)
- [x] Axum WebSocket sunucu
- [x] JWT kimlik doğrulama
- [x] Ses akışı pipeline'ı
- [x] API gateway (STT / Çeviri yönlendirme)
- [x] PostgreSQL şeması
- [x] Admin / yayıncı yetkilendirme katmanı

#### 1.2 Güvenlik Altyapısı
- [x] TLS 1.3 tüm bağlantılarda (ses akışı transit şifreleme — üstüne AES yok)
- [x] HashiCorp Vault kurulumu (key yönetimi)  <!-- infra/docker-compose vault dev servisi; KV wiring SecretStore'da skeleton -->
- [x] API key'leri Vault'a taşı (koda gömme)  <!-- VaultSecretStore KV v2 HTTP okuması hazır+test (mock server); main.rs VAULT_ADDR+VAULT_TOKEN varsa Vault'u, yoksa Env'i seçer. Kalan: deploy'da vault'a key'lerin populate edilmesi (ops) -->
- [x] Veritabanı şifreleme at-rest (AES-256)  <!-- crypto::Cipher (AES-256-GCM) hazır+test; _encrypted kolonlara uygulama ilgili özellik gelince -->
- [x] Ses modeli şifreleme (kullanıcı bazlı izolasyon, AES-256)  <!-- aynı Cipher; ses modeli depolama Faz 2 -->
- [x] Stream key'ler AES-256 şifreli lokal saklanır  <!-- aynı Cipher; platform_accounts token kolonları için hazır -->
- [x] mTLS SDK ↔ bulut doğrulaması

#### 1.3 Lokal SDK — Tauri + Rust (çok platformlu temel)
- [x] Tauri proje kurulumu, platform-bağımsız çekirdek mimarisi
- [x] Ses yakalama — Windows (WASAPI), macOS + Linux (cpal)  <!-- Claude 2026-07-05: GERÇEK WASAPI implementasyonu yazıldı — audio/wasapi.rs: varsayılan kayıt cihazı, shared-mode + event-driven, AUTOCONVERTPCM ile motor 16kHz mono s16le'ye dönüştürür (ek resampler yok); tüm COM nesneleri özel capture thread'inde (CaptureSource Send kalır), tüketici yavaşsa kare düşürülür (canlıda gecikme biriktirilmez), kurulum hatası open()'dan senkron döner. wasapi=0.23 crate'i yalnız Windows build'ine girer; x86_64-pc-windows-gnu cross-check + clippy temiz. KALAN: Windows makinede gerçek mikrofonla smoke (user). Claude 2026-07-07: macOS+Linux audio/cpal_capture.rs — cpal (CoreAudio/ALSA), kendi lineer resampler+downmix'iyle 16kHz mono i16'ya çeviriyor; hand-rolled FFI değil cpal seçildi çünkü macOS bu repoda hiç derleme kontrolünden geçemiyor (darwin target eklendi, cross-check denendi — coreaudio-rs'in native ObjC build script'i gerçek macOS SDK/toolchain olmadan derlenemedi, beklenen). Linux: bu makinede gerçek mikrofonla (PCH ALC257) #[ignore] testiyle --ignored koşulup uçtan uca doğrulandı (5 gerçek kare). Bkz. docs/audio-pipeline.md "Yerel Ses Yakalama". -->
- [x] SDK masaüstü arayüzü — gerçek Tauri penceresi  <!-- Claude 2026-07-07: sdk/ui (düz HTML/JS, build adımsız) + main.rs tauri::Builder ile gerçek pencere açıyor (önceden headless, frontendDist boştu, tauri build hiç çalışmamıştı). commands.rs: submit_pairing_code var olan POST /api/pairing/claim'i çağırıyor, get_status/unpair. core/local_store.rs: eşleşme token'ı AES-256-GCM ile diske yazılıyor. cargo build ile gerçek link+çalıştırma doğrulandı (Linux). -->
- [x] VAD (konuşma tespiti) — boş ses göndermeyi engelle  <!-- core/vad.rs (RMS+hangover) + core/gate.rs (capture→VAD→frame, sessizlik dropped) -->
- [x] Gürültü temizleme / ön-işleme  <!-- core/preprocess.rs (DC giderme + pre-emphasis); gate'e opsiyonel adım -->
- [x] Video + ses birleştirme (lokal)  <!-- core/mux.rs: Faz1 Mod A çıktı planı (video lokal/OBS, sadece ses buluta); gerçek mux + cloned audio Faz 2 -->
- [x] Bulut WebSocket bağlantısı (sadece ses, video lokal kalır)  <!-- core/cloud.rs CloudClient; backend'e karşı e2e test (2) geçiyor -->
- [x] OBS WebSocket entegrasyonu (altyazı + ses kanalı)  <!-- integrations/obs.rs: obs-websocket v5 mesaj tipleri (Identify, SetInputSettings); canlı OBS ile manuel test gerekir -->
- [x] Web panel ile eşleşme (hesap bağlama)  <!-- core/pairing.rs: 6 haneli kod doğrulama + PairingRequest; backend pairing endpoint'i 1.4'te -->
- [x] AES-256 şifreli sadece-ses akışı → bulut (video lokal kalır)  <!-- wss/TLS 1.3 transport (CLAUDE.md/security.md: üstüne AES YOK, double encryption). mTLS client-cert connector TODO -->
- [x] Code signing (Windows; Mac notarization sonra)  <!-- tauri.conf.json bundle signing config + docs/sdk-distribution.md. Claude 2026-07-07: .github/workflows/sdk-release.yml artık build→sign→publish pipeline'ını gerçekten çalıştırıyor (tauri-action, Windows/macOS/Linux matrix); Windows Authenticode PFX-import+thumbprint mekanizması best-effort/gerçek sertifikayla hiç denenmedi. Gerçek sertifikalar (WINDOWS_CERTIFICATE/APPLE_*) hâlâ ops beklemede. -->
- [x] Otomatik güncelleme mekanizması  <!-- tauri.conf.json plugins.updater config + docs. Claude 2026-07-07: release CI artık var (TAURI_SIGNING_PRIVATE_KEY ile imzalıyor); SDK runtime'ında updater init hâlâ yapılmadı (aşağıdaki canlı doğrulama listesinde). -->
> Çekirdek platform-bağımsız yazılır; Mac/Linux ses adaptörü Claude 2026-07-07'de cpal ile aktive edildi (yukarıya bkz.).
> Faz 1.3 NOT (Claude): cloud client + e2e gerçek; preprocess/VAD/capture gerçek+test (üç platform da). SDK artık gerçek bir Tauri penceresi. OBS/signing/updater protokol+config seviyesinde — canlı OBS, sertifikalar ve release CI'nin gerçek bir sertifikayla uçtan uca denenmesi sonraki tasklarda.

#### 1.4 Yayıncı Dashboard — Next.js + Tailwind
- [x] Yayıncı giriş (2FA — email)  <!-- Claude 2026-07-05 (1.4): GERÇEK backend akışı — backend/src/users: UserStore (argon2id şifre, USERS_FILE AES-256-GCM şifreli kalıcı, DATABASE_URL varsa users tablosuna best-effort upsert) + TwoFactorStore (6 haneli kod, TTL 10dk, 5 deneme limiti, 30sn reissue guard). Uçlar: POST /api/auth/signup|login|2fa|reset/request|reset/confirm — login tek uç (önce bootstrap admin, sonra yayıncı; role yanıtla döner). 2FA kipi auth.email_2fa config'i: auto (SMTP panelden girildiyse kod maillenir, yoksa dev'de atlanır) | required | off. Frontend: login/signup/verify/reset sayfaları canlı API'ye bağlı client formlar (sd.user.token + userFetch, 2FA e-postası sessionStorage'da — URL'e yazılmaz). 10 unit + 5 e2e test (mock SMTP ile gerçek kod akışı, şifre/kod log sızıntısı yok) + #[ignore] users_persist_roundtrip_against_real_db (TEST_DATABASE_URL ile koşulur). Şifreler ve kodlar hiçbir logda yer almaz. -->
- [x] Ana sayfa (durum, hızlı başlat)  <!-- overview: metrics, izleyici dil dağılımı, stream key kopyala, hızlı başlat -->
- [x] SDK indirme ve eşleşme sayfası  <!-- /dashboard/sdk: Windows indir + 6 haneli pairing kod UI -->
- [x] Yayın kurulumu — dağıtım modu seçimi (Mod A/C)  <!-- /dashboard/distribution: "OBS'ten yönet" (SDK relay) / "Strovak yönlendirsin" (bulut) kartları; Restream/Castr/Streamlabs ayrı mod olarak gösterilmiyor -->
- [x] Dil seçimi ve overlay URL  <!-- /dashboard/captions: konuşma dili, hedef diller, görünüm, overlay URL, canlı önizleme. Claude 2026-07-05: statik "appearance" listesi caption-overlay-builder.tsx ile gerçek interaktif panele çevrildi (dil/font/boyut/renk/arkaplan/pozisyon kontrolleri, computeOverlayStyle() paylaşımlı fonksiyonuyla /o/[token] render yoluyla drift'siz canlı önizleme, üretilen URL). Ayrıca site genelinde 23 OBS'e-kilitli metin yazılım/OS-agnostik hale getirildi (OBS/Streamlabs Desktop/XSplit/vMix/Browser Source + konsol+capture-card güvencesi), favicon+benzersiz sekme başlıkları (27 route) eklendi, ve /solutions kataloğu 15 kaleme çıkarılıp gerçek waitlist backend'ine (POST /api/waitlist) bağlandı. Bkz. docs/overlay-builder-plan.md. -->
- [x] Platform hesapları (YouTube, Twitch, Kick OAuth)  <!-- /dashboard/platforms: connect/disconnect akışı, OAuth izin/scope notu, bağlı/bağsız durum UI; gerçek OAuth token exchange backend beklemede -->
- [x] Stream key yönetimi (SDK'ya senkronize)  <!-- /dashboard/sdk: stream key görüntüle/kopyala/regenerate + SDK sync durum göstergesi UI; gerçek senkron backend beklemede -->
<!-- Ek olarak UI tamamlandı (backend wiring beklemede): /dashboard/go-live (preflight + start), /dashboard/analytics, /dashboard/billing, /dashboard/voice (Faz 2 disabled state) -->
<!-- Shell: dashboard layout (sidebar nav + status strip), mobil yatay-scroll nav; DESIGN.md Responsive bölümüne uygun -->
<!-- Hepsi config-driven (en.json), i18n-ready; hard-coded sağlayıcı/plan/dil/platform yok -->

#### 1.5 Admin Dashboard — Faz 1 Temeli
<!-- NOT: 1.7 ve 1.8 STT/Çeviri görevleri; 1.5/1.6 numaraları Admin/Multistream için ayrılmıştır -->
- [x] Admin giriş (2FA — email)  <!-- /admin/login + /admin/login/verify, console shell dışında (route group); mail stub, noreply config /admin/routing'de -->
- [x] Kullanıcı listesi ve detay  <!-- /admin/users: arama + filtre + tablo (plan/ülke/durum/kullanım) + Manage; detay drawer backend bağlanınca -->
- [x] Sistem durumu izleme  <!-- /admin/infrastructure: servis sağlık (healthy/degraded/down) + CPU/GPU/RAM/bant barları -->
- [x] Servis toggle (manuel başlangıç)  <!-- /admin/routing: STT/Çeviri/TTS provider toggle (config_key görünür, hard-coded değil) + fallback + değişiklik geçmişi; /admin/infrastructure servis start/stop -->
- [x] Temel maliyet özeti  <!-- /admin/costs: STT/çeviri/TTS/infra aylık tahmin metrikleri -->
<!-- Admin shell: (console) route group layout (sidebar + status strip, Admin rozeti), DashboardNav rootHref="/admin" ile yeniden kullanıldı -->
<!-- Hepsi config-driven (admin.* en.json); canlı veri + servis start/stop + mail gönderimi backend beklemede -->
<!-- Çekirdek: /admin/routing CLAUDE.md "hiçbir şey hard-coded değil" kuralının UI'ı (provider seçimi DB'de saklanacak) -->
<!-- Ek (1.5 checklist dışı, Bölüm B specinden): /admin/multistream UI — yöntem kullanımı + plan erişim matrisi; backend 1.6'da bağlanacak -->

#### 1.6 Multistream — Kendi SDK Relay'i (Mod A) + Opsiyonel Streamlabs
- [x] SDK içine RTMP relay motoru (Rust)  <!-- sdk/src/core/rtmp: handshake (simple/uncompressed) + AMF0 codec + chunk stream demux/mux (fmt0-3, extended timestamp, Set Chunk Size) + connect/createStream/publish session negotiasyonu (accept_publisher: OBS'i sunucu rolünde kabul eder; connect_publish: hedef platforma istemci rolünde bağlanır) + RelayEngine fan-out orkestrasyonu. 65 test (33 rtmp-özel, gerçek TCP loopback ile uçtan uca fan-out testi dahil), clippy -D warnings temiz. -->
- [x] Yayıncının kendi internetinden N platforma forward  <!-- RelayEngine::start: ingest bağlantısından gelen audio/video/data mesajları tokio broadcast channel ile N hedefe fan-out edilir; her hedef kendi bağlantısını, kendi stream key'iyle, bağımsız connect/reconnect (2sn backoff) ile yönetir — biri düşerse diğerleri etkilenmez. Zaman damgaları hedef bazında sıfırdan yeniden taban alınır (geç bağlanan/yeniden bağlanan hedef için). -->
- [x] Yayıncı dashboard'unda Mod A (SDK relay) seçeneğini aç ve varsayılan/önerilen yap  <!-- Codex: /dashboard/distribution artık SDK relay'i default/recommended seçiyor; Streamlabs yalnız opsiyonel Mod A' olarak listeleniyor. -->
- [x] Upload bant genişliği uyarısı (çoklu platform için ≥25 Mbps öner)  <!-- Codex: distribution panelinde lokal multi-platform relay için 25 Mbps+ upstream uyarısı gösteriliyor. -->
- [x] Streamlabs OAuth entegrasyonu (opsiyonel alternatif)  <!-- Codex: broadcaster UI GET /api/integrations + /authorize ile bağlanıyor; token persist zaten vardı; POST /api/integrations/streamlabs/test kayıtlı token ile Streamlabs /socket/token canlı API çağrısı yapıyor. Gerçek Streamlabs hesabıyla smoke user/ops adımı. -->
- [x] Dağıtım yöntemi seçim arayüzü (web panelden) — SDK relay birincil, Streamlabs opsiyonel  <!-- Codex: /dashboard/distribution client paneli GET /api/integrations durumunu, plan/configured/connected state'lerini ve authorize yönlendirmesini bağlıyor. -->
- [x] Her bağlantı için test aracı  <!-- Codex: broadcaster dashboard test butonu POST /api/integrations/{provider}/test endpoint'ine bağlı; Streamlabs gerçek API socket-token check'i testli. -->
- [x] Admin: hangi planda hangi yöntemin açılacağı  <!-- Codex: Production Setup içinde OAuth provider plan checkbox'ları eklendi; PUT /admin/config ile integration.<provider>.plans güncelleniyor. -->
> SDK gömülü RTMP relay Faz 1'e alındı ve asıl dağıtım yöntemi oldu (video buluta çıkmaz, Gbit sunucu gerekmez). Restream ve Castr desteği kaldırıldı; yayıncı isterse Streamlabs'ı opsiyonel bağlayabilir. İkili yayın (TR/EN ayrı hesap, klonlanmış ses) Faz 2'de ses klonlama ile birlikte gelir (bkz. 2.3).
> Faz 1.6 NOT (Claude/Codex): OAuth **akış çekirdeği** (authorize URL + CSRF state tek-kullanım + token değişimi) + plan-erişim gating config-driven ve mock-test'li. Endpoint'ler: GET /api/integrations, /api/integrations/{provider}/authorize (yayıncı), /callback (state ile), POST /api/integrations/{provider}/test (yayıncı canlı bağlantı testi). client_id/secret SecretStore'dan (.env/Vault). Restream/Castr sağlayıcı kayıtları ve secret'ları kod ve `.env.example`'dan kaldırıldı. Token persist hazır; Streamlabs için kayıtlı token ile gerçek `/socket/token` API çağrısı kod ve mock test seviyesinde hazır. Gerçek Streamlabs hesabı ile canlı smoke user/ops adımı.
> Faz 1.6 NOT 2 (Claude, bu oturum): SDK RTMP relay motoru (`sdk/src/core/rtmp/`) protokol+orkestrasyon seviyesinde tamamlandı ve test edildi (gerçek TCP loopback: sahte OBS → RelayEngine → 2 sahte hedef sunucu, her hedef kendi stream key'ini ve bağımsız bağlantısını doğruluyor). KALAN (desktop-shell wiring, ayrı iş): (1) gerçek bir TCP listener'ı OBS'in bağlanacağı yerel porta bağlamak (main.rs/Tauri tarafı — `RelayEngine::start` zaten herhangi bir `AsyncRead+AsyncWrite` bağlantıyı kabul ediyor, `TcpListener::accept()`'ten gelen soketle doğrudan çalışır), (2) hedef listesini (RTMP URL + stream key) web panelden şifreli senkronize etmek — `pairing.rs`'in backend ucu gibi ayrı bir görev, (3) gerçek OBS ve gerçek platform (YouTube/Twitch/Kick) ile canlı doğrulama — hiçbiri simüle/mock dışında test edilmedi. Ayrıca: cargo workspace tokio feature'larına `sync`+`time` eklendi (broadcast channel + reconnect backoff için); `sdk/tests/cloud_e2e.rs` içindeki eski `AppState` initializer'ı (docs/sessions/codex.md'de flagli, bu işten bağımsız önceden bozuktu) güncellendi çünkü tüm workspace test/clippy koşumunu engelliyordu.

> **1.7 STT — Deepgram API** roadmap'ten kaldırıldı (kullanıcı kararı). STT olarak sadece Deepgram'ın mevcut prerecorded REST entegrasyonu (`gateway::deepgram::DeepgramStt`, DEEPGRAM_KEY varsa aktif) + fallback olarak AssemblyAI (`gateway::assemblyai::AssemblyAiStt`, 5 test) kullanılıyor. Gerçek zamanlı streaming WS ve ayrıca Türkçe kalite testi artık Faz 1 tamamlanma kriteri değil — bkz. STT kaynağı tartışması (yayıncı PC'si vs bulut) Faz 2.6b'ye taşındı.

#### 1.8 Çeviri — DeepL API
- [x] DeepL API (TR → EN, ES, DE, FR başlangıç)  <!-- gateway::deepl::DeeplTranslate: REST /v2/translate, free/pro endpoint auto, dil kodları upper-case; mock-server test; main.rs DEEPL_KEY varsa aktif. Canlı doğrulama key ile (user) -->
- [x] Google fallback  <!-- gateway::google_translate::GoogleTranslate: REST v2/translate (API key query param), TranslateProvider trait; main.rs GOOGLE_TRANSLATE_KEY varsa aktif. 3 test (mock server). Canlı doğrulama key ile (user) -->

#### 1.9 OBS Browser Source Altyazı
- [x] Gerçek zamanlı altyazı render  <!-- backend/src/overlay: OverlayHub (token→broadcast::channel) + GET /overlay/{token} (auth gerektirmez, token capability'dir). Her subtitle, ws/mod.rs'te SDK'ya gönderilirken aynı anda session'ın overlay_token'ına da publish edilir (stream_ack.overlay_token). frontend/app/o/[token]: WS'e bağlanıp gelen altyazıyı render eden client sayfa, reconnect'li. 2 backend test (tests/overlay_api.rs: SDK→overlay uçtan uca akış + bilinmeyen token'a hiçbir şey gelmemesi) + 3 OverlayHub unit test -->
- [x] Özelleştirilebilir görünüm  <!-- /o/[token] query param'ları: ?lang= (hangi çeviri gösterilir, boşsa kaynak dil) &font= &size= &color= &bg= &position=top|bottom -->
- [ ] 2 sn altı latency  <!-- canlı ölçüm gerekir (ops/user) — mimari olarak WS push + broadcast channel, ek kuyruklama yok -->
- [ ] 3 platformda test  <!-- gerçek OBS + YouTube/Twitch/Kick ile manuel doğrulama gerekir (user) -->

#### 1.10 YouTube Native CC
- [x] YouTube Live CC API  <!-- integrations::youtube::YouTubeCaptions: liveBroadcasts.list ile closedCaptionsIngestionUrl çözme + ingestion URL'ine seq'li caption POST'u. 5 test (mock Data API + ingestion sunucusu: url çözme, canlı olmayan broadcast, yanlış token, seq/body doğrulama) -->
- [x] OAuth bağlantısı  <!-- integrations/mod.rs default_providers()'a "youtube" eklendi (Google authorize/token uç noktaları, youtube.force-ssl scope), YOUTUBE_CLIENT_ID/SECRET SecretStore'dan. Paylaşılan integrations::oauth çekirdeği kullanılıyor (Streamlabs'la aynı akış, aynı testler). KALAN: Google'ın offline refresh token için gereken access_type=offline&prompt=consent parametreleri generic OAuth çekirdeğinde yok; canlı Google Cloud Console app'iyle doğrulanmadı -->

#### 1.9/1.10 NOT (Claude, bu oturum): Yukarıdaki maddeler backend'de gerçek REST client + mock-server test'li; hiçbiri canlı sağlayıcı hesabıyla (gerçek Deepgram/AssemblyAI/Google/YouTube key'leri, gerçek OBS/platform yayını) doğrulanmadı. SDK gömülü RTMP relay motoru (1.6) kullanıcı kararıyla bu oturuma dahil edilmedi — SDK'nın video/relay mimarisini değiştiren, kendi başına ayrı bir PR gerektiren büyük bir iş.

#### 1.11 Ücretsiz Plan + Stripe
- [x] Stripe altyapısı (checkout + webhook)  <!-- backend/src/billing: StripeClient (checkout session, form+Basic Auth) + webhook imza doğrulama (Stripe-Signature, zaman toleranslı HMAC-SHA256) + event parse; POST /api/billing/checkout (auth, plan→price_id config'ten) ve POST /webhooks/stripe endpoint'leri; secret'lar STRIPE_SECRET_KEY/STRIPE_WEBHOOK_SECRET SecretStore'dan. 14 test (mock Stripe server + imza roundtrip + tampered/expired/missing-sig red + checkout auth/plan doğrulama). KALAN: gerçek Stripe hesabıyla canlı test, webhook event'lerinin subscriptions/users.stripe_customer_id'ye DB persist edilmesi (şu an sadece loglanıyor, plans tablosundaki gerçek stripe_price_id'lerin admin panelden girilmesi gerekiyor) -->
- [x] Free plan (watermark, 5 saat limit)  <!-- plans::PlanRegistry: free planı monthly_hours=5, has_watermark=true, max_languages=1, max_platforms=1 ile seedli. GET /api/plans (public) + PUT /admin/plans (admin) — admin panelden değiştirilebilir, hard-coded değil. 4 test (tests/plans_api.rs: public liste, admin günceller, broadcaster reddedilir, auth gerekir) + 4 PlanRegistry unit test -->
- [x] Starter / Pro / Streamer planları  <!-- aynı PlanRegistry: starter (20h, 3 dil, watermark yok), pro (40h, 10 dil, ses klonlama flag'i, sınırsız platform), streamer (100h, sınırsız dil/platform) — fiyatlandırma tablosuyla birebir -->
- [x] Kullanım takibi  <!-- usage::UsageTracker: kullanıcı+dönem (YYYY-MM UTC) başına duvar-saati kullanım sayacı, ay değişince otomatik sıfırlanır. ws/mod.rs: session_ready artık gerçek remaining_seconds döner (UsageTracker.status + PlanRegistry.monthly_hours); her ses frame'inde pipeline::StreamSession kullanım kaydeder, limit dolunca STT/çeviri hiç çağrılmadan Error(PlanLimitReached, 4003) döner ve session kapatılır. 6 UsageTracker unit test + 1 pipeline test (plan_limit_reached_short_circuits_before_stt). KALAN: bellek içi (restart'ta sıfırlanır) — kalıcılık için usage_records tablosuna yazılması gerekiyor -->

#### 1.12 Loglama Sistemi (Admin Web Erişimi)
- [x] Merkezi loglama altyapısı (backend servisleri: WS pipeline, gateway, billing, integrations, overlay tek log formatına yazar)  <!-- backend/src/logging: Logger + LogEntry (ts_ms/level/service/message/user_id/session_id) AppState'e enjekte; ws (auth/stream_start/stop/codec red), pipeline (plan limit), gateway (stt hatası), billing (webhook imza+event), integrations (oauth callback), overlay (subscriber, token sadece 8 char önek) yazıyor -->
- [x] Log seviyeleri (error / warn / info / debug) ve yapılandırılabilir seviye (config-driven, restart gerektirmez)  <!-- LogLevel sıralı enum; asgari seviye ServiceConfig "log.level" anahtarından her yazımda okunur — PUT /admin/config ile anında değişir, restart yok (logs_api testi uçtan uca doğruluyor) -->
- [x] Loglar DB'ye veya log storage'a kalıcı yazılır (in-memory değil — restart'ta kaybolmaz)  <!-- Logger::with_file: LOG_FILE env → append-only JSONL, açılışta geri yüklenir (reopen testi var); LOG_FILE boşsa bellek içi (dev/CI) + startup warn. .env.example'a LOG_FILE eklendi. İleride istenirse aynı arayüzle Postgres store yazılabilir -->
- [x] Admin panel: `/admin/logs` sayfası — sadece admin yetkili kullanıcı erişebilir (mevcut admin auth katmanı ile korunur)  <!-- backend GET /admin/logs AdminUser extractor'lı (broadcaster 403 / anonim 401 testli); frontend /admin/logs sayfası (console) grubunda, nav'a eklendi, config-driven (admin.logs en.json) -->
- [x] Log arama/filtreleme (servis, seviye, tarih aralığı, kullanıcı/session ID)  <!-- GET /admin/logs?service=&level=&from_ms=&to_ms=&user_id=&session_id=&limit= — AND'lenir, yeniden-eskiye, limit max 1000; UI'da arama + seviye/servis filtre çipleri -->
- [x] Son N hata canlı listesi (Admin Dashboard "Log ve Hata İzleme" bölümüyle birleşir — bkz. satır 482-486)  <!-- Logger::recent_errors(n) = level=error&limit=N; /admin/logs sayfasının üstünde "Recent errors" paneli -->
- [x] Hassas veri loglanmaz (API key, stream key, JWT, ses/görüntü verisi log'a yazılmaz — sadece referans ID)  <!-- redact(): key/token/secret/password/authorization içeren anahtar=değer çiftleri + "Bearer <x>" deseni depoya yazılmadan [REDACTED]; overlay token'ı sadece 8 char önek. Testli (unit + e2e) -->
> Not: Bu madde CLAUDE.md "console.log commit etme — logger kullan" kuralının altyapısal karşılığıdır. Admin Dashboard spec'inde zaten "Log ve Hata İzleme" bölümü var (satır 482-486); burada eklenen asıl loglama motoru + kalıcılık + admin-only erişim.
> Faz 1.12 NOT (Claude): 11 logging unit + 5 logs_api e2e (raw HTTP + gerçek WS client) + 2 pipeline log testi eklendi; toplam workspace 217 test + clippy -D warnings temiz. Frontend: admin-logs.test.ts (5 test, nav/ikon/redaction sözleşmeleri), toplam 24 test + lint + tsc temiz. KALAN (diğer admin sayfalarıyla aynı desen): /admin/logs UI'ının canlı GET /admin/logs'a bağlanması (şu an config-driven mock satırlar) ve üretimde LOG_FILE'ın set edilmesi (ops).

---

## Faz 1 — MVP Öncesi Canlı Doğrulama Listesi

Aşağıdaki maddeler kod olarak `[x]` işaretli ve mock-server / birim testleriyle doğrulanmış, ancak gerçek sağlayıcı hesabı, gerçek cihaz veya gerçek platform ile HİÇ canlı test edilmemiş. MVP çıkışından önce bunların tek tek doğrulanması gerekiyor:

> **Canlı smoke (Claude, 2026-07-03): `bash scripts/smoke.sh` GEÇTİ** — gerçek backend binary'si geçici ortamla ayağa kaldırılıp CANLI doğrulandı: /health, admin login (yanlış şifre 401), panelden secret girme + sadece-maskeli okuma (anonim 401), restart'sız config toggle, pairing kodu → SDK token → relay hedef senkronu, audit logları (stream key sızıntısı yok), **restart sonrası kalıcılık** (secret panel+maskeli kaldı, relay hedefi durdu, diskteki dosyalar şifreli — düz metin yok). Lokalde doğrulanabilir her şey canlı doğrulandı; kalan maddeler gerçek hesap/cihaz/platform gerektirir.

### Güvenlik / Altyapı (1.2)
- [ ] Vault'a gerçek API key'lerin deploy sırasında populate edilmesi (ops adımı — aracı hazır: `scripts/vault-seed.sh`; MVP için zorunlu DEĞİL, key'ler admin panelden giriliyor ve şifreli kalıcı)
- [x] AES-256 at-rest şifrelemesinin gerçek veriye uygulanması <!-- Claude 2026-07-03 (V8/V9): Cipher artık gerçek at-rest veriye bağlı — OAuth token'ları (TOKENS_FILE), SDK relay stream key'leri (RELAY_FILE) ve panel secret'ları (SECRETS_FILE) AES-256-GCM şifreli; smoke testi diskte düz metin olmadığını doğruluyor. DB _encrypted KOLONLARINA (users.totp_secret_encrypted, platform_accounts.*) uygulama, tabloları dolduran user sistemi (1.4) ile aynı Cipher üzerinden yapılacak. -->
- [x] **Admin panelden secret yönetimi (restart'sız, maskeli okuma)** — `GET/PUT/DELETE /admin/secrets[/{AD}]` (sadece admin): sağlayıcı key'leri (Deepgram/AssemblyAI/DeepL/Google/ElevenLabs, Streamlabs/YouTube OAuth client'ları, Stripe secret+webhook) panelden girilir, **sadece baş+son maskeli okunur** (`sk_l…3456` — tam değer hiçbir API yanıtında yer almaz). Girilen key çağrı-anı çözümleme sayesinde restart GEREKMEDEN devreye girer: STT/çeviri/OAuth/Stripe client'ları key'i her istekte `SecretStore`'dan okur (`SecretRef`). Kalıcılık: `SECRETS_FILE` env'i verilirse AES-256-GCM şifreli dosya (anahtar `SECRETS_ENC_KEY`, yoksa JWT_SECRET'tan SHA-256 türetme). Key yokken davranış `providers.mock_fallback` config'ine bağlı: `true` (dev/CI) → mock cevap, `false` (üretim) → açık hata; admin panelden değiştirilebilir. Secret güncellemeleri audit log'a düşer (kim/hangi ad/ne zaman — değer asla loglanmaz). `JWT_SECRET` bilinçli olarak panel dışında (runtime'da değişmesi oturumları ve dosya anahtarını bozar; deploy-zamanı env/Vault işi). <!-- Claude 2026-07-03: backend/src/secrets/mod.rs (ManagedSecrets+SecretRef+mask+MemorySecretStore), crypto::derive_key, app.rs /admin/secrets rotaları, main.rs build_managed_secrets + tüm sağlayıcılar managed(store) kuruculu (deepgram/assemblyai/deepl/google/StripeClient/Integrations), gateway'e MockFallbackStt/MockFallbackTranslate. 21 yeni test: unit (maske/whitelist/şifreli persist+reload/yanlış anahtar reddi) + tests/secrets_api.rs e2e (RBAC 401/403, maskeli okuma, tam değer sızıntısı yok, audit izi) + runtime mock→real geçişi. Toplam workspace 238 test + clippy -D warnings temiz. -->

### SDK (1.3)
- [ ] Gerçek WASAPI cihaz I/O — Windows'ta gerçek mikrofon ile capture testi <!-- Claude 2026-07-05: kod TAMAM (audio/wasapi.rs, cross-check+clippy temiz) — kalan yalnız Windows'ta canlı mikrofon smoke'u (user) -->
- [x] Gerçek cpal cihaz I/O — Linux'ta gerçek mikrofon ile capture testi <!-- Claude 2026-07-07: #[ignore] testi --ignored ile bu makinede gerçek mikrofonla (PCH ALC257) koşuldu, 5 gerçek 16kHz mono kare yakalandı. -->
- [ ] Gerçek cpal cihaz I/O — macOS'ta gerçek mikrofon ile capture testi (kod yazıldı ama bu repoda hiç derleme kontrolünden bile geçemedi — darwin SDK/toolchain yok; ilk gerçek doğrulama bir Mac'te olmalı)
- [ ] OBS WebSocket entegrasyonu — canlı OBS ile manuel test (protokol mesajları yazıldı, gerçek OBS'e hiç bağlanılmadı)
- [ ] SDK masaüstü penceresi — gerçek işletim sisteminde görsel/manuel test (bu oturumda sandbox'ın WebView render'ı yazılımsal GPU kısıtı yüzünden görsel doğrulanamadı; native pencere derleyip çalışıyor ama içerik render'ı teyit edilmedi)
- [ ] Code signing — gerçek Windows/macOS sertifikaları ile imzalama (CI pipeline'ı hazır — .github/workflows/sdk-release.yml — ama gerçek sertifika yok, Windows PFX-thumbprint mekanizması hiç denenmedi)
- [ ] Otomatik güncelleme mekanizması — runtime init + release CI'nin gerçek bir sertifikayla uçtan uca testi (release CI artık var, SDK runtime init'i hâlâ yok)
- [ ] SDK ↔ bulut mTLS client-cert connector — SDK'nın cloud client'ına gerçek istemci sertifikası bağlanması + canlı karşılıklı doğrulama testi (1.2'de sunucu tarafı hazır; SDK tarafında connector TODO — bkz. 1.3 "AES-256 şifreli sadece-ses akışı" notu)
- [ ] Google/Twitch login — canlı bir OAuth hesabıyla uçtan uca test (backend mock-server testli, hiçbir gerçek Google/Twitch hesabıyla denenmedi; GOOGLE_LOGIN_CLIENT_ID/SECRET, TWITCH_CLIENT_ID/SECRET admin panelden girilmeli)

### Dashboard / Backend Wiring (1.4, 1.5)
- [ ] Platform OAuth token exchange (YouTube/Twitch/Kick) — çekirdek + şifreli token persist + YouTube offline params HAZIR (V8; streamlabs+youtube kayıtlı); KALAN: gerçek Google/Streamlabs uygulamalarıyla canlı akış; Twitch/Kick OAuth sağlayıcı kayıtları native özellik gerektiğinde eklenir (Faz 1 dağıtımı SDK relay ile — hesap bağlamak yayın için şart değil)
- [x] Stream key senkronizasyonu SDK'ya — gerçek backend wiring <!-- Claude 2026-07-03 (V9): PUT/GET /api/relay/targets, stream key'ler AES-256 şifreli kalıcı (RELAY_FILE), SDK pairing token'ıyla okuyor — canlı smoke doğrulandı. SDK'nın GET'i RelayEngine'e bağlaması 1.6 NOT 2'deki desktop-shell wiring (ayrı iş). -->
- [x] Pairing backend endpoint'i — 6 haneli kod ile SDK ↔ hesap eşleştirmesi <!-- Claude 2026-07-03 (V9): POST /api/pairing/code (panel) + /api/pairing/claim (SDK, PairingRequest sözleşmesiyle birebir) → 30 günlük cihaz JWT'si; tek kullanımlık kod, TTL 10dk; canlı smoke doğrulandı. -->
- [x] Admin panel canlı veri + mail gönderimi <!-- Claude 2026-07-03 (V2/V3/V5/V10): gerçek admin login; /admin/secrets tam canlı (maskeli+test butonu); /admin/logs canlı fetch (filtreli, Live/Sample rozetli); /admin genel bakış canlı durum şeridi; SMTP gerçek (lettre STARTTLS, panelden girilir) + POST /admin/mail/test. KALAN: servis start/stop Faz 1 kapsamı dışında bırakıldı (süreç yönetimi Railway/console'da; uygulama içi tüm toggle'lar zaten restart'sız config'te), 2FA mail AKIŞI user sistemi (1.4) ile bağlanır. -->

### Multistream (1.6)
- [ ] SDK RTMP relay'in gerçek OBS + gerçek YouTube/Twitch/Kick ile canlı fan-out testi (şu an sadece TCP loopback simülasyonu)
- [ ] Streamlabs OAuth gerçek hesap smoke — token persist + gerçek `/socket/token` API çağrısı kodda TAMAM (Codex: POST /api/integrations/streamlabs/test, mock testli); KALAN: gerçek Streamlabs app/client ve yayıncı hesabıyla canlı connect + API smoke + canlı akış doğrulaması
- [ ] Yerel TCP listener'ın OBS'in bağlanacağı porta gerçek bağlanması (main.rs/Tauri tarafı wiring)

### STT / Çeviri (1.7, 1.8)
> Araç hazır (V5): key admin panelden girilir, satırdaki **Test** butonu (`POST /admin/providers/{ad}/test`) gerçek sağlayıcı API'sine canlı doğrulama çağrısı yapar. Aşağıdakiler için tek eksik: gerçek hesap/key (user).
- [ ] Deepgram gerçek key ile canlı STT testi (TR dil kalitesi dahil)
- [ ] AssemblyAI fallback canlı testi
- [ ] DeepL gerçek key ile canlı çeviri testi
- [ ] Google Translate fallback canlı testi

### Overlay / YouTube (1.9, 1.10)
- [ ] OBS Browser Source altyazısının 3 platformda (YouTube/Twitch/Kick) canlı testi
- [ ] 2 saniye altı latency ölçümü (canlı yayında)
- [ ] YouTube Live CC — gerçek Google Cloud Console app ile canlı doğrulama (offline refresh token parametreleri V8'de eklendi: access_type=offline&prompt=consent)

### Ödeme (1.11)
- [ ] Gerçek Stripe hesabıyla canlı checkout + webhook testi
- [x] Webhook event'lerinin `subscriptions`/`users.stripe_customer_id`'ye DB persist edilmesi <!-- Claude 2026-07-03 (V6): checkout.session.completed → müşteri eşleme, subscription created/updated → upsert (price→plan config'ten), deleted → canceled; eşleşmeyen kullanıcı/plan soft-miss warn. Claude 2026-07-05: #[ignore] roundtrip testleri gerçek Postgres 16'ya karşı KOŞULDU ve GEÇTİ (stripe+users+waitlist). -->
- [ ] Plans tablosundaki gerçek `stripe_price_id`'lerin admin panelden girilmesi (mekanizma hazır: `PUT /admin/config` `billing.plan.<plan>.stripe_price_id`; gerçek price_id'ler Stripe hesabı açılınca girilir — ops)
- [x] Kullanım takibinin (UsageTracker) kalıcı hale getirilmesi <!-- Claude 2026-07-03 (V7): USAGE_FILE JSONL persist + boot restore + kompaksiyon; DATABASE_URL varsa usage_records dakika-bazlı upsert. Restart kalıcılığı testli. -->

---

## Faz 1 — Cloud Deploy Hazırlık Planı (detaylı, sıralı)

> **Deploy hedefi güncellendi (2026-07-05):** frontend **Vercel**, backend **VPS (Docker + Caddy)** — rehber: `infra/deploy.md` (railway.md arşiv). Frontend↔API ayrı origin olduğu için dashboard CORS katmanı eklendi: `cors.allowed_origins` config anahtarı (restart'sız, admin panelden), ilk deploy `CORS_ALLOWED_ORIGINS` env'iyle tohumlanır; ext/waitlist aileleri kendi Any-origin CORS'unu korur (4 e2e test: tests/cors_api.rs).

> Claude 2026-07-03: "Doğrulama listesini hallet, her şey cloud'da deploy'a hazır olsun,
> .env/secret'lar admin panelden girilsin (maskeli okunsun)" talebinin iş planı.
> V1 (admin secrets backend'i) TAMAMLANDI (yukarıda 1.2 altında işaretli).
> Aşağıdakiler tasarımı kararlaştırılmış, sıralı bekleyen işlerdir — her biri
> ayrı task/commit. Tamamlandıkça hem burada hem yukarıdaki doğrulama
> listesindeki karşılığı işaretlenmeli.

#### V2 — Minimal admin login (env-bootstrap) + frontend login wiring — TAMAMLANDI
- [x] Backend: `POST /api/auth/login` `{email, password}` → `{token, expires_at}`. Env-bootstrap tek admin: `ADMIN_EMAIL` + `ADMIN_PASSWORD_HASH` (argon2id PHC; üretmek için `cargo run --bin hash_password`) veya dev için `ADMIN_PASSWORD` (düz, startup warn). Yanlış kimlik → 401 (alan söylenmez, sabit zamanlı karşılaştırma), env tanımsız → 503. Denemeler audit log'da, şifre asla loglanmaz (testli). JWT: `sub=admin-bootstrap`, `role=admin`, `plan=streamer`, 12 saat. <!-- backend/src/auth/bootstrap.rs + bin/hash_password.rs -->
- [x] Backend testleri: `tests/auth_api.rs` (login→token→/admin/ping gerçekten açılıyor; 401; 503; audit'te şifre sızıntısı yok) + bootstrap unit testleri (argon2/plain/malformed hash/from_env).
- [x] Frontend: login formu client component (`AdminLoginForm`) — POST `/api/auth/login` (`NEXT_PUBLIC_API_URL`), token `localStorage["sd.admin.token"]`, başarıda `/{locale}/admin`'e redirect, 401/503/ağ hataları için ayrı sözlük mesajları. Eski GET-form stub'ı (şifreyi query-string'e koyuyordu) kaldırıldı. 2FA verify sayfası akış dışı görsel stub (SMTP → V10).
- [x] `.env.example`: `ADMIN_EMAIL`/`ADMIN_PASSWORD_HASH`/`ADMIN_PASSWORD`; frontend: `NEXT_PUBLIC_API_URL`. `src/lib/api.ts`: API_URL + admin token helper'ları + `adminFetch` (sonraki sayfalar bunu kullanır).
- [x] Ek: `AppState::skeleton()` test kurucusu — bundan sonra AppState'e alan eklemek test dosyalarında churn yaratmaz.
- Yayıncı login'i (users tablosu, signup, e-posta doğrulama, 2FA) bu maddenin DIŞINDA — 1.4'ün kendi işi.

#### V3 — /admin/secrets frontend sayfası (canlı API, maskeli görünüm) — TAMAMLANDI
- [x] `/{locale}/admin/secrets` sayfası: `AdminSecretsPanel` (client) canlı `GET /admin/secrets` listeler (`adminFetch` + Bearer `sd.admin.token`); satır = ad (mono), kaynak rozeti (panel/env/vault/unset), maskeli değer, `type=password` yeni değer girişi + Kaydet (PUT), panel kaynaklı satırda onaylı "Clear override" (DELETE). API tam değeri hiç göndermediği için DOM'a da gelmez; kaydet sonrası input temizlenir, liste API yanıtından yenilenir. <!-- frontend/src/components/admin-secrets-panel.tsx + app/[locale]/admin/(console)/secrets/page.tsx -->
- [x] 401/403 → oturum uyarısı + admin login linki; ağ hatası ve kaydetme hatası için ayrı sözlük mesajları.
- [x] Nav "Configuration" grubuna `KeyRound` ikonlu girdi; `admin.secrets.*` sözlük anahtarları; `admin-secrets.test.ts` sözleşme testleri (nav+ikon kayıtlı, 4 kolon, kaynak etiketleri, maskeleme notu, emoji yasağı).
- [x] DESIGN.md uyumu: pill butonlar/rozetler, hairline tablo, mono maskeli değerler, lucide ikonlar, tablo kendi `overflow-x-auto` konteynerinde (sayfa yatay taşmaz). Frontend: 43 test + lint + tsc + production build yeşil.

#### V4 — Cloud deploy paketi (Dockerfile'lar, compose, Railway rehberi, vault seed) — TAMAMLANDI
- [x] `frontend/Dockerfile`: multi-stage (deps → build → node standalone runtime), non-root `nextjs` user, node-fetch tabanlı HEALTHCHECK, `NEXT_PUBLIC_API_URL`/`NEXT_PUBLIC_OVERLAY_WS_URL` build-arg. `next.config.mjs` → `output: "standalone"` (doğrulandı: `.next/standalone/server.js` üretiliyor). docs/docker-config.md "nginx" kararı "node standalone" olarak güncellendi (SSR gerekçesiyle).
- [x] `infra/docker-compose.yml`: `frontend` servisi (backend healthy'ye depends_on, build-arg'lar); backend'e `sddata:/data` volume + `SECRETS_FILE=/data/secrets.enc` + `LOG_FILE=/data/logs.jsonl`; `env_file` opsiyonel (`required: false`) — `.env` yokken de compose config geçerli (doğrulandı).
- [x] `infra/railway.md`: iki servis (kök`/`+`backend/Dockerfile`; `frontend` root), Postgres/Redis eklentileri, zorunlu env tablosu, `/data` volume, `/health` healthcheck, TLS notu (Railway edge TLS → backend düz HTTP), deploy-sonrası sıralı kurulum (admin login → Secrets'ten key gir → test → `providers.mock_fallback=false` → stripe_price_id'ler).
- [x] `scripts/vault-seed.sh`: `.env`'deki bilinen adları KV v2'ye HTTP ile yazar (backend'in okuduğu şemayla birebir: `{mount}/data/{NAME}` → `.data.data.value`); boşları atlar; names listesi `secrets::names` ile senkron. Doğrulama listesindeki "Vault populate" adımının aracı (panel varken opsiyonel).
- [x] Doğrulama: `bash -n` script; `docker compose config` geçerli; `npm run build` standalone çıktı veriyor. (Compose'un tam `up` koşusu ilk gerçek deploy'da — imaj build'i uzun, davranış Dockerfile'larla sabitlendi.)

#### V5 — Sağlayıcı canlı bağlantı testi endpoint'i + panel Test butonu — TAMAMLANDI
- [x] `POST /admin/providers/{name}/test` (sadece admin): deepgram `GET /v1/projects`, assemblyai `GET /v2/transcript?limit=1`, deepl `GET /v2/usage` (`:fx` key'de free host'a otomatik geçiş), google `GET /v2/languages?key=`, stripe `GET /v1/balance`. Dönüş `{provider, ok, status, detail}`: key yoksa 503 (panele yönlendiren mesaj), sağlayıcı reddi/ağ hatası `ok:false` ile HTTP 200 (test koştu, sonuç negatif). Sonuçlar audit log'a düşer. <!-- backend/src/provider_check/mod.rs -->
- [x] Test URL'leri `providers.<ad>.test_url` config'inden override edilebilir → mock-server testleri: doğru key ok / yanlış key ok:false(401) / unset 503 / bilinmeyen sağlayıcı 404 / ağ hatası ok:false (7 test).
- [x] Frontend: /admin/secrets satırlarında yapılandırılmış+test-edilebilir secret'lar için "Test" butonu (Wifi ikonu) + success/danger sonuç rozeti (detay title'da); secret→sağlayıcı eşlemesi backend `provider_for_secret` ile senkron. Roadmap 560 "Her bağlantı için test aracı"nın backend yarısı kapandı; canlı key doğrulaması artık panelden tek tıkla.

#### V6 — Stripe webhook → DB persist (subscriptions + users.stripe_customer_id) — TAMAMLANDI
- [x] `AppState.db: Option<sqlx::PgPool>` — main.rs pool'u state'e taşır (skeleton() sayesinde test churn'ü yok); `FromRef` ile handler'lara akar.
- [x] `billing/persist.rs`: saf `action_for_event` eşlemesi (checkout.session.completed → LinkCustomer; customer.subscription.created/updated → Upsert [price→plan çözümü `billing.plan.<plan>.stripe_price_id` config'inin tersine taranmasıyla — panelden girilir]; deleted → Cancel; eksik alan/bilinmeyen tür → Ignore) — 5 unit test. SQL: runtime `sqlx::query`, `ON CONFLICT (stripe_subscription_id)` upsert, kullanıcı `users.stripe_customer_id` üzerinden; eşleşmeyen kullanıcı/plan soft-miss (warn log, user sistemi 1.4'te doğal kapanır).
- [x] Webhook handler: imza doğrulandıktan sonra pool varsa persist (hata log'a, yanıt 200 — sonsuz Stripe retry yerine alarm), pool yoksa "not persisted" warn. `#[ignore]` `stripe_persist_roundtrip_against_real_db` testi `TEST_DATABASE_URL` ile koşulur (plan+user seed → link → upsert → cancel). DOĞRULANDI (Claude 2026-07-05): geçici Postgres 16 container'ıyla `cargo test -- --ignored` koşuldu — stripe_persist + users_persist + waitlist roundtrip'leri GEÇTİ; migration zinciri (0001-0003) temiz DB'ye sorunsuz uygulandı.

#### V7 — UsageTracker kalıcılığı (USAGE_FILE + usage_records) — TAMAMLANDI
- [x] `USAGE_FILE`: append-only JSONL snapshot satırları (`{user_id, period, used_seconds}`); boot'ta geri yüklenir (kullanıcı+dönem başına son satır kazanır) ve dosya kompakte edilir; her gerçek artış append edilir (status() sorguları yazmaz). Boşsa bellek içi + startup warn (eski davranış).
- [x] `DATABASE_URL` varsa write-behind: her tam dakika sınırında `usage_records` upsert (fire-and-forget tokio task; FK ihlali/eksik users kaydı warn — pipeline asla düşmez).
- [x] Testler: restart simülasyonu (aynı dosyadan yeni tracker eski sayaçları görür), kompaksiyon (10 append → 1 satır), bozuk satır atlama, dönem sıfırlama reload sonrası korunur — 4 yeni test (mevcut 6 unit korunarak). DB yolu koda alındı, canlı doğrulaması V6'daki `TEST_DATABASE_URL` koşusuyla birlikte yapılır.
- [x] `.env.example` `USAGE_FILE`; compose `USAGE_FILE=/data/usage.jsonl`; railway.md env tablosuna eklendi.

#### V8 — OAuth offline params + token'ların AES-256 şifreli persist'i — TAMAMLANDI
- [x] `OAuthProvider.extra_authorize_params` — YouTube `access_type=offline&prompt=consent` (Google refresh token'ı ancak bununla verir; 1.10 eksiği kapandı). Authorize URL testleri (çekirdek + Integrations üzerinden youtube URL doğrulaması).
- [x] `integrations/token_store.rs` `TokenVault`: `(user_id, provider) → {access_token, refresh_token, expires_at}` tek şifreli blob (`TOKENS_FILE`, AES-256-GCM; anahtar `SECRETS_ENC_KEY` veya JWT'den `streamdubber.tokens.v1` bağlamıyla türetme — `resolve_enc_key` helper'ı). `complete()` başarılı değişim sonrası store'a yazar (persist hatası bağlantıyı düşürmez, warn); `GET /api/integrations` artık kullanıcı başına gerçek `connected` alanı döner. DB `platform_accounts` geçişi users FK'sı nedeniyle 1.4 sonrası (arayüz aynı kalacak).
- [x] Testler: in-memory store/connected/remove, şifreli persist+reload, dosyada düz token/refresh YOK, yanlış anahtar reddi, mock OAuth tam akışında token'ın persist edildiği. Compose/railway/.env.example: `TOKENS_FILE=/data/tokens.enc`. Workspace 265 test + clippy temiz.
- Doğrulama listesi karşılıkları: "Streamlabs OAuth — token persist" KAPANDI (canlı Streamlabs API çağrısı kaldı); "AES-256 _encrypted uygulanması"nın token ayağı KAPANDI; YouTube offline refresh parametreleri KAPANDI (gerçek Google app ile canlı doğrulama kaldı).

#### V9 — Pairing backend ucu + SDK relay hedef senkronizasyonu — TAMAMLANDI
- [x] SDK sözleşmesiyle birebir (`sdk/src/core/pairing.rs` PairingRequest): `POST /api/pairing/code` (yayıncı, auth) → 6 haneli kriptografik-RNG kod (TTL 10 dk, kullanıcı başına tek aktif kod, bellek içi — restart bekleyen kodu düşürür, panel yenisini üretir) ve `POST /api/pairing/claim` (SDK, auth'suz — kod capability) → 30 günlük cihaz JWT'si (role=broadcaster, sub=user, plan taşınır). DB varsa `sdk_sessions` best-effort insert (users FK 1.4'te kapanır). Format hatası 400 / bilinmeyen 404 / süresi geçmiş 410. <!-- backend/src/pairing/mod.rs -->
- [x] Relay hedef senkronu: `PUT /api/relay/targets` (doğrulama: rtmp(s):// zorunlu, boş key reddi, max 8 hedef; 422) + `GET /api/relay/targets` (SDK pairing token'ı veya panel; TLS üstünden düz key — RTMP publish için gerekli). `RELAY_FILE` ile AES-256-GCM şifreli kalıcılık (diskte düz stream key YOK); loglara sadece hedef ADEDİ yazılır, key asla. <!-- backend/src/relay/mod.rs -->
- [x] Testler: pairing unit (üretim/tek kullanım/eski kod iptali/expiry) + relay unit (roundtrip, doğrulama, şifreli persist+reload, yanlış anahtar reddi) + `tests/pairing_api.rs` e2e (tam akış: kod→claim→SDK token'ıyla hedef çekme; token claim'leri doğru; kod tek kullanımlık; kullanıcı izolasyonu; stream key'in /admin/logs çıktısında OLMADIĞI). Workspace 275 test + clippy temiz. Compose/railway/.env.example: `RELAY_FILE=/data/relay.enc`.
- Doğrulama listesi karşılıkları: "Pairing backend endpoint'i" KAPANDI; "Stream key senkronizasyonu SDK'ya" backend ucu KAPANDI (SDK'nın GET'i çağırıp RelayEngine'e bağlaması = 1.6 NOT 2'deki desktop-shell wiring, ayrı iş).

#### V10 — Admin panel canlı veri başlangıcı + kalan wiring — TAMAMLANDI
- [x] `/admin/logs` canlı: `AdminLogsPanel` (client) `GET /admin/logs?level=&service=&limit=200`'e Bearer ile bağlanır; seviye/servis filtreleri query param'a, arama client-side; "Live" / "Sample data" rozetiyle hangi modda olduğu görünür; API yoksa sözlük örnek satırlarına düşer (testler bozulmaz); 401'de login yönlendirmesi; son hatalar paneli canlı veriden türetilir.
- [x] `/admin` genel bakış: `AdminLiveStatus` — `/health` + `/config` + `/api/plans` canlı durum şeridi (API sağlığı, stt/translate sağlayıcısı, mock_fallback uyarı rozeti [on=warn/off=ok], plan sayısı). Diğer admin sayfaları bu deseni izler.
- [x] SMTP: `mail::send` (lettre; STARTTLS varsayılan, `mail.smtp.tls=none` test/dahili relay) — SMTP_HOST/PORT/USERNAME/PASSWORD/MAIL_FROM MANAGED secret listesine eklendi (panelden girilir, restart'sız) + `POST /admin/mail/test` gerçek test maili atar (503 yapılandırılmamış / ok:false gönderim hatası). Testler: mini SMTP mock sunucusuyla tam konuşma (zarf+konu+gövde doğrulanır), yapılandırılmamış/geçersiz adres/ulaşılamayan sunucu. 2FA mail akışı 1.4 user sistemiyle bu Mailer'a bağlanır. vault-seed.sh names listesi senkronlandı.
- [x] Workspace 279 test + clippy temiz; frontend 43 test + lint + tsc temiz.

#### V11 — Production operator handoff + no-code kurulum rehberi
> Hedef: production'da kullanıcı/operator teknik işlem yapmadan admin panelden değer girerek sistemi ayağa kaldırabilsin. AI agentlar kod, wiring, validation ve dokümantasyonu tamamlar; kullanıcı sadece gerçek hesap/key/client oluşturur, panele girer ve canlı smoke yapar.
- [x] Production operator dokümantasyonu oluşturuldu: `docs/production-operator-guide.md`  <!-- Sorumluluk ayrımı (operator vs AI agent), deploy/provider/broadcaster profile yapısı, Google/YouTube OAuth, Stripe, Streamlabs, provider key smoke ve canlı yayın checklist'i. Google resmi OAuth dokümanına göre redirect URI birebir eşleşme, youtube.force-ssl scope ve offline refresh token gereksinimi notlandı. -->
- [x] AI Agent: Admin panelde "Production Setup" sihirbazı ekle  <!-- Claude (V11): frontend/app/[locale]/admin/(console)/production-setup/page.tsx + admin-production-setup-panel.tsx — GET /admin/setup/status'tan 7 readiness kartı (admin_login/secrets/mock_fallback/redirect_base/stripe_prices/logs/persistence) yeşil/sarı/kırmızı pill ile. Nav'a "Production Setup" eklendi (ShieldCheck ikonu). Kalıcılık/pairing/relay targets ayrı kart olarak "persistence" check'ine katıldı (operatörün restart-safe olup olmadığını görmesi asıl amaç); webhook/test checkout sonuçları Faz 1'de admin/logs sayfasından izleniyor, ayrı bir panel eklenmedi. -->
- [x] AI Agent: Provider profile UI modelini tamamla  <!-- Claude (V11): aynı panel — provider tablosu (ad/tür/secret adları/configured/last tested/aksiyon), api_key sağlayıcılar için Test butonu (admin-secrets-panel'deki desenle), oauth sağlayıcılar için Connect butonu. environment=production ayrı bir alan olarak eklenmedi (Faz 1 tek ortam; deploy profile check'i zaten bunu operatöre gösteriyor). -->
- [x] AI Agent: Google/YouTube Connect akışını dashboard'a tam bağla  <!-- Claude (V11): admin-production-setup-panel.tsx Connect butonu → adminFetch(connect_path) → authorize_url → window.location.href yönlendirmesi. Faz 1'de broadcaster dashboard'u yok; admin kendi bootstrap hesabıyla bağlanıp doğrular. Gerçek Google app ile canlı doğrulama hâlâ user/ops adımı (docs/production-operator-guide.md). -->
- [x] AI Agent: Stripe price ID yönetimini admin UI'dan yapılır hale getir  <!-- Claude (V11): aynı sayfada "Stripe plan price IDs" bölümü — GET /api/plans'tan ücretli planları listeler, her biri için PUT /admin/config ile billing.plan.<plan>.stripe_price_id yazan edit-in-place form (free plan'da not gösterilir, input yok). Webhook/checkout sonuçları admin/logs'ta izleniyor, ayrı bir gösterge eklenmedi (Faz 1 kapsamı). -->
- [x] AI Agent: SDK desktop relay wiring'i tamamla  <!-- Claude (V11): core::relay_sync::fetch_relay_targets (GET /api/relay/targets, backend'in {platform,rtmp_url,stream_key} modelini SDK'nın RelayTarget'ına çevirir) + core::relay_service::{bind_ingest_listener, accept_and_relay} (gerçek localhost TCP dinleyici → RelayEngine::start). run_desktop_shell artık bu ikisini bağlayan async fonksiyon (DesktopShellConfig: backend_base_url/device_token/listen_addr); sdk-tauri main.rs #[tokio::main] ile env'den okuyup çağırır. KAPSAM BİLİNÇLİ SINIRLANDI (kullanıcı onayıyla): gerçek `tauri` crate'i/Builder/pencere/GUI kurulmadı — sdk/ui hiç yok, bu ayrı bir iş. sdk/tests/relay_wiring.rs (3 test, gerçek backend'e karşı): fetch targets round-trip, kötü token 401, gerçek listener→accept→engine bağı. OBS ile gerçek canlı smoke hâlâ user/ops adımı. -->

##### V11 tamamlandı (backend + admin panel + SDK çekirdek wiring)
Roadmap V11'deki 5 "AI Agent" maddesi tamamlandı (V11 backend commit'leri +
Production Setup admin sayfası + SDK relay wiring). Kalan tüm V11 maddeleri
"Kullanıcı/Operator" sorumluluğunda: gerçek hesap/key/client oluşturma ve
canlı smoke — kod tarafında eksik yok.
- [ ] Kullanıcı/Operator: Production deploy profile değerlerini gir  <!-- JWT_SECRET, ADMIN_EMAIL, ADMIN_PASSWORD_HASH, DATABASE_URL, REDIS_URL, SECRETS_FILE/TOKENS_FILE/RELAY_FILE/USERS_FILE/USAGE_FILE/LOG_FILE/SECRETS_ENC_KEY, CORS_ALLOWED_ORIGINS. VPS kurulum adımları: infra/deploy.md §2. -->
- [ ] Kullanıcı/Operator: Google production profile'ı oluştur  <!-- Google Cloud project, OAuth branding, authorized domain, YouTube/Google APIs enable, Web OAuth client, redirect URI: https://api.<domain>/api/integrations/youtube/callback; YOUTUBE_CLIENT_ID/SECRET ve GOOGLE_TRANSLATE_KEY admin panelden girilir. -->
- [ ] Kullanıcı/Operator: Stripe production profile'ı oluştur  <!-- Products/prices, webhook endpoint https://api.<domain>/webhooks/stripe, STRIPE_SECRET_KEY/WEBHOOK_SECRET, price_id'ler admin panelden girilir. -->
- [ ] Kullanıcı/Operator: Deepgram/DeepL/AssemblyAI/Streamlabs key/client bilgilerini admin panelden gir ve Test/Connect smoke yap  <!-- Test geçmeden providers.mock_fallback=false yapılmaz; testler geçince mock fallback kapatılır. -->
- [ ] Kullanıcı/Operator: Gerçek OBS + SDK + YouTube/Twitch/Kick canlı smoke'u yap  <!-- Fan-out, overlay, latency, loglarda secret sızıntısı yok, Stripe checkout/webhook, YouTube CC doğrulaması. -->

---

**Faz 1 tamamlandığında:**
Yayıncı ücretsiz hesap açar → SDK'yı kurar (kendi RTMP relay'i veya OBS ile) → Türkçe konuşur → İngilizce altyazı çıkar. Rakipten üstün, ücretsiz başlanabiliyor.

---

## Faz 2 — Ses Klonlama + İzleyici Dil Seçimi

**Hedef:** Rakipte olmayan özellikleri devreye al, ücretli plana geçişi tetikle  
**Mesaj:** "Artık izleyicin kendi dilini seçiyor ve senin sesinle duyuyor"  
**Stack:** ElevenLabs + Twitch Extension + Chrome Extension + faster-whisper geçiş başlıyor

### Görevler

#### 2.0 Çeviri — DeepL Dil Matrisi Genişletme
> 1.8'de DeepL TR→EN/ES/DE/FR ile sınırlı başlatıldı. Faz 2'de DeepL'in desteklediği tüm diller açılır — çeviri tarafında local (HY-MT1.5) hâlâ Faz 3'te kalır, Faz 2'de tek sağlayıcı DeepL'dir ama kapsamı tam.
- [x] DeepL'in desteklediği tüm hedef dilleri (100+) `gateway::deepl` dil listesine ekle — hard-coded 4 dil yerine DeepL API'nin `/v2/languages` uç noktasından çekilen veya config'e seed edilen tam liste <!-- Claude 2026-07-06: `gateway::deepl::translate()` zaten Faz 1.8'den beri rastgele target kabul ediyordu (hard-code yoktu) — asıl eksik, hangi kodların GERÇEKTEN sunulacağını bilen katalogdu. backend/src/translate_languages.rs: 29 dilli statik katalog (kod+ad+formality desteği; DeepL "100+" iddiası abartılı — canlı key yokken /v2/languages yerine seed, periyodik doğrulama notu koda eklendi). config `translate.deepl.enabled_targets` (seed: tam katalog) admin'in daraltabileceği alt küme. -->
- [x] Admin panelden hangi hedef dillerin yayıncıya sunulacağı seçilebilir olsun (config-driven, plan bazlı gating `1.6`/`2.6b` ile aynı desen) <!-- Claude 2026-07-06: GET /api/translate/catalog (auth'suz, gizli veri yok) + PUT /admin/config (zaten genel) ile admin/routing sayfasına yeni "DeepL target languages" kartı (admin-deepl-languages-panel.tsx). Plan bazlı azami sayı zaten PlanRegistry.max_languages'te vardı (free=1/starter=3/pro=10/streamer=-1) — hiç enforce edilmiyordu; ws::handle_control'e eklendi (4008 too_many_target_languages) + PUT /api/translate/languages'ta da aynı kural. -->
- [x] Yayıncı dashboard dil seçimi: sabit 4 dilli liste yerine tam DeepL listesinden çoklu seçim UI'ı <!-- Claude 2026-07-06: backend stream_settings modülü (StreamSettingsStore, düz JSON — dil kodu secret değil, sdk_link::SttSettingsStore deseni) + GET/PUT /api/translate/languages (katalog+enabled+plan azami+kayıtlı seçim tek çağrıda; PUT katalog/plan'a karşı doğrular). Frontend target-language-picker.tsx (/dashboard/captions, captions.targets static listesinin yerine) — 8 sözleşme testi. NOT (2026-07-06): SDK'nın stream_start anında bu ayarları çekmesi ayrı işti — run_desktop_shell o an henüz gerçek bir stream_start kurmuyordu (yalnız relay+stt-control loop'u vardı). ÇÖZÜLDÜ (2026-07-07): bkz. bu bölümün sonundaki "SDK'da gerçek zamanlı bulut ses oturumu wiring" checkbox'ı — core::cloud_session artık tam olarak bunu yapıyor. -->
- [x] Kaynak dil oto-algılama (`source_lang=auto`) opsiyonu — yayıncı konuşma dilini elle seçmek istemezse <!-- Claude 2026-07-06: gateway/deepl.rs — source="auto" (case-insensitive) iken `source_lang` form alanı TAMAMEN gönderilmez (DeepL "AUTO" diye bir kod tanımıyor, alan yoksa oto-algılıyor). stream_settings default kaynak dili "auto". Test: auto_source_lang_omits_the_form_field. -->
- [x] Bazı dil çiftleri için DeepL "beta/formal-informal" desteği yoksa UI'da uyarı (kalite beklentisi netleşsin) <!-- Claude 2026-07-06: katalogdaki her dilde supports_formality alanı; picker'da formality desteklemeyen dilde satır içi not ("No formal/informal tone control for this language"). -->
- [x] SDK'da gerçek zamanlı bulut ses oturumu wiring — 758'deki NOT'ta işaretlenen eksik: `run_desktop_shell` artık gerçek bir `stream_start` kuruyor <!-- Claude 2026-07-07: yeni sdk core::cloud_session modülü — capture→preprocess→VAD→framing (gate.rs) ile CloudClient'ı (cloud.rs) birleştirir: GET /api/translate/languages'i çeker (relay_sync.rs deseni), target_langs boşsa NoTargetLanguages ile açıkça reddeder, connect→authenticate→start_stream, spawn_blocking'teki capture pipeline'ı tokio mpsc ile async tarafa köprüler, tokio::select! ile hem ses gönderir hem Subtitle/UsageUpdate/Error okur, kapanışta 500ms drain penceresiyle yolda kalan son altyazıyı kaçırmaz. capture.rs'e `impl CaptureSource for Box<dyn CaptureSource>` eklendi (CapturePipeline generic'i boxed capture'ı doğrudan kabul etsin diye). run_desktop_shell relay motoru başladıktan sonra bunu ayrı bir tokio::spawn ile başlatır (stt_control'ün poll loop'uyla aynı "logla+yut, join etme" duruşu) — capture açılamazsa (Linux/Mac Faz-1 iskeleti) veya oturum hata verirse sadece loglanır, RTMP relay etkilenmez. sdk/tests/cloud_session_wiring.rs (2 e2e, gerçek backend'e karşı): dil ayarları kaydedilmiş yayıncı için MockCapture'la tam ses→altyazı döngüsü (1 konuşma karesi → 1 altyazı), dil seçilmemiş yayıncı için NoTargetLanguages reddi. KASITLI KAPSAM DIŞI: OBS bağlanma/kopma ile ses oturumunun senkronize başlatılıp durdurulması (RelayEngine bugün "durdu" sinyali vermiyor, mevcut olgunlukla tutarlı), yeniden bağlanma/retry, local STT/TTS entegrasyonu (native motor yok, ayrı blocker — whisper.cpp/XTTS ile aynı kategori). -->

#### 2.3'ten önce (SDK ses wiring bulgusu, 2026-07-07)
> `sdk/lib.rs::run_desktop_shell` incelenirken bulundu: SDK'nın ses-buluta-gönderme istemcisi
> (`core::cloud::CloudClient`) daha önce çalışan hiçbir yere bağlı değildi — capture/VAD/framing
> (`gate.rs`) ve `cloud.rs` birbirinden habersizdi, `run_desktop_shell` sadece RTMP relay'i
> başlatıyordu. Bu, Task 2.3'ün ("TR ses → hesap 1, EN klonlanmış ses → hesap 2") ön koşuluydu ve
> yukarıdaki yeni checkbox'la kapatıldı (kullanıcı kararı: 2.3'ün mux motorundan önce bu bitirilsin).
> ÇÖZÜLDÜ (2026-07-07): 2.3'ün kendisi (cihazda mux) de aynı oturumda kodlandı — bkz. 2.2'deki
> "SDK'da video + klonlanmış ses birleştirme" ve 2.3'ün kendi checkbox'ı. Buluttan mux hâlâ ayrı,
> henüz kodlanmadı.

#### 2.1 Ses Klonlama — ElevenLabs
- [x] ElevenLabs API entegrasyonu <!-- Claude 2026-07-06: gateway::tts (TtsProvider trait + MockTts + MockFallbackTts — stt.rs deseni) + gateway::elevenlabs (POST /v1/voices/add multipart clone_voice, POST /v1/text-to-speech/{id} synthesize; managed/with_endpoint, mock-server testli). Gateway.clone_voice/.synthesize dispatch: tts.provider="disabled" (varsayılan, admin PUT /admin/config ile "elevenlabs" yapar) → NoProvider. ELEVENLABS_KEY zaten secrets::MANAGED'daydı; provider_check'e XiApiKey auth stili + CheckDef eklendi (admin/secrets "Test connection" artık ElevenLabs'i de destekliyor). -->
- [x] Ses kaydı stüdyo arayüzü <!-- Claude 2026-07-06: /dashboard/voice — voice-clone-studio.tsx: MediaRecorder ile mikrofon kaydı VEYA dosya yükleme (audio/*), Blob→base64, model listesi + durum rozetleri. -->
- [x] Model oluşturma pipeline'ı <!-- Claude 2026-07-06: backend/src/voice_models — POST /api/voice/clone (plan has_voice_clone şartı — PlanRegistry zaten vardı, free/starter 403; base64 decode + 10MB üst sınır; gateway.clone_voice çağırır) + GET /api/voice/models (yayıncının kendi modelleri). Model plain-JSON VoiceModelStore'da (VOICE_MODELS_FILE) — ham ses örneği hiç saklanmaz, sadece sağlayıcının provider_voice_id'si. 6 e2e test (tests/voice_models_api.rs). -->
- [x] Admin ses model onay kuyruğu <!-- Claude 2026-07-06: GET /admin/voice/models (tüm kullanıcılar) + POST .../{id}/approve|reject. Yeni model her zaman pending_review ile başlar — ElevenLabs'in kendisinde onay kavramı yok, bu StreamDubber'ın rıza/kötüye kullanım riski için koyduğu bilinçli insan-onaylı kapı (Faz 2.2 canlı TTS sadece approved modelleri kullanacak). /admin/voice-models sayfası (admin-voice-models-panel.tsx) — tablo + onayla/reddet. -->
- [x] Ses modeline dil ataması şeması: `language_mode` (`single_to_single`/`multi_to_single`/`single_to_multi`) + `source_languages`/`target_languages` — düz `language: String` yerine (2026-07-06 kullanıcı kararı, bkz. dosya sonu "Öz Kaynaklı..." bölümü) <!-- Claude 2026-07-06: voice_models::LanguageMode enum + VoiceModel.source_languages/target_languages: Vec<String>. clone_voice request'i language_mode+source_languages alır (target_languages BOŞ başlar, admin sonradan assign ile doldurur — bkz. 2.2). Doğrulama: single_to_single/single_to_multi tam 1 source_language ister, multi_to_single en az 2. voice-clone-studio.tsx'e 3 modlu seçici + /api/translate/catalog'dan (2.0) beslenen kaynak-dil seçici eklendi (tekli select veya çoklu checkbox, moda göre). -->
- [x] Ham ses örneği artık AES-256-GCM şifreli saklanır (2026-07-06 kullanıcı kararı — önceki "hiç saklama" kararının bilinçli tersi, gerekçe: yeniden eğitim/sağlayıcı değişimi/model iyileştirme) <!-- Claude 2026-07-06: voice_models::VoiceSampleStore (yeni dosya, sample_store.rs) — relay::RelayTargetStore ile BİREBİR AYNI desen: tüm JSON blob'u (model_id→base64 örnek) tek parça AES-256-GCM şifreli, ayrı dosyada (VOICE_SAMPLES_FILE, main.rs'te streamdubber.voice_samples.v1 context'iyle derive_key). Model metadata'sından (VoiceModelStore, gizli değil) kasıtlı AYRI tutuldu. clone_voice best-effort persist eder — şifreleme/disk hatası model oluşturmayı ENGELLEMEZ, sadece VoiceModel.has_stored_sample=false kalır. VoiceModel'e ayrıca mode (Dubbing|SpeechToSpeech, varsayılan Dubbing) + source (UserCloned|Preset, varsayılan UserCloned) alanları eklendi — şema hazır ama dashboard'da SpeechToSpeech/Preset SEÇİLEMEZ (gerçek self-hosted motor/S2S pipeline'ı dosya sonundaki ayrı işe bağlı). -->

#### 2.2 Gerçek Zamanlı TTS
- [x] Metin → klonlanmış ses (2 sn altı) <!-- Claude 2026-07-06: pipeline::StreamSession.synthesize_dubs — final (is_final=true) altyazı çevirisi sonrası, tts_source=cloud VE hedef dil için admin'in aktif kıldığı onaylı bir model varsa gateway.synthesize gerçekten çağrılır; sonuç ws::protocol::ServerMessage::Subtitle.dubbed_audio'ya (yeni alan, Vec<DubbedAudioTrack{lang,audio_base64}>, eski SDK'lar serde default ile yok sayar) eklenir. Model yoksa/sağlayıcı hata verirse o dil sessizce atlanır (gateway.translate_all'ın best-effort deseniyle aynı) — bir dilin dub başarısızlığı ne diğer dilleri ne altyazının kendisini etkiler. tts_source=local'de (SDK kendi motoruyla sentezleyecek, KALAN — bkz. 2.2b) backend hiç sentezlemez, sadece translations metnini taşır. "2 sn altı" gecikme hedefi CANLI ElevenLabs key'i olmadan ÖLÇÜLEMEDİ (ağ+model gecikmesi) — dispatch kodu gerçek ve 6 yeni pipeline testiyle (mock TTS + gerçek/başarısız senaryolar) doğrulandı, uçtan uca gecikme doğrulaması kullanıcı aksiyonu. -->
- [x] SDK'da video + klonlanmış ses birleştirme (cihazda mux) <!-- Claude 2026-07-07: sdk/src/core/mux/ (mux.rs bir dizine dönüştü) — flv.rs: rtmp::RtmpMessage'ı FLV byte akışına serileştirir (video+script tag'leri, audio ATLANIR — ses input 1'den geliyor), rtmp media payload'ı zaten FLV tag body'siyle birebir aynı format olduğu için saf konteyner framing, re-encode yok. audio_scheduler.rs: DubAudioScheduler — kesikli dublaj sesini (push_dub_audio) sürekli PCM akışına çevirir (next_chunk(n) — kuyrukta yeterince örnek yoksa sessizlikle doldurur, meetily referans projesindeki reaktif silence-fill fikriyle aynı). ffmpeg_process.rs: ffmpeg child process lifecycle (gpu_probe.rs'nin best-effort external-binary deseni — ffmpeg_available() best-effort kontrol, spawn/stdin/wait, kill_on_drop). rtmp/relay.rs'e run_dub_target + run_dub_session eklendi: dub_language'i olan hedefler run_target yerine buraya yönlendirilir — yerel bir TcpListener açar, ffmpeg'i "-f flv -i pipe:0 -f s16le -ar 24000 -ac 1 -i tcp://127.0.0.1:<port> -map 0:v -map 1:a -c:v copy -c:a aac -f flv <hedef>" komutuyla spawn eder (RTMP çıkışı ffmpeg'in KENDİ muxer'ı üzerinden — bizim rtmp modülümüz yalnız publish-accept/publish-connect biliyor, "bir client'a stream servis et" rolü hiç yok, bu yüzden yerel TCP kaynağı; ekstra fd yerine stdin+TCP tercih edildi çünkü Windows'ta ekstra fd taşınabilir değil). KRİTİK bulgu (elle doğrulandı): ffmpeg girişleri sırayla açıyor/analiz ediyor — stdin (canlı, EOF vermeyen) girişi sınırsız analiz edilirse ikinci girişi (TCP) hiç açmıyor (elle test: EOF'suz 5sn tutulan stdin'de ffmpeg TCP'ye AS SONRA bağlandı) — `-probesize 32k -analyzeduration 500000` ile bu tıkanma giderildi (load-bearing, tuning değil). RelayEngine.dub_audio_sinks() (dil→mpsc::Sender<Vec<i16>> haritası) dışa açıldı; cloud_session.rs Subtitle.dubbed_audio'yu (base64→PCM i16, decode_dub_audio) dile göre ilgili sink'e yönlendirir (route_dub_audio, best-effort — sink yoksa/decode hatası sessizce düşer). run_desktop_shell iki alt-sistemi birbirine bağlar. Test: flv.rs/audio_scheduler.rs saf birim testleri + ffmpeg_process.rs gerçek ffmpeg smoke testi + rtmp/relay.rs'te GERÇEK ffmpeg'le uçtan uca test (sahte OBS → gerçek ffmpeg mux → sahte gerçek RTMP hedefi, pushlanan dublaj sesi gerçekten AAC'ye encode edilip hedefe ulaştığı doğrulandı — ffmpeg test fixture'ı da yine gerçek ffmpeg'le testsrc'ten üretildi). Ön-adım: gateway::elevenlabs OUTPUT_FORMAT mp3_44100_128→pcm_24000 (SDK'da mp3 decode bağımlılığı hiç eklenmedi, ASSUMED_TTS_BITRATE_BPS 384_000'e güncellendi). backend::relay::RelayTarget + sdk::rtmp::RelayTarget'a dub_language: Option<String> eklendi (yönlendirme anahtarı). KASITLI KAPSAM DIŞI: buluttan mux (ayrı görev, backend'e video-ingest + core::rtmp'in paylaşılan crate'e çıkarılması gerektiriyor), uzun gecikmede orijinal mikrofona düşme (kullanıcı kararıyla elendi — sessizlik sürer), ducking/crossfade, dashboard'da mux durum göstergesi, OBS bağlantı kopma/yeniden bağlanma döngüsü (run_target'ın bugünkü tek-seferlik doğasıyla tutarlı). -->
- [x] Admin: yayıncı dil seçtiğinde o dil için hangi onaylı modelin kullanılacağının (kullanıcı, hedef dil) başına seçilmesi <!-- Claude 2026-07-06: voice_models::VoiceModelStore.set_active_for_language(model_id, lang) + POST /admin/voice/models/{id}/assign (AdminUser). Model onaylı değilse 422; single_to_single/multi_to_single zaten FARKLI bir dile atanmışsa 409 (bu modlar tek hedefe kilitli tasarım gereği — N hedef dil = N ayrı model); single_to_multi aynı anda birden fazla dile atanabilir (target_languages büyür). active_model_for(user_id,lang) admin modeli SONRADAN reddederse bile stale atamayı görmezden gelir (status=Approved filtresi, atama haritası ayrıca temizlenmez). /admin/voice-models sayfasına dil sütunu (language_mode+source→target özeti) + onaylı modeller için dil seçici+"Assign" butonu eklendi. -->
- [x] TTS kota/kredi tüketimi — gerçek ses baytından hesaplanan saniye, STT ile PAYLAŞILAN aynı `UsageTracker` havuzuna (kullanıcı kararı: "saniye ama müşteriye saat başına fiyatlandırma") <!-- Claude 2026-07-06: gateway::elevenlabs artık output_format="mp3_44100_128" AÇIKÇA istiyor (tahmini karakter/saniye oranı DEĞİL, kesin bilinen bitrate) — pipeline bytes*8/128_000 ile gerçek süreyi hesaplar, indirimsiz uygular (cloud sağlayıcı tam maliyetli — STT'nin process_audio'sundaki 1.0 tarife mantığıyla aynı). Ayrıca limit kontrolü YAPILMIYOR burada — aynı havuzu paylaştığı için STT'nin bir sonraki tick'i zaten yakalıyor (bir tick gecikmeli ama doğru, yeni bir "session kapatma" yolu icat edilmedi). Yerel TTS (tts_source=local) kullanım tüketimi KALAN — gerçek motor yok (2.2b), backend'e ses/karakter hiç ulaşmıyor, ölçülecek bir "TTS anı" henüz yok. -->

#### 2.2b TTS Kaynağı — Bulut (ElevenLabs) vs Yayıncı PC'si (Local XTTS-v2, Faz 3'ten taşındı)
> Kullanıcı kararı: XTTS-v2 yerel ses klonlama, STT'nin 2.6b'de kurduğu bulut/cihaz toggle desenine uyacak şekilde Faz 3'ten Faz 2'ye çekildi. Aynı mimari tekrar kullanılır: SDK'da opsiyonel yerel motor + donanım benchmark + watchdog + fallback, backend'de kaynak alanı + plan gating + kota teşviki, dashboard'da Bulut/Cihazında kartı.
- [x] Protokole `tts_source: "cloud" | "local"` alanı ekle (`stream_start`'a, `stt_source` ile aynı desen) — local modda SDK'nın kendi XTTS-v2 motoru metin+dili alıp klonlanmış sesi lokalde üretir, backend'e ses göndermez <!-- Claude 2026-07-06: ws/protocol.rs TtsSource enum (SttSource ile birebir aynı, serde default=cloud) + StreamStart.tts_source; pipeline::StreamSession alanı+getter olarak taşıyor. Gerçek dispatch (hangi yolun ne yaptığı) Faz 2.2'nin işi — burada yalnız protokol+taşıma var. -->
- [ ] SDK'ya gömülü local TTS motoru (XTTS-v2/Coqui, native bind) — model indirme + SHA-256 doğrulama (ses modeli STT'den büyük, ~2GB+) <!-- KALAN: whisper.cpp gibi native bind bu makinede cmake yokluğu nedeniyle ayrı oturuma bırakıldı. model_fetch.rs zaten jenerik (url+hash+dest) — TTS modeli için de doğrudan kullanılabilir, ek iş gerekmiyor. -->
- [x] Donanım kontrolü + benchmark (RTF/latency ölçümü) ve runtime watchdog — STT'deki `stt_watchdog` deseninin TTS'e uyarlanması (yavaşlarsa buluta fallback + histerezis) <!-- Claude 2026-07-06: sdk core local_tts.rs (LocalTtsEngine trait — synthesize(text)->PCM, MockTtsEngine, run_benchmark: RTF artık ÇIKTI ses uzunluğuna göre — STT'nin girdi uzunluğuna göre ölçmesinin tersi) + tts_watchdog.rs (stt_watchdog.rs ile birebir aynı deterministik karar makinesi, TtsRoute::Local/CloudFallback). 12 test. Gerçek motora bağlanan tts_control.rs (stt_control.rs'in fetch_policy/report_benchmark/poll döngüsü eşdeğeri) KALAN — gerçek XTTS-v2 motoruyla birlikte ayrı iş. -->
- [ ] Kullanıcının klonlanmış ses modeli local'de AES-256 ile şifreli saklanır, buluta hiç çıkmaz (gizlilik avantajı — pazarlama mesajı: "sesin hiç buluta gitmiyor") <!-- KALAN: gerçek native motor + model depolamasıyla birlikte. -->
- [x] `GET /api/tts/policy` — SDK'nın çektiği sunucu-taraflı yerel TTS politikası (izin, donanım eşikleri, kota çarpanı) — `stt/policy` ile aynı desen <!-- Claude 2026-07-06: backend/src/tts_policy — stt_policy ile birebir aynı yapı (Requirements/Watchdog/policy_for_plan/get_policy), tts.local.* config anahtarlarından okur (watchdog_p95_ms seed'i 2000ms — roadmap 2.2'nin "2 sn altı" hedefiyle aynı). PUT /api/tts/mode + POST /api/tts/benchmark: backend/src/sdk_link'e TtsMode/UserTtsSettings/TtsSettingsStore/set_tts_mode/report_tts_benchmark eklendi (STT karşılıklarıyla birebir aynı, ayrı depo — STT ve TTS modları bağımsız). Komut kuyruğu STT ile PAYLAŞILIYOR (KNOWN_COMMANDS'e sadece "tts_benchmark" eklendi — zaten jenerikti). 5+ backend e2e testi (tests/tts_link_api.rs) + ws/protocol.rs testleri. -->
- [x] Dashboard "Ses işleme" kartı: Bulut (ElevenLabs) / Cihazında (XTTS-v2, deneysel) seçimi, gereksinim tablosu, [Donanımımı test et] — STT'nin "Altyazı işleme" kartıyla aynı desen <!-- Claude 2026-07-06: tts-processing-panel.tsx (/dashboard/voice üstünde) — stt-processing-panel.tsx ile birebir aynı yapı, /api/tts/policy + /api/tts/mode + "tts_benchmark" komutuna bağlı. 9 sözleşme testi. -->
- [x] Admin: hangi planda hangi TTS kaynağının açık olduğu (`tts.local.plans` config anahtarı, `stt.local.plans` ile aynı desen) <!-- Claude 2026-07-06: config seed tts.local.plans="pro,streamer" (has_voice_clone ile aynı kilit). Özel admin panel UI'ı henüz yok — 2.6b'nin kendi notuyla aynı: "özel panel UI toggle'ı 1.6'daki admin toggle işiyle birlikte" (Faz 2.7, admin hibrit kontrol genişletme). -->
- [x] Yerel TTS fiyat teşviki: kota çarpanı (ElevenLabs karakter/dakika maliyeti yüksek — local seçilirse kota indirimli sayılır, `stt.local.usage_multiplier` deseniyle aynı) <!-- Claude 2026-07-06: config seed tts.local.usage_multiplier=0.5 + tts_policy::usage_multiplier() (aynı muhafazakâr 0..=1 clamp). Gerçek KULLANIM anında düşülmesi (tick_usage benzeri) Faz 2.2'nin gerçek zamanlı TTS pipeline'ı kurulunca anlamlı olacak — o olmadan düşülecek bir "TTS kullanım anı" yok. -->
> Not: SDK/backend zaten senkron istek-cevap mimarisinde; local TTS de "adımı bulut yerine lokalde çalıştır" mantığına uyar, protokolü bozmaz.

#### 2.3 İkili Yayın — SDK Relay'e Klonlanmış Ses Entegrasyonu
- [x] İkili yayın: TR ses → hesap 1, EN klonlanmış ses → hesap 2 (Faz 1.6'daki SDK relay üzerinden) <!-- Claude 2026-07-07: mekanizma çalışıyor — bkz. 2.2'deki "SDK'da video + klonlanmış ses birleştirme" maddesi (dub_language + cihazda mux motoru). Bir hedefe dub_language="en" atanırsa o hedef otomatik olarak run_dub_target'a (gerçek ses değişimi) yönlendirilir, diğerleri (örn. dub_language yok TR hesabı) değişmeden run_target'tan geçer — "TR→hesap1, EN dublaj→hesap2" tam olarak bu. Gerçek OBS + gerçek YouTube/Twitch/Kick ile canlı doğrulama KULLANICI AKSİYONU (Windows). KALAN: buluttan mux modu (dashboard'da henüz seçenek yok, tek çalışan yol cihazda). -->
- [ ] İkili yayın için upload bant genişliği uyarısı güncellemesi (≥25 Mbps öner)
> SDK gömülü RTMP relay motoru Faz 1.6'da kuruldu; klonlanmış sesle ikili yayın (cihazda mux) 2026-07-07'de eklendi.

#### 2.4 Twitch Extension
- [x] Twitch Extension policy ve uyumluluk dokümanı  <!-- Codex 2026-07-03: docs/twitch-extension-policy.md; Faz 2 öncesi policy gate, review checklist, EBS/JWT, audio, UGC, Bits, mobile ve submission kuralları -->
- [x] Uygulama planı (Faz 1 hazırlık / Faz 2 ayrımı)  <!-- Claude 2026-07-04: docs/twitch-extension-plan.md -->
- [x] Faz 2 hazırlık A1: overlay abone limiti + son N altyazı snapshot'ı (config-driven)  <!-- Claude 2026-07-04: backend/src/overlay, overlay.max_subscribers / overlay.snapshot_lines -->
- [x] Faz 2 hazırlık A2: channel_bindings modeli + migration (platform-bağımsız; Kick/Chrome ext aynı depoyu kullanır)  <!-- Claude 2026-07-04: backend/src/bindings + 0002_channel_bindings.sql -->
- [x] EBS backend: Twitch JWT doğrulama (rotasyonlu), kanal bağlama (kod-claim), caption state + canlı caption WS, per-viewer rate limit + per-channel viewer limiti, CORS  <!-- Claude 2026-07-04: backend/src/ext_twitch (+jwt.rs), TWITCH_EXT_SECRET(+_PREVIOUS), ext.twitch.* config (varsayılan KAPALI), 10 e2e test (tests/ext_twitch_api.rs). Overlay token viewer'a asla dönmez — sunucu içinde çözülür -->
- [x] Component overlay extension  <!-- Claude 2026-07-04: /extension paketi — bundler'sız human-readable ES modules (viewer.html + js/), textContent-only render, WS + poll fallback, backoff, 11 node --test birim testi -->
- [x] İzleyici dil seçim butonu  <!-- Claude 2026-07-04: viewer bar'da dil select (backend snapshot dillerinden, allowlist regex) + aç/kapat + S/M/L + durum göstergesi; tercih anonim localStorage -->
- [x] Broadcaster config sayfası + dashboard kartı  <!-- Claude 2026-07-04: extension/config.html (6 haneli kodla bağlama, yalnız public değerler Twitch config service'e) + /dashboard/platforms kartı. Kart CANLI: adminFetch ile kod üretme/binding/unlink (V11 emsali — admin bootstrap oturumu; 1.4 user backend'i gelince yalnız token kaynağı değişir) -->
- [ ] Twitch console kaydı + Hosted Test (CSP/wss doğrulaması, mobile, slow network)  <!-- ext.strovak.com DNS + Twitch developer console insan adımları; bkz. infra/deploy.md §7 -->
- [ ] Twitch review başvurusu (erken başla — 2–4 hafta)  <!-- Hosted Test bitmeden gönderilmez; walkthrough + changelog + test hesapları (plan A5) -->

#### 2.5 Chrome Extension — Kick ve Evrensel
- [ ] Manifest V3
- [ ] Chrome Web Store başvurusu

#### 2.6 faster-whisper Geçişi (bulut)
- [ ] GPU'lu VPS/VDS sunucu (sağlayıcıdan bağımsız — Hetzner/OVH/Vast.ai/kendi donanımı; "Hetzner GPU sunucu" maddesi 2026-07-08 kullanıcı kararıyla genelleştirildi) <!-- Claude 2026-07-08: backend/src/stt_servers (SttServerStore — RelayTargetStore şablonu, ssh-keygen'e shell-out ile ed25519 keypair üretimi, private key/API token AES-256-GCM şifreli, hiçbir JSON yanıtında düz metin dönmez) + provision.rs (Provisioner trait + FakeProvisioner — durum makinesi pending→provisioning→ready/error) + admin route'ları (POST/GET /admin/stt-servers, GET/DELETE /admin/stt-servers/{id}, POST /admin/stt-servers/{id}/provision — 202 + tokio::spawn arka plan). gateway/selfhosted_stt.rs: SttProvider impl'i SttServerStore::active_ready()'yi her çağrıda okur, "selfhosted" adıyla kayıtlı. stt-server/: faster-whisper (CTranslate2 large-v3-turbo, docs/local-stt-plan.md §2 kararıyla aynı model) saran FastAPI servisi + Dockerfile (NVIDIA CUDA runtime, healthcheck) + CI (pytest + GHCR image build/push). infra/stt-server/bootstrap.sh: idempotent kurulum script'i (nvidia-smi şartı, Docker+nvidia-container-toolkit, API token üretimi, docker compose up, /health poll, sonuç JSON'u). 19 yeni backend testi (unit + e2e) + 7 pytest, hepsi yeşil. KALAN: gerçek SshProvisioner (ssh/scp shell-out + TOFU fingerprint pinleme — FakeProvisioner'ın yerini alacak) + admin frontend paneli ("STT Sunucuları" — sunucu ekle/public key göster/kur/sil) + gerçek bir GPU kutusunda canlı smoke test. -->
- [ ] TR dil kalite testi <!-- gerçek bir GPU VPS/VDS'te SshProvisioner ile kurulum bitmeden koşulamaz; bkz. yukarıdaki madde -->
- [x] Admin toggle: Deepgram → Whisper <!-- Claude 2026-07-08: yeni bir config anahtarı gerekmedi — mevcut PUT /admin/config zaten stt.provider'ı whitelist'ten günceller (config.rs:69 seed "deepgram"), "selfhosted" da aynı whitelisted key'in geçerli bir değeri; Gateway routing/fallback zinciri (stt.provider/stt.fallback) değişmedi. -->

#### 2.6b STT Kaynağı — Bulut vs Yayıncı PC'si (Faz 1'den taşındı)
- [x] Protokole `transcript` mesajı ekle (SDK → Backend): local STT kullanan SDK, ses yerine zaten yazıya dökülmüş metni gönderir <!-- Claude 2026-07-05: backend ws/protocol.rs ClientMessage::Transcript{session_id,text,is_final(default true)} + ErrorCode::LocalSttRejected(4007); sdk core/cloud.rs Transcript + send_transcript(); docs/websocket-protocol.md transcript bölümü + 4007 tablo satırı. -->
- [x] `stream_start`'a `stt_source: "cloud" | "local"` alanı ekle — hangi modda çalışıldığı backend'e bildirilir <!-- Claude 2026-07-05: SttSource enum (serde default=cloud → eski SDK'lar alansız gönderir, geriye dönük uyumlu); sdk core/stream.rs StreamConfig.stt_source. Bilinmeyen değer parse hatası (testli). -->
- [x] Pipeline'da iki yolu birleştir: `cloud` → audio frame → STT gateway → çeviri; `local` → transcript mesajı → STT adımı atlanır, direkt çeviri. İkisi de aynı `Subtitle` çıktısını üretir <!-- Claude 2026-07-05: pipeline StreamSession.process_transcript (usage tick + çeviri + Subtitle, ortak tick_usage/subtitle_for refactor'ü); ws send_for_transcript + publish_and_send (overlay yayını + plan-limit kapanışı iki yolda ortak). Kurallar: session'sız transcript→4002, cloud session'dan transcript→4007 stt_source_not_local (plan gating baypas edilemez), local session'da ses frame'i de geçerli (bulut fallback, sequence paylaşılır), boş metin atlanır. 5 pipeline unit + 3 ws e2e testi. -->
- [ ] SDK'ya gömülü local model (faster-whisper/whisper.cpp) — dashboard'dan "Bulut STT" / "Yerel STT (deneysel)" seçimi, donanım (GPU) kontrolü + uyarı <!-- Sunum/gereksinim/fiyat kararları: docs/local-stt-plan.md (2026-07-05) — NVIDIA-only ilk sürüm, large-v3-turbo, etkinleştirme benchmark'ı (RTF≤0.5) + runtime watchdog (p95>1.5sn→bulut fallback, 5dk histerezis), model CDN'den indirilir (~1.5GB, SHA-256). ÇEKİRDEK HAZIR (Claude 2026-07-05): sdk core local_stt (LocalSttEngine trait + run_benchmark + MockEngine), stt_watchdog (deterministik karar makinesi: 3 ardışık yavaş / pencere p95 / 5dk histerezis+temiz pencere — 7 test), stt_control (fetch_policy/poll_commands/report_benchmark/handle_pending_commands — gerçek backend'e karşı 2 e2e: panel→SDK→policy tam döngüsü). GPU/VRAM YOKLAMA HAZIR (Claude 2026-07-06): sdk core gpu_probe — nvidia-smi --query-gpu üzerinden isim+toplam VRAM parse'ı (saf parser + gerçek nvidia-smi'ye karşı best-effort test, bu makinede gerçek RTX 3070 ile doğrulandı); binary yoksa/parse edilemezse HardwareInfo::default() (donanım şartı sağlanmadı sayılır, hata değil). MODEL İNDİRME+SHA-256 HAZIR (Claude 2026-07-06): sdk core model_fetch — ensure_model() .part dosyasına stream eder + SHA-256 doğrular, eşleşmezse .part silinir ve ChecksumMismatch döner (dest'e asla kötü dosya taşınmaz), eşleşirse rename; dest'te zaten doğru hash'li dosya varsa indirme atlanır (hash hesap spawn_blocking'te — 1.5GB'lık dosya async executor'ı bloklamaz). Mock axum sunucusuna karşı 5 test (başarı, checksum mismatch, http 404, cache-skip indirme-sayaçsız, bozuk dosya yeniden indirilir). TRAY POLL WIRING HAZIR (Claude 2026-07-06): sdk core stt_control::run_polling_loop — sabit aralıkla handle_pending_commands çağırır (her tick'te gpu_probe::probe() ile taze donanım), hata/ağ kopması logla+yut (loop asla ölmez, sıradaki tick'te tekrar dener). run_desktop_shell yeni `Option<SttControlHandle>` parametresi alır — Some ise tokio::spawn ile arka planda başlar. main.rs şimdilik None geçiyor (gerçek motor yok — MockEngine'i prod'a bağlamak sahte donanım-testi sonucu göstermek olurdu); SttControlHandle{engine, sample, poll_interval} gerçek motor gelince main.rs'te doldurulup Some(...) yapılacak, loop'un kendisi ve wiring'i değişmeyecek. 2 e2e test (stt_control_loop.rs): loop başladıktan SONRA bırakılan komutu bir sonraki tick'te işliyor, ulaşılamayan backend loop'u öldürmüyor. KALAN: yalnız whisper.cpp/GGML native bind (LocalSttEngine gerçek impl — bu makinede cmake yok, ayrı oturuma bırakıldı) + gerçek Windows+RTX canlı smoke (user). -->
- [x] Yerel STT fiyat teşviki: kota çarpanı — transcript yolunda elapsed × `stt.local.usage_multiplier` (seed 0.5), ses/fallback yolu 1× (yol bazlı → "yerel seçip buluttan hizmet alma" kendiliğinden kapalı) <!-- Claude 2026-07-05: pipeline tick_usage(scale) — process_audio 1.0, process_transcript ws'ten okunan çarpanla; local_usage_multiplier helper'ı bozuk/aralık-dışı değerde muhafazakâr 1.0'a düşer (0..=1 clamp). Deterministik testler: backdate_last_tick ile 10sn×0.5→5sn, ses 10sn→10sn; config fallback testi. Policy config anahtarları (min_vram_mb vb.) policy endpoint task'ında. -->
- [x] `GET /api/stt/policy` — SDK'nın çektiği sunucu-taraflı yerel STT politikası (izin, çarpan, donanım eşikleri, watchdog); eşikler SDK güncellemeden panelden ayarlanır <!-- Claude 2026-07-05: backend/src/stt_policy — policy_for_plan (local_allowed=plan_allowed, usage_multiplier ortak fonksiyon, min_vram_mb/benchmark_rtf_max/watchdog_p95_ms/retry_minutes/max_chars seed'li, bozuk config→seed varsayılanı). tests/stt_policy_api.rs: seed yanıtı, 401, restart'sız config değişimi yansıması. -->
- [x] SDK komut kanalı + ayar senkronu — SDK tamamen arka planda (tray), tüm kumanda web panelde: panel komut bırakır (`POST /api/sdk/commands`), SDK yoklar (`GET`, pairing token, tek seferlik teslim), benchmark sonucunu raporlar (`POST /api/sdk/benchmark`); STT mod tercihi panelden yazılır SDK okur — bkz. docs/local-stt-plan.md §2/§5 <!-- Claude 2026-07-05: backend/src/sdk_link — SdkCommandQueue (bellek içi, whitelist=benchmark, 422 bilinmeyen komut) + SttSettingsStore (STT_SETTINGS_FILE düz JSON — secret yok; persist+reload testli) + PUT /api/stt/mode (local: plan izni 403 + geçen benchmark şartı 409; cloud şartsız). GET /api/stt/policy artık mode+benchmark da döner (SDK tek çağrı). .env.example+compose. 4 unit + 5 e2e test. -->
- [x] Dashboard "Altyazı işleme" kartı: Bulut / Cihazında açık seçimi, gereksinim tablosu, [Donanımımı test et] sonucu, kota avantajı metni ("kotan yarı hızda tükenir"); fallback dipnotu zorunlu <!-- Claude 2026-07-05: frontend stt-processing-panel.tsx (/dashboard/captions üstünde) — GET /api/stt/policy canlı; iki mod kartı (Cloud=Default, On your device=Experimental rozet); local seçimi plan izni + geçen benchmark olmadan kilitli; Test my hardware → POST /api/sdk/commands + Check result yenilemesi; çarpan/vram policy değerlerinden şablonla doldurulur; fallback dipnotu sabit. stt-processing.test.ts 9 sözleşme testi (endpoint sabitleri + zorunlu dipnot + emoji yasağı). 89 test + lint + tsc + build yeşil. -->
- [x] Admin: hangi planda hangi STT kaynağının açık olduğu (config-driven) <!-- Claude 2026-07-05: stt.local.plans config anahtarı (seed: tüm planlar — lokal STT bulut maliyetini düşürür; PUT /admin/config ile restart'sız daraltılır), stream_start stt_source=local'da plan_allowed enforce edilir (izinsiz → 4007 local_stt_not_allowed, cloud başlangıç etkilenmez). Ek koruma: stt.local.max_chars=500 (çeviri API maliyeti; aşım → 4007 transcript_too_long, session açık kalır). integration.<provider>.plans ile aynı desen; özel panel UI toggle'ı 1.6'daki admin toggle işiyle birlikte. -->
> Yayıncıdan yayıncıya donanım farkı riski nedeniyle Faz 1'den çıkarıldı, Faz 2'ye alındı. Local STT, mevcut senkron istek/cevap mimarisini BOZMAZ — çünkü yerel model de VAD ile bölünmüş bitmiş cümleyi gönderiyor (Deepgram'ın gerçek streaming WS'i gibi sürekli açık bağlantı gerektirmiyor); sadece "STT adımını bazen atla" mantığı eklenir.

#### 2.7 Admin — Hibrit Kontrol Genişletme
- [ ] STT toggle (Deepgram ↔ Whisper ↔ SDK local)
- [ ] TTS toggle (ElevenLabs ↔ Cartesia ↔ SDK local/XTTS-v2)
- [ ] Canlı maliyet göstergesi

#### 2.8 SDK Dağıtım — Gerçek İndirme Akışı
> `/dashboard/sdk` şu an statik mockup: indirme butonu hiçbir URL'e gitmiyor, pairing kodu ve stream key sabit placeholder metin (config'ten, canlı API'ye bağlı değil). Faz 2'de yayıncı gerçekten SDK'yı indirip eşleştirebilmeli.
- [x] Release CI pipeline: `tauri build` → code sign → imzalı `latest.json` yayınla (bkz. `docs/sdk-distribution.md` "Yapılacaklar") <!-- Claude 2026-07-07: .github/workflows/sdk-release.yml — sdk-v* tag push'ta Windows/macOS/Linux matrix, tauri-action ile build→sign→GitHub Release. KALAN: gerçek bir tag push/release koşumu bu oturumda tetiklenemedi (push gerekir), YAML sadece izole bir venv'de pyyaml ile syntax/yapı doğrulandı. -->
- [ ] Gerçek Windows Authenticode (OV/EV) + macOS notarization sertifikaları (CI wiring hazır — PFX import + `--config` thumbprint mekanizması ve APPLE_* env'leri — ama gerçek sertifika yok, hiç denenmedi — 1.3/SDK doğrulama listesinden)
- [ ] `releases.strovak.com` statik host kurulumu (S3/Cloudflare) — build artefaktları + `latest.json` <!-- Claude 2026-07-07: interim olarak indirme linkleri GitHub Releases'e gidiyor (NEXT_PUBLIC_SDK_RELEASES_BASE, aşağıya bkz.); custom domain hâlâ ops task'ı. -->
- [ ] `tauri-plugin-updater` runtime init (`run_desktop_shell()` içine wiring)
- [x] `/dashboard/sdk` indirme butonunu gerçek release URL'ine bağla (platforma göre: Windows .msi/.exe öncelik, macOS/Linux "yakında" durumu CLAUDE.md'ye göre korunur) <!-- Claude 2026-07-07: KARAR DEĞİŞTİ — kullanıcı isteğiyle macOS/Linux artık "yakında" değil, gerçek cpal capture'ı (yukarı bkz. 1.3) yansıtarak "enabled". src/lib/sdk-releases.ts: NEXT_PUBLIC_SDK_RELEASES_BASE'den platform başına sabit isimli installer URL'i üretir (CI'nin her release'te yüklediği sabit-isimli asset'lerle eşleşir); env boşsa hayali URL'e düşmez, "yapılandırılmadı" gösterir. Üç platformun butonu da artık gerçek <a href>, dekoratif değil. -->
- [x] Pairing kodu üretme butonunu gerçek `POST /api/pairing/code`'a bağla (backend endpoint 1.3/1.4'te hazır, frontend hâlâ statik kod gösteriyor) <!-- Claude 2026-07-06: sdk-pairing-panel.tsx — userFetch("/api/pairing/code", POST), kod+geçerlilik süresi (dk) gösterir, 401/ağ/genel hata durumları ayrı. -->
- [x] Stream key alanını gerçek `GET/PUT /api/relay/targets`'a bağla (backend hazır — bkz. Faz 1 "Dashboard/Backend Wiring" listesi — frontend hâlâ config'ten statik değer okuyor) <!-- Claude 2026-07-06: sdk-stream-key-panel.tsx — GET ile mevcut "obs" hedefini okur (platform sabiti — YouTube/Twitch/Kick OAuth hesap bağlama henüz kurulmadığından tek hedef), yoksa istemci tarafında rastgele anahtar üretip PUT ile kaydeder; Regenerate aynı akışı tekrarlar. Ayrıca copy-row.tsx'teki "Copy" butonu hiçbir şey yapmıyordu (dekoratifti) — artık navigator.clipboard.writeText çağırıyor + "Kopyalandı" durumu gösteriyor. -->
- [x] İndirme sonrası ilk-kurulum akışı: SDK açılışta pairing kodu ister, dashboard'daki kodu girince otomatik eşleşir (kullanıcı testi) <!-- Claude 2026-07-07: sdk/ui + commands.rs::submit_pairing_code — SDK'nın kendi penceresinde kod girilince var olan POST /api/pairing/claim'e gönderilir, token AES-256-GCM ile yerel diske yazılır, shell otomatik başlar. KALAN: gerçek dashboard + gerçek SDK binary ile uçtan uca bir insanın denemesi (bu oturumda derleme/test/cargo-run ile doğrulandı, tam kullanıcı akışı değil). -->

#### 2.9 Login/Signup — Google/Twitch OAuth
> Yayıncı sadece e-posta/şifreyle değil, mevcut YouTube (Google) veya Twitch hesabıyla da giriş/kayıt olabilmeli.
- [x] Backend: Google + Twitch OAuth login (ayrı `client_id`/`client_secret` — mevcut YouTube captions/Twitch Extension entegrasyonlarından bağımsız) <!-- Claude 2026-07-07: backend/src/auth/oauth_login.rs — integrations::oauth çekirdeğini (authorize URL, CSRF state, token exchange) reuse eden ayrı registry. GET /api/auth/oauth/{provider}/start|callback. UserRecord.password_hash: String → Option<String> + oauth_provider/oauth_external_id (migration 0004). E-posta çakışması OTOMATİK BAĞLAMA YAPMADAN reddedilir (hesap ele geçirme riski sıfır — kullanıcı kararı). 21 yeni test, hepsi mock-server ile (gerçek Google/Twitch hesabıyla hiç denenmedi — GOOGLE_LOGIN_CLIENT_ID/SECRET, TWITCH_CLIENT_ID/SECRET admin panelden girilmeli). -->
- [x] Frontend: `/login` + `/signup` sosyal butonlar + `/login/oauth-complete` <!-- Claude 2026-07-07: oauth-buttons.tsx (düz <a href>, backend'e doğrudan), oauth-complete.tsx (URL fragment'ından token okur, setUserToken). /signup'a eklendi; /login'e de eklendi ama dosyadaki pre-existing bir legal-note değişikliğiyle satır satır iç içe geçtiği için o dosya bu oturumda commit'lenmedi (kullanıcı kendi legal-pages işiyle birlikte commit'lemeli). -->
- [ ] Kick login — OAuth 2.1 + PKCE gerektiriyor (mevcut `exchange_code` sadece client_secret akışını biliyor, ek plumbing lazım) ve endpoint'leri (id.kick.com/api.kick.com) canlı doğrulanmadı; kullanıcı kararıyla bu pass'in dışında bırakıldı, ayrı bir task
- [ ] OAuth-bağlı hesabı sonradan var olan şifreli hesaba bağlama (settings'ten) — e-posta çakışması şu an sadece reddediliyor, "linkleme" akışı yok
- [ ] Gerçek bir Google/Twitch hesabıyla uçtan uca canlı test (şu an sadece mock-server testli)

**Faz 2 tamamlandığında:**
İzleyici kendi dilini seçiyor. Yayıncı kendi sesiyle başka dilde konuşuyor (bulutta ElevenLabs veya cihazında XTTS-v2). Çeviri DeepL'in tam dil matrisiyle çalışıyor. STT maliyeti $1,032 → $80/ay. Yayıncı SDK'yı dashboard'dan indirip tek pairing koduyla eşleştirebiliyor.

---

## Faz 3 — Tam Bağımsızlık: Local AI + Premium Bulut Multistream

**Hedef:** API bağımlılığını kır, tek paket sun, maliyet %82 düşür  
**Mesaj:** "Artık ElevenLabs'a da, DeepL'e de, harici servise de ihtiyacın yok"  
**Stack:** XTTS-v2 + HY-MT1.5 local + premium bulut multistream (opsiyonel)

### Görevler

#### 3.1 Bulut Multistream — MediaMTX (Premium opsiyon)
- [ ] Hetzner dedicated (1 Gbps) — sadece premium/zayıf internet için
- [ ] MediaMTX kurulum ve konfigürasyon
- [ ] Yayıncının interneti yetmezse video buluttan dağıtılır
- [ ] Yayıncı dashboard'unda "Bulut multistream" premium seçeneği
- [ ] Admin: SDK relay ↔ bulut multistream yönlendirme
> Not: Çoğu yayıncı SDK relay (Mod B) kullanır, bu ücretsizdir. Bulut multistream sadece interneti zayıf olanlar için premium opsiyon.

#### 3.2 XTTS-v2 Self-Hosted
- [ ] Hetzner GPU'da kurulum
- [ ] 17 dil testi (TR dahil)
- [ ] Admin toggle: ElevenLabs → XTTS-v2
- [ ] ElevenLabs yalnızca Streamer plan premium fallback

#### 3.3 HY-MT1.5 Çeviri Geçişi
- [ ] HY-MT1.5-7B local kurulum
- [ ] Dil bazlı yönlendirme (local önce, DeepL fallback)

#### 3.4 YouTube Multi-Audio
- [ ] Tek hesaptan 6 dile ses kanalı

#### 3.5 İkili Yayın İyileştirme (SDK relay zaten Faz 2'de)
- [ ] Kick AI içerik etiketi otomasyonu
- [ ] İkili yayın kalite/senkron optimizasyonu (TR ve EN ses gecikme eşitleme)
> İkili yayın altyapısı Faz 2'deki SDK RTMP relay'de kuruldu; burada iyileştiriliyor.

#### 3.6 Admin — Maliyet Optimizasyon
- [ ] API vs local maliyet karşılaştırma grafiği
- [ ] Otomatik fallback kuralları
- [ ] Sunucu bant genişliği monitörü

**Faz 3 tamamlandığında:**
Yayıncı tek abonelikle her şeyi alıyor. Harici servise ihtiyaç yok. Maliyet $2,332 → $310/ay.

---

## Faz 4 — Büyüme

### Görevler

#### 4.1 10 Dil Tam Desteği
- [ ] TR, EN, ES, DE, FR, PT, JP, KO, AR, RU

#### 4.2 Yayıncı Dashboard — Gelişmiş Analitik
- [ ] İzleyici dil ve platform dağılımı
- [ ] Haftalık email raporu

#### 4.3 Admin — Tam Operasyon
- [ ] Churn analizi
- [ ] Otomatik ölçekleme
- [ ] Gelir tahmini

#### 4.4 SDK Mac + Linux Aktivasyonu
- [ ] macOS ses adaptörü (CoreAudio) aktive et + notarization
- [ ] Linux ses adaptörü (PulseAudio/PipeWire) aktive et
- [ ] Üç platformda test ve dağıtım
> Çekirdek Faz 1'de platform-bağımsız yazıldığı için bu faz sadece ses adaptörlerini aktive etmek + test.

#### 4.5 Oyun Jargonu
- [ ] Dashboard'dan özel terim yönetimi
- [ ] Kategori bazlı jargon paketleri (FPS, MOBA, IRL)

#### 4.6 Mobil Uygulama
- [ ] iOS ve Android
- [ ] IRL yayın desteği

---

## Platform Uyumluluk

| Özellik | YouTube | Twitch | Kick |
|---|---|---|---|
| OBS altyazı overlay | ✅ | ✅ | ✅ |
| İzleyici dil seçimi | ✅ Native CC | ✅ Extension | ✅ Chrome Ext |
| Ses klonlama | ✅ Multi-audio | ⚠️ İkili hesap | ⚠️ İkili hesap |
| Tek hesap yeterli mi? | ✅ | ⚠️ 2 hesap | ⚠️ 2 hesap |
| ToS uyumu | ✅ | ✅ | ✅ (AI etiketi) |

---

## Tahmini Maliyet Evrimi (1000 kullanıcı/ay)

> Not: Multistream maliyeti senin tablonda yok — video yayıncının kendi internetinden dağıtılır (SDK relay) veya yayıncı kendi Streamlabs aboneliğini kullanır. Sen sadece sesi işliyorsun.

### Faz 1 — Full API (sadece altyazı)

| Servis | Maliyet |
|---|---|
| Deepgram STT | $1,032 |
| DeepL Çeviri | $150 |
| Sunucu + Vault | $80 |
| **Toplam** | **~$1,262/ay** |

### Faz 2 — Hibrit + Ses Klonlama

| Servis | Maliyet |
|---|---|
| faster-whisper GPU | $80 |
| DeepL (azalan) | $100 |
| ElevenLabs TTS | $600 |
| Sunucu (GPU + genel) | $150 |
| **Toplam** | **~$930/ay** |

### Faz 3 — Tam Hibrit (local AI)

| Servis | Maliyet |
|---|---|
| faster-whisper GPU | $80 |
| HY-MT1.5 + DeepL fallback | $30 |
| XTTS-v2 GPU | $80 |
| Sunucu (genel) | $80 |
| **Toplam** | **~$270/ay** |

**Faz 1 → Faz 3 düşüş: %79**

> Bulut multistream (premium opsiyon) kullanılırsa Hetzner Gbit ~$65/ay eklenir — ama bu sadece interneti zayıf yayıncılar için ve onlardan premium ücret alınır.

---

## Bant Genişliği Gerçekçi Hesabı

```
## Bant Genişliği — Kimde, Ne Kadar?

**Senin sunucunda (sadece ses):**
```
1 yayıncı sesi = ~50-128 Kbps (çok küçük)
1000 eşzamanlı yayıncı = ~128 Mbps
→ Standart sunucu yeterli, Gbit gerekmez
```

**Yayıncının internetinde (video dağıtım):**
```
Tek platform 1080p = ~6 Mbps upload
İkili yayın (2 platform) = ~12 Mbps upload
3 platform = ~18 Mbps upload
→ Yayıncıya "≥25 Mbps upload öneriyoruz" denir
```

**Premium bulut multistream (opsiyonel):**
```
Sadece interneti zayıf yayıncılar için
Bu durumda video buluta çıkar → Hetzner Gbit (~$65/ay)
Bu yayıncılardan premium ücret alınır
```

**Kritik fark:** Video yayıncının kendi internetinden gittiği için senin Gbit RTMP sunucusu maliyetin yok. Sadece GPU sunucu (ses klonlama) gerekiyor.

---

## Notlar

- Twitch Extension onay süreci 2–4 hafta; Faz 2 başında başlanmalı.
- Kick ses klonlamada AI içerik etiketi zorunlu (ToS).
- İkili yayın için aynı IP'den iki hesap ban riski; SDK farklı bağlantı/profil yönetebilmeli.
- Faz 1'de SDK gömülü RTMP relay asıl dağıtım yöntemidir; yayıncı isterse Streamlabs'ı da opsiyonel bağlayabilir.
- Free plan ile kitle çekilmeden Pro planı zorlamak churn'ü artırır.
- XTTS-v2 ElevenLabs'a kıyasla ~%10–15 kalite farkı; kullanıcıya şeffaf olunmalı.
- Admin hibrit kontrol her fazda birlikte geliştirilmeli.
- SDK temelden çok platformlu (Tauri+Rust) yazılır; Windows önce çıkar, Mac/Linux ses adaptörü hazır bekler, sonradan zorlanmadan aktive edilir.
- Video yayıncının kendi internetinden dağıtılır (SDK relay) → senin Gbit sunucu maliyetin yok.
- Buluta sadece ses gider (~50-128 Kbps); video lokal kalır.
- VAD lokalde olduğu için boş ses buluta gitmez → STT maliyeti ~%30–40 düşer.
- Ses klonlama buluttadır; yayıncının güçlü GPU'su olması gerekmez.
- AES-256 ses akışı ve Vault key yönetimi Faz 1'de kurulmalı, sonraya bırakılmamalı.
- Code signing SDK için şart (Windows SmartScreen + macOS notarization).
- Ses modelleri kullanıcı bazlı şifreli; hesap silinince kalıcı silinmeli (KVKK/GDPR).
- Bulut multistream sadece interneti zayıf yayıncılar için premium opsiyon.

---

## Coming Soon — Palabra Benzeri Çözüm Kataloğu

> Bu bölüm Faz 1 kapsamına alınmaz. Frontend'de bu çözümler aktif ürün gibi gösterilmez; admin panelden yönetilen `coming_soon`, `waitlist`, `disabled` veya `beta` durumlarıyla gösterilir. Metinler, logolar, sıralama, CTA, feature flag ve görünürlük DB/config kaynaklı olmalıdır; frontend'e hard-coded gömülmez.

### Toplantı ve Webinar Çözümleri

- [ ] Online Meetings Translation — Zoom, Teams ve Meet gibi toplantılarda gerçek zamanlı altyazı/çeviri deneyimi
- [ ] Zoom Translation — Zoom toplantıları için StreamDubber canlı çeviri modu
- [ ] MS Teams Translation — Microsoft Teams toplantıları için canlı çeviri modu
- [ ] Google Meet Translation — Google Meet toplantıları için canlı çeviri modu
- [ ] Webinar Translation — webinar ve online eğitim yayınlarında altyazı/çeviri

### Yayın ve Etkinlik Çözümleri

- [ ] Live Stream Translation — mevcut Faz 1 yayıncı ürünüyle uyumlu ana çözüm sayfası
- [ ] Events and Conferences — fiziksel/hibrit etkinlik ve konferans çeviri çözümü
- [ ] Watch Party Mode — izleyici taraflı, herhangi bir tarayıcı sekmesini (Netflix, YouTube, Twitch, canlı spor) canlı çeviren mod; detaylı analiz ve uygulama planı aşağıda "Rakip Analizi — Watch Party Mode" bölümünde

### Platform ve Geliştirici Çözümleri

- [ ] API for Developers — canlı STT/çeviri/TTS pipeline'ı için developer API ürünü
- [ ] Text-to-Speech / TTS — ses üretimi ve ileride ses klonlama ürün yüzeyi

### Sektörel Çözüm Sayfaları

- [ ] Churches — ibadet/topluluk yayınları için canlı çeviri
- [ ] Education — ders, kurs ve akademik etkinlikler için canlı çeviri
- [ ] Nonprofit — uluslararası topluluk ve bağış etkinlikleri için canlı çeviri
- [ ] Broadcasting — yayıncı, medya ve prodüksiyon ekipleri için canlı çeviri
- [ ] International Companies — global şirket toplantıları ve townhall'lar için canlı çeviri

> Faz 2/3 katalog NOT (Claude, 2026-07-05): Yukarıdaki 9 kalemin hepsi artık
> `/solutions`'da statik `coming_soon`/`waitlist` kart olarak görünüyor
> (`en.json` `solutions.items`, bkz. `docs/overlay-builder-plan.md` §6).
> Codex 2026-07-05: kullanıcı isteğiyle public `/solutions` kartlarındaki
> `Notify me` formları kaldırıldı; `POST /api/waitlist` backend'i ve
> Postgres tablosu ileride hesaplı/admin akıştan DB'ye yazmak için korunuyor.
> KAPSAM BİLİNÇLİ SINIRLANDI (kullanıcı onayıyla): bu satırlar hâlâ `[ ]` —
> ayrı sektörel sayfalar, `ui.public.solutions.catalog` DB/admin-config sistemi,
> Zoom/Teams/Meet marka asset'leri hiçbiri bu işin parçası değil; sadece
> "tanıtım" yapıldı, aşağıdaki "Frontend Coming Soon Kuralları"
> tam anlamıyla henüz karşılanmıyor.

### Frontend Coming Soon Kuralları

- [ ] Çözüm kataloğu `ui.public.solutions.catalog` config'inden render edilir.
- [ ] Her çözümün `status` alanı olur: `coming_soon`, `waitlist`, `beta`, `enabled`, `disabled`.
- [ ] Logo/ikon asset referansları config'ten gelir; Zoom/Teams/Google Meet marka kullanımı için marka kılavuzları kontrol edilmeden asset commit edilmez.
- [ ] Faz 1 dışı çözümler dashboard'da satın alınabilir/aktif özellik gibi gösterilmez.
- [ ] Coming soon kartlarında CTA admin-config kaynaklı olur: bekleme listesi, demo talebi veya görünmez.
- [ ] Admin panelden çözüm sırası, görünürlük, metin, CTA, hedef route ve feature flag değiştirilebilir.

---

## Rakip Analizi — Watch Party Mode (streamtranslate.live)

> Kaynak: `streamtranslate.live/watch-party-info` (2026-07-02 itibarıyla). Bu bölüm sadece analiz + uygulama planıdır — Faz 1 kapsamına girmez, frontend'e şimdi eklenmez (kullanıcı talebiyle roadmap-only).

### Rakip Ne Sunuyor
- **Ne yapıyor:** Tarayıcı sekmesindeki HERHANGİ bir sesi (Netflix, YouTube, Twitch, canlı spor, podcast, haber, ders) canlı çeviriyor — yayıncıya değil, **izleyiciye** yönelik bir mod.
- **Kullanıcı akışı:** `/control` sayfasına giriş → "Watch Party" aç → "GO LIVE" → paylaşılacak sekmeyi seç (3 adım).
- **Teknik:** Uzantı/kurulum gerekmiyor — tarayıcının yerleşik ekran/sekme paylaşım API'si (`getDisplayMedia`) kullanılıyor. Video akışı değil, **çıkış (render edilmiş) ses** yakalandığı için DRM korumalı içerikte (Netflix) de çalışıyor. <2sn gecikme, 30+ dil.
- **Fiyatlandırma:** Sadece en üst plan (**Elite, $149/ay**) üzerinde açık — premium/yüksek-fiyat kilitli özellik.

### Bizim Ürünle Farkı — Kritik Nokta
Faz 1-4'teki StreamDubber tamamen **yayıncı taraflı** bir üründür (SDK, RTMP relay, OBS, platform OAuth). Watch Party ise **izleyici taraflı** ayrı bir dikey: yayıncının hiçbir katılımı/kurulumu olmadan, herhangi bir kullanıcı herhangi bir sekmeyi kendi başına çevirebiliyor. Bu, StreamDubber'ın "yayıncı öder" iş modeline **ek bir bacak** (B2C, kurulumsuz, tek kullanıcı) — SDK'ya hiç ihtiyaç yok.

### Nasıl Yapılabilir — Mevcut Mimariyle Örtüşme

**Büyük avantaj: bulut işleme katmanı zaten var, sıfırdan yazmaya gerek yok.**

| İhtiyaç | Watch Party için gereken | Mevcut StreamDubber'da karşılığı |
|---|---|---|
| Ses yakalama | `navigator.mediaDevices.getDisplayMedia({video:true, audio:true})` (Chrome: "sekme sesini paylaş" seçeneğiyle) — saf tarayıcı JS, kurulum yok | SDK'daki `core/capture.rs` yerine tamamen web-native; SDK'ya gerek yok |
| Ses → buluta akış | WebSocket üzerinden ses frame'leri | `backend/src/ws` + mevcut ses akışı protokolü (`docs/websocket-protocol.md`) — **aynen yeniden kullanılabilir** |
| VAD / ön-işleme | Boş sesi filtrele | `sdk/src/core/vad.rs`, `preprocess.rs` mantığı — tarayıcıda WASM'a taşınabilir veya bulutta STT öncesi uygulanabilir |
| STT + Çeviri | Gerçek zamanlı metne çevirip hedef dile çevirme | `backend/src/gateway` (Deepgram/AssemblyAI + DeepL/Google) — **birebir aynı pipeline** |
| Altyazı gösterimi | Kullanıcının ekranında overlay/widget | `backend/src/overlay` (OverlayHub, WS push) + `/o/[token]` sayfası — **aynı mekanizma**, sadece SDK yerine tarayıcı sekmesi üstüne floating widget olarak render edilir |
| Kimlik/plan kontrolü | Hangi planda açık, kullanım limiti | Mevcut `plans::PlanRegistry` + `usage::UsageTracker` — config-driven, yeni plan/flag eklemek yeterli |

**Sonuç:** Bu özellik için SDK, RTMP relay, OBS entegrasyonu, platform OAuth, multistream — **hiçbiri gerekmiyor**. Sadece yeni bir **hafif web istemcisi** (mikrofon yerine `getDisplayMedia` sekme sesi kullanan bir capture sayfası) + mevcut STT/çeviri/overlay pipeline'ının bu yeni istemciyi kabul etmesi yeterli.

### Uygulama Planı (Görevler — henüz hiçbir faza atanmadı)
- [ ] `/watch-party` (veya `/control`) sayfası — tarayıcıda `getDisplayMedia({video:true, audio:true})` ile sekme sesi yakalama, kullanıcıya "sekme sesini paylaş" seçtirme UI'ı
- [ ] Yakalanan ses'i mevcut WS ses protokolüne (`docs/websocket-protocol.md`) uyacak şekilde encode edip backend'e gönderme (SDK'nın yaptığı işin tarayıcı-JS karşılığı)
- [ ] Backend: yeni bir `client_type: "watch_party"` ayrımı (SDK session'larından farklı — RTMP/relay alanları yok, sadece STT+çeviri) — pipeline zaten sağlayıcı-agnostik olduğu için minimal değişiklik
- [ ] Floating altyazı widget'ı (mevcut `/o/[token]` overlay mantığının tarayıcı-içi, sekme üstü versiyonu — konumlandırma/opaklık ayarlı)
- [ ] Plan gating: yeni bir flag (`watch_party.enabled`) — hangi planda açık olduğu admin panelden (config-driven, CLAUDE.md kuralına uygun), rakip gibi sadece en üst planla sınırlamak zorunda değiliz
- [ ] Kullanım takibi: `UsageTracker`'a watch-party seansları da dahil edilir mi yoksa ayrı kotalı mı olacağı kararı (ürün kararı, admin panelden yapılandırılabilir)
- [ ] Hukuki/ToS notu: sekme sesi yakalama kullanıcının **kendi** cihazında render edilen sesi kişisel kullanım için çeviriyor — yeniden yayın/dağıtım değil. Netflix/YouTube gibi platformların ToS'una aykırı olmayan "erişilebilirlik amaçlı kişisel altyazı" çerçevesinde konumlandırılmalı; pazarlama metninde "DRM atlatma" gibi ifadelerden kaçınılmalı (hukuki risk)
- [ ] Tarayıcı uyumluluğu testi — `getDisplayMedia` sekme-sesi paylaşımı Chrome/Edge'de net, Safari/Firefox'ta kısıtlı/farklı davranabilir; desteklenmeyen tarayıcıda net bir uyarı gerekir

> Bu özellik SDK'dan bağımsız, tamamen web tabanlı olduğu için Faz 1-3'ün SDK/RTMP/multistream işiyle çakışmaz — ayrı, paralel geliştirilebilecek bir görev grubu. Hangi faza (Faz 2 sonrası mı, Faz 4 büyüme kapsamında mı) alınacağı kullanıcı kararı bekliyor; şimdilik sadece "Coming Soon" kataloğunda ve bu analiz bölümünde duruyor.

---

## Öz Kaynaklı Ses Klonlama + Gerçek Speech-to-Speech (Faz 3 sonrası / araştırma — henüz hiçbir faza atanmadı)

> Kaynak: kullanıcı isteği, 2026-07-06. **Güncelleme (aynı gün):** kullanıcı bu bölümün büyük kısmını
> ("dublaj" modu için gerekenler — dil ataması şeması, ham örneğin şifreli saklanması, `mode`/`source`
> alanları) Faz 2'ye (2.1/2.2) taşınıp hemen kodlanmasına karar verdi; bkz. 2.1'in yeni maddeleri ve
> aşağıdaki "Faz 2'ye Taşınanlar" notu. Burada SADECE gerçekten ayrı bir faz/karar gerektiren kısım
> kaldı: **gerçek Speech-to-Speech motoru** (metin ara katmanı olmadan, orijinal ses tonlaması
> korunarak çalışan bir ses-dalgasından-ses-dalgasına model) + **ElevenLabs bağımlılığının
> kaldırılıp kendi self-hosted klonlama/TTS/S2S motorumuza geçilmesi**. İkisi de native motor
> derlemesi gerektirir — whisper.cpp'deki `cmake` blocker'ıyla AYNI kategori, kullanıcıyla AYRI
> görüşülmeden koda dökülmemeli.

### Faz 2'ye Taşınanlar (bu oturumda kodlandı — artık burada değil)
- `VoiceModel.language_mode` (`SingleToSingle`/`MultiToSingle`/`SingleToMulti`) + `source_languages`/
  `target_languages` şeması — bkz. 2.1.
- Ham ses örneğinin AES-256-GCM şifreli saklanması (`voice_models::VoiceSampleStore`) — bkz. 2.1.
- `VoiceModel.mode` (`Dubbing`/`SpeechToSpeech`) + `source` (`UserCloned`/`Preset`) alanları — şema
  hazır (varsayılan `Dubbing`/`UserCloned`), ama dashboard'da SEÇİLEMEZ: `SpeechToSpeech` gerçek bir
  S2S motoru gerektirir (bkz. aşağı), `Preset` gerçek bir self-hosted model çıktısı gerektirir (bkz.
  aşağı) — ikisi de bu bölümdeki motor işine bağlı olduğu için UI'da yalnızca çalışan yol (Dubbing +
  UserCloned) sunuluyor.
- Admin'in (yayıncı, hedef dil) başına aktif model ataması (`POST /admin/voice/models/{id}/assign`)
  — bkz. 2.2.
- TTS kota/kredi tüketimi (gerçek ses baytından hesaplanan saniye, STT ile paylaşılan `UsageTracker`
  havuzuna) — bkz. 2.2. Madde 6'nın "kredi sistemi tam admin-konfigüre edilebilir olmalı" isteği,
  bugün TEK bir cloud sağlayıcı (ElevenLabs, sabit 1:1 oran) olduğu için basit tutuldu; birden fazla
  sağlayıcı/mod arasında gerçek bir oran karşılaştırması ancak self-hosted motor gelince anlamlı
  olacak (bkz. aşağıdaki "Kredi/kota" görevi).

### Kalan Kullanıcı Kararları (birebir, henüz kodlanmadı)
1. **Yayıncı panelden seçebilmeli: "dublaj" (klonlanmış ses ile TTS) mı yoksa "kendi sesi" (orijinal
   sesin korunduğu S2S) mı kullanılsın.** Şema (`VoiceModel.mode`) hazır; UI ve gerçek S2S sentezi
   motor gelince eklenecek.
2. **ElevenLabs'a bağımlılık kaldırılmalı** — kendi self-hosted klonlama+TTS/S2S modelimiz olmalı
   (örn. XTTS-v2/Coqui, veya gerçek S2S için Seamless/StyleTTS2 ailesi).
3. **Alternatif olarak "seçili sesler"den de kullanılabilsin** — yani ElevenLabs'in hazır ses
   kütüphanesi değil, **bizim kendi modelimizle** üretilmiş, önceden hazırlanmış hazır seslerden
   birini seçme opsiyonu da olmalı (klonlama yapmadan hızlı başlangıç için). Şema (`VoiceModel.source
   = Preset`) hazır, ama gerçek preset SESLERİ üretecek bir self-hosted model olmadan bu kaynaktan
   hiçbir model olamaz.
4. **Self-hosted mod ElevenLabs'e göre daha az kredi/kota tüketmeli** — kendi altyapımızda çalıştığı
   için birim maliyet düşük, bu avantaj kota mantığına yansıtılmalı.

### Bu, Şu Anki Mimariyi Nasıl Etkiliyor
- **Yeni bir `TtsProvider`/`S2sProvider` implementasyonu** self-hosted motor için gerekiyor —
  ElevenLabs'takiyle aynı trait deseni (`gateway::tts`), ama gerçek S2S modelin ses dalgasından
  ses dalgasına çalışması STT→çeviri→TTS zincirinden **farklı bir pipeline dalı** gerektirir
  (metin ara katmanı yok — bkz. önceki oturumda konuşulan S2ST/voice-conversion farkı).
- **Native motor derlemesi gerekir** (whisper.cpp ile aynı blocker ailesi) — bu makinede `cmake`
  yok, bu yüzden gerçek motor entegrasyonu ERTELENECEK, ama trait/API/admin panel konfigürasyonu
  şimdiden koda dökülebilir (mock motor ile test edilir, CLAUDE.md deseni).
- **Admin panelde yeni bir "Kredi/Kota Konfigürasyonu" ekranı** gerekiyor (muhtemelen 2.7'nin
  kapsamına giriyor — `/admin/routing`'in "canlı maliyet göstergesi" maddesiyle birleştirilebilir):
  her mod/sağlayıcı için birim-başı kredi tüketimi admin'den girilebilir, restart gerektirmez —
  bugün tek sağlayıcı olduğu için erken.

### Görevler (henüz faza atanmadı — muhtemelen Faz 3 veya ayrı bir "Faz 2.9")
- [ ] Self-hosted klonlama+TTS motoru seçimi (XTTS-v2/Coqui gibi) + gerçek S2S motoru araştırması
      (Seamless/StyleTTS2 ailesi) — native derleme gerektirdiği için cmake kararı kullanıcıyla
      AYRI görüşülmeli (whisper.cpp sorusuyla aynı ciddiyette)
- [ ] Gerçek S2S sentez yolu: `gateway::tts`'ten farklı, ses-dalgasından-ses-dalgasına ayrı bir
      pipeline dalı (STT/çeviri metin ara katmanını atlar) + `VoiceModel.mode=SpeechToSpeech`
      dashboard'da seçilebilir hâle gelir
- [ ] "Seçili sesler" (bizim ürettiğimiz hazır ses kütüphanesi) — self-hosted motor çıktısından
      üretilir, `VoiceModel.source=Preset` UI'da seçilebilir hâle gelir
- [ ] Kredi/kota: self-hosted mod için ElevenLabs'e göre daha düşük çarpan (`usage_multiplier`
      deseni) — admin panelden TAM konfigüre edilebilir (her mod/sağlayıcı için ayrı birim-kredi
      oranı, restart'sız, `config::get` üzerinden); birden fazla sağlayıcı olunca anlamlı
- [ ] ElevenLabs bağımlılığının admin panelden opsiyonel/kaldırılabilir hale getirilmesi (fallback
      zinciri: self-hosted → ElevenLabs, tersi değil — maliyet önceliği self-hosted'da)

> Bu bölüm SIRF tasarım/niyet notudur — self-hosted motor/gerçek S2S için hiçbir kod yazılmadı
> (kasıtlı — cmake/motor seçimi kullanıcıyla ayrı görüşülmeli). `VoiceModel` şeması artık nihai
> hâlini aldı (2.1/2.2'de kodlandı) — bu bölümün görevleri şemayı DEĞİŞTİRMEZ, sadece `mode=
> SpeechToSpeech`/`source=Preset` değerlerini gerçek anlamda kullanılabilir hâle getirir.
