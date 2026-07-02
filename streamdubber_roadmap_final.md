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
| **Altyapı** | Railway (MVP) | Hetzner GPU + Cloudflare edge | Geliştirme → üretim ölçeği |
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
| **Railway** | Kolay deploy | ~$20–50/ay | MVP geliştirme |
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
- [x] Ses yakalama — Windows (WASAPI) önce, Mac/Linux adaptör iskeleti hazır  <!-- core/capture.rs + audio/open_capture; gerçek WASAPI cihaz binding'i Windows'ta yazılacak (stub: NotImplemented) -->
- [x] VAD (konuşma tespiti) — boş ses göndermeyi engelle  <!-- core/vad.rs (RMS+hangover) + core/gate.rs (capture→VAD→frame, sessizlik dropped) -->
- [x] Gürültü temizleme / ön-işleme  <!-- core/preprocess.rs (DC giderme + pre-emphasis); gate'e opsiyonel adım -->
- [x] Video + ses birleştirme (lokal)  <!-- core/mux.rs: Faz1 Mod A çıktı planı (video lokal/OBS, sadece ses buluta); gerçek mux + cloned audio Faz 2 -->
- [x] Bulut WebSocket bağlantısı (sadece ses, video lokal kalır)  <!-- core/cloud.rs CloudClient; backend'e karşı e2e test (2) geçiyor -->
- [x] OBS WebSocket entegrasyonu (altyazı + ses kanalı)  <!-- integrations/obs.rs: obs-websocket v5 mesaj tipleri (Identify, SetInputSettings); canlı OBS ile manuel test gerekir -->
- [x] Web panel ile eşleşme (hesap bağlama)  <!-- core/pairing.rs: 6 haneli kod doğrulama + PairingRequest; backend pairing endpoint'i 1.4'te -->
- [x] AES-256 şifreli sadece-ses akışı → bulut (video lokal kalır)  <!-- wss/TLS 1.3 transport (CLAUDE.md/security.md: üstüne AES YOK, double encryption). mTLS client-cert connector TODO -->
- [x] Code signing (Windows; Mac notarization sonra)  <!-- tauri.conf.json bundle signing config + docs/sdk-distribution.md; gerçek sertifikalar/CI beklemede -->
- [x] Otomatik güncelleme mekanizması  <!-- tauri.conf.json plugins.updater config + docs; runtime init + release CI beklemede -->
> Çekirdek platform-bağımsız yazılır; Mac/Linux ses adaptörü sonradan zorlanmadan aktive edilir.
> Faz 1.3 NOT (Claude): cloud client + e2e gerçek; preprocess/VAD/capture gerçek+test. OBS/pairing/signing/updater protokol+config seviyesinde — canlı OBS, backend pairing endpoint, sertifikalar ve release CI sonraki tasklarda. Gerçek WASAPI cihaz I/O Windows'ta yazılıp test edilmeli.

#### 1.4 Yayıncı Dashboard — Next.js + Tailwind
- [x] Yayıncı giriş (2FA — email)  <!-- login + 2FA verify UI akışı; mail gönderimi stub (noreply SMTP/API admin panelden ayarlanacak, şimdilik boş) -->
- [x] Ana sayfa (durum, hızlı başlat)  <!-- overview: metrics, izleyici dil dağılımı, stream key kopyala, hızlı başlat -->
- [x] SDK indirme ve eşleşme sayfası  <!-- /dashboard/sdk: Windows indir + 6 haneli pairing kod UI -->
- [x] Yayın kurulumu — dağıtım modu seçimi (Mod A/C)  <!-- /dashboard/distribution: "OBS'ten yönet" (SDK relay) / "Strovak yönlendirsin" (bulut) kartları; Restream/Castr/Streamlabs ayrı mod olarak gösterilmiyor -->
- [x] Dil seçimi ve overlay URL  <!-- /dashboard/captions: konuşma dili, hedef diller, görünüm, overlay URL, canlı önizleme -->
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
- [ ] Yayıncı dashboard'unda Mod A (SDK relay) seçeneğini aç ve varsayılan/önerilen yap  <!-- Faz 2.3'ten taşındı -->
- [ ] Upload bant genişliği uyarısı (çoklu platform için ≥25 Mbps öner)  <!-- Faz 2.3'ten taşındı -->
- [ ] Streamlabs OAuth entegrasyonu (opsiyonel alternatif)  <!-- authorize/token uç noktaları + scope tanımlı; KALAN: persist + Streamlabs API + canlı test -->
- [ ] Dağıtım yöntemi seçim arayüzü (web panelden) — SDK relay birincil, Streamlabs opsiyonel  <!-- /dashboard/distribution UI 1.4'te var; KALAN: authorize URL'lerini (GET /api/integrations, /authorize) UI'a bağlama -->
- [ ] Her bağlantı için test aracı  <!-- GET /api/integrations sağlayıcı durum listesi (configured + enabled_for_plan) hazır; KALAN: gerçek sağlayıcı API'sine canlı "bağlantıyı test et" çağrısı -->
- [ ] Admin: hangi planda hangi yöntemin açılacağı  <!-- MEKANİZMA HAZIR+TEST: config integration.<provider>.plans (admin /admin/config'ten değiştirilir), begin() plan erişimini enforce eder, list() plana göre gösterir. KALAN: admin UI toggle'ları + canlı doğrulama -->
> SDK gömülü RTMP relay Faz 1'e alındı ve asıl dağıtım yöntemi oldu (video buluta çıkmaz, Gbit sunucu gerekmez). Restream ve Castr desteği kaldırıldı; yayıncı isterse Streamlabs'ı opsiyonel bağlayabilir. İkili yayın (TR/EN ayrı hesap, klonlanmış ses) Faz 2'de ses klonlama ile birlikte gelir (bkz. 2.3).
> Faz 1.6 NOT (Claude): OAuth **akış çekirdeği** (authorize URL + CSRF state tek-kullanım + token değişimi) + plan-erişim gating config-driven ve mock-test'li (backend/src/integrations, sadece Streamlabs sağlayıcısı). Endpoint'ler: GET /api/integrations, /api/integrations/{provider}/authorize (yayıncı), /callback (state ile). client_id/secret SecretStore'dan (.env/Vault). Restream/Castr sağlayıcı kayıtları ve secret'ları kod ve `.env.example`'dan kaldırıldı. Hiçbiri CANLI çalıştırılmadı; token persist + gerçek Streamlabs API kullanımı kaldı.
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
- [ ] Merkezi loglama altyapısı (backend servisleri: WS pipeline, gateway, billing, integrations, overlay tek log formatına yazar)
- [ ] Log seviyeleri (error / warn / info / debug) ve yapılandırılabilir seviye (config-driven, restart gerektirmez)
- [ ] Loglar DB'ye veya log storage'a kalıcı yazılır (in-memory değil — restart'ta kaybolmaz)
- [ ] Admin panel: `/admin/logs` sayfası — sadece admin yetkili kullanıcı erişebilir (mevcut admin auth katmanı ile korunur)
- [ ] Log arama/filtreleme (servis, seviye, tarih aralığı, kullanıcı/session ID)
- [ ] Son N hata canlı listesi (Admin Dashboard "Log ve Hata İzleme" bölümüyle birleşir — bkz. satır 482-486)
- [ ] Hassas veri loglanmaz (API key, stream key, JWT, ses/görüntü verisi log'a yazılmaz — sadece referans ID)
> Not: Bu madde CLAUDE.md "console.log commit etme — logger kullan" kuralının altyapısal karşılığıdır. Admin Dashboard spec'inde zaten "Log ve Hata İzleme" bölümü var (satır 482-486); burada eklenen asıl loglama motoru + kalıcılık + admin-only erişim.

---

## Faz 1 — MVP Öncesi Canlı Doğrulama Listesi

Aşağıdaki maddeler kod olarak `[x]` işaretli ve mock-server / birim testleriyle doğrulanmış, ancak gerçek sağlayıcı hesabı, gerçek cihaz veya gerçek platform ile HİÇ canlı test edilmemiş. MVP çıkışından önce bunların tek tek doğrulanması gerekiyor:

### Güvenlik / Altyapı (1.2)
- [ ] Vault'a gerçek API key'lerin deploy sırasında populate edilmesi (ops adımı, henüz yapılmadı)
- [ ] AES-256 DB şifrelemesinin `_encrypted` kolonlara gerçek uygulanması (Cipher hazır, kolonlara henüz bağlanmadı)

### SDK (1.3)
- [ ] Gerçek WASAPI cihaz I/O — Windows'ta gerçek mikrofon ile capture testi (şu an stub/NotImplemented)
- [ ] OBS WebSocket entegrasyonu — canlı OBS ile manuel test (protokol mesajları yazıldı, gerçek OBS'e hiç bağlanılmadı)
- [ ] Code signing — gerçek Windows/macOS sertifikaları ile imzalama (config hazır, sertifika yok)
- [ ] Otomatik güncelleme mekanizması — runtime init + release CI ile uçtan uca test
- [ ] SDK ↔ bulut mTLS client-cert connector — SDK'nın cloud client'ına gerçek istemci sertifikası bağlanması + canlı karşılıklı doğrulama testi (1.2'de sunucu tarafı hazır; SDK tarafında connector TODO — bkz. 1.3 "AES-256 şifreli sadece-ses akışı" notu)

### Dashboard / Backend Wiring (1.4, 1.5)
- [ ] Platform OAuth token exchange (YouTube/Twitch/Kick) — gerçek OAuth akışı backend'e bağlanmalı
- [ ] Stream key senkronizasyonu SDK'ya — gerçek backend wiring
- [ ] Pairing backend endpoint'i — 6 haneli kod ile SDK ↔ hesap eşleştirmesinin backend ucu (SDK tarafı `core/pairing.rs` hazır; backend endpoint'i hiç yazılmadı — 1.3 "Web panel ile eşleşme" notunda 1.4'e bırakılmıştı)
- [ ] Admin panel canlı veri + servis start/stop + mail gönderimi (2FA mailleri dahil, şu an stub SMTP)

### Multistream (1.6)
- [ ] SDK RTMP relay'in gerçek OBS + gerçek YouTube/Twitch/Kick ile canlı fan-out testi (şu an sadece TCP loopback simülasyonu)
- [ ] Streamlabs OAuth — token persist + gerçek Streamlabs API çağrısı
- [ ] Yerel TCP listener'ın OBS'in bağlanacağı porta gerçek bağlanması (main.rs/Tauri tarafı wiring)

### STT / Çeviri (1.7, 1.8)
- [ ] Deepgram gerçek key ile canlı STT testi (TR dil kalitesi dahil)
- [ ] AssemblyAI fallback canlı testi
- [ ] DeepL gerçek key ile canlı çeviri testi
- [ ] Google Translate fallback canlı testi

### Overlay / YouTube (1.9, 1.10)
- [ ] OBS Browser Source altyazısının 3 platformda (YouTube/Twitch/Kick) canlı testi
- [ ] 2 saniye altı latency ölçümü (canlı yayında)
- [ ] YouTube Live CC — gerçek Google Cloud Console app + offline refresh token ile canlı doğrulama

### Ödeme (1.11)
- [ ] Gerçek Stripe hesabıyla canlı checkout + webhook testi
- [ ] Webhook event'lerinin `subscriptions`/`users.stripe_customer_id`'ye DB persist edilmesi (şu an sadece loglanıyor)
- [ ] Plans tablosundaki gerçek `stripe_price_id`'lerin admin panelden girilmesi
- [ ] Kullanım takibinin (UsageTracker) kalıcı hale getirilmesi — şu an bellek içi, restart'ta sıfırlanıyor

---

**Faz 1 tamamlandığında:**
Yayıncı ücretsiz hesap açar → SDK'yı kurar (kendi RTMP relay'i veya OBS ile) → Türkçe konuşur → İngilizce altyazı çıkar. Rakipten üstün, ücretsiz başlanabiliyor.

---

## Faz 2 — Ses Klonlama + İzleyici Dil Seçimi

**Hedef:** Rakipte olmayan özellikleri devreye al, ücretli plana geçişi tetikle  
**Mesaj:** "Artık izleyicin kendi dilini seçiyor ve senin sesinle duyuyor"  
**Stack:** ElevenLabs + Twitch Extension + Chrome Extension + faster-whisper geçiş başlıyor

### Görevler

#### 2.1 Ses Klonlama — ElevenLabs
- [ ] ElevenLabs API entegrasyonu
- [ ] Ses kaydı stüdyo arayüzü
- [ ] Model oluşturma pipeline'ı
- [ ] Admin ses model onay kuyruğu

#### 2.2 Gerçek Zamanlı TTS
- [ ] Metin → klonlanmış ses (2 sn altı)
- [ ] SDK'da video + klonlanmış ses birleştirme

#### 2.3 İkili Yayın — SDK Relay'e Klonlanmış Ses Entegrasyonu
- [ ] İkili yayın: TR ses → hesap 1, EN klonlanmış ses → hesap 2 (Faz 1.6'daki SDK relay üzerinden)
- [ ] İkili yayın için upload bant genişliği uyarısı güncellemesi (≥25 Mbps öner)
> SDK gömülü RTMP relay motoru Faz 1.6'da kuruldu; burada klonlanmış sesle ikili yayın eklenir.

#### 2.4 Twitch Extension
- [ ] Component overlay extension
- [ ] İzleyici dil seçim butonu
- [ ] Twitch review başvurusu (erken başla — 2–4 hafta)

#### 2.5 Chrome Extension — Kick ve Evrensel
- [ ] Manifest V3
- [ ] Chrome Web Store başvurusu

#### 2.6 faster-whisper Geçişi (bulut)
- [ ] Hetzner GPU sunucu
- [ ] TR dil kalite testi
- [ ] Admin toggle: Deepgram → Whisper

#### 2.6b STT Kaynağı — Bulut vs Yayıncı PC'si (Faz 1'den taşındı)
- [ ] Protokole `transcript` mesajı ekle (SDK → Backend): local STT kullanan SDK, ses yerine zaten yazıya dökülmüş metni gönderir
- [ ] `stream_start`'a `stt_source: "cloud" | "local"` alanı ekle — hangi modda çalışıldığı backend'e bildirilir
- [ ] Pipeline'da iki yolu birleştir: `cloud` → audio frame → STT gateway → çeviri; `local` → transcript mesajı → STT adımı atlanır, direkt çeviri. İkisi de aynı `Subtitle` çıktısını üretir
- [ ] SDK'ya gömülü local model (faster-whisper/whisper.cpp) — dashboard'dan "Bulut STT" / "Yerel STT (deneysel)" seçimi, donanım (GPU) kontrolü + uyarı
- [ ] Admin: hangi planda hangi STT kaynağının açık olduğu (config-driven)
> Yayıncıdan yayıncıya donanım farkı riski nedeniyle Faz 1'den çıkarıldı, Faz 2'ye alındı. Local STT, mevcut senkron istek/cevap mimarisini BOZMAZ — çünkü yerel model de VAD ile bölünmüş bitmiş cümleyi gönderiyor (Deepgram'ın gerçek streaming WS'i gibi sürekli açık bağlantı gerektirmiyor); sadece "STT adımını bazen atla" mantığı eklenir.

#### 2.7 Admin — Hibrit Kontrol Genişletme
- [ ] STT toggle (Deepgram ↔ Whisper)
- [ ] TTS toggle (ElevenLabs ↔ Cartesia)
- [ ] Canlı maliyet göstergesi

**Faz 2 tamamlandığında:**
İzleyici kendi dilini seçiyor. Yayıncı kendi sesiyle başka dilde konuşuyor. STT maliyeti $1,032 → $80/ay.

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

### Platform ve Geliştirici Çözümleri

- [ ] API for Developers — canlı STT/çeviri/TTS pipeline'ı için developer API ürünü
- [ ] Text-to-Speech / TTS — ses üretimi ve ileride ses klonlama ürün yüzeyi

### Sektörel Çözüm Sayfaları

- [ ] Churches — ibadet/topluluk yayınları için canlı çeviri
- [ ] Education — ders, kurs ve akademik etkinlikler için canlı çeviri
- [ ] Nonprofit — uluslararası topluluk ve bağış etkinlikleri için canlı çeviri
- [ ] Broadcasting — yayıncı, medya ve prodüksiyon ekipleri için canlı çeviri
- [ ] International Companies — global şirket toplantıları ve townhall'lar için canlı çeviri

### Frontend Coming Soon Kuralları

- [ ] Çözüm kataloğu `ui.public.solutions.catalog` config'inden render edilir.
- [ ] Her çözümün `status` alanı olur: `coming_soon`, `waitlist`, `beta`, `enabled`, `disabled`.
- [ ] Logo/ikon asset referansları config'ten gelir; Zoom/Teams/Google Meet marka kullanımı için marka kılavuzları kontrol edilmeden asset commit edilmez.
- [ ] Faz 1 dışı çözümler dashboard'da satın alınabilir/aktif özellik gibi gösterilmez.
- [ ] Coming soon kartlarında CTA admin-config kaynaklı olur: bekleme listesi, demo talebi veya görünmez.
- [ ] Admin panelden çözüm sırası, görünürlük, metin, CTA, hedef route ve feature flag değiştirilebilir.
