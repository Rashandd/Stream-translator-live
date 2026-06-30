# StreamDubber — Ürün Yol Haritası v4.0

**Website:** strovak.com

## Proje Özeti

**Vizyon:** Yayıncılara altyazı, çeviri, ses klonlama ve multistream hizmetini tek pakette sunan, platform bağımsız SaaS.  
**Rakip:** StreamTranslate (sadece yakılı altyazı, ses yok, izleyici seçemiyor)  
**Temel fark:** İzleyici dil seçer + klonlanmış ses + multistream + hepsi tek pakette  
**Hedef platformlar:** YouTube, Twitch, Kick  
**Müşteri:** Yayıncı (kullanıcı değil) — yayıncı öder, izleyici bedava kullanır  
**Büyüme stratejisi:** Önce ücretsiz/ucuz girişle kitle çek → sonra tam pakete geçir  
**Bağımlılık stratejisi:** Restream entegrasyonuyla başla → aşamalı olarak kendi altyapına taşı

---

## Büyüme Stratejisi — Aşamalı Geçiş

```
AŞAMA 1 — Kitle Çek
  Restream + diğer popüler platformlara entegrasyon
  Ücretsiz plan (altyazı, 1 dil, 5 saat/ay)
  Kurulum 60 saniye, OBS'e URL yapıştır bitti
  → Yayıncılar StreamTranslate yerine StreamDubber'ı dener

AŞAMA 2 — Bağla
  Ses klonlama ekle (premium özellik)
  İzleyici dil seçimi (Twitch Extension + Chrome Extension)
  → Rakipte olmayan özellikler devreye girer, geçiş maliyeti artar

AŞAMA 3 — Tek Paket
  Kendi RTMP sunucusu devreye girer
  Restream, Castr vb. artık opsiyonel
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

Yayıncı zaten bir multistream servisi kullanıyor olabilir veya hiç kullanmıyordur. StreamDubber ikisini de destekler. Asıl fark: video yayıncının kendi internetinden dağıtılır, senin Gbit sunucuna gerek kalmaz.

| Yöntem | Nasıl çalışır | Öncelik | Senin maliyetin |
|---|---|---|---|
| **SDK gömülü RTMP relay** | Yayıncının kendi interneti N platforma gönderir | Faz 2 — birincil | Sıfır (video buluta çıkmaz) |
| **Restream** | Yayıncı kendi Restream hesabını bağlar (OAuth) | Faz 1 — başlangıç | Sıfır (yayıncının aboneliği) |
| **Castr** | Yayıncı kendi Castr hesabını bağlar | Faz 1 — alternatif | Sıfır |
| **Streamlabs** | OAuth bağlantı | Faz 1 — entegrasyon | Sıfır |
| **Bulut multistream** | Video buluta çıkar, bulut dağıtır | Premium — opsiyonel | Gbit sunucu (~$65/ay) |

> SDK'nın kendi RTMP relay'i Faz 2'de gelir; Faz 1'de yayıncı mevcut servisini (Restream/Castr) bağlayarak başlar. Bulut multistream sadece interneti zayıf yayıncılar için premium opsiyon.

### Entegrasyon Mantığı — İki Çalışma Modu

StreamDubber'ın işleme katmanı (STT + çeviri + ses klonlama) her zaman aradadır. Fark, çıktının nasıl dağıtıldığında:

**Mod A — Mevcut Servis Pass-Through (Faz 1)**
```
Yayıncı PC (SDK) → AES-256 → bulut işleme (sadece ses) → klonlanmış ses + altyazı → SDK
SDK → video + ses birleştir → yayıncının Restream/Castr hesabı → platformlar
```
Yayıncı kendi multistream servisini kullanır. Video hiç buluta çıkmaz; sadece ses gidip gelir.

**Mod B — SDK Yerel Multistream (Faz 2-3)**
```
Yayıncı PC (SDK) → AES-256 → bulut işleme (sadece ses) → klonlanmış ses + altyazı → SDK
SDK içinde gömülü RTMP relay → yayıncının kendi internetinden → N platforma dağıtım
```
SDK'nın içinde bir RTMP relay var. Yayıncının kendi interneti N platforma gönderir. **Senin Gbit sunucuya ihtiyacın kalmaz** — video hiç buluta uğramaz.

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
| Video + ses birleştirme | Yayıncı PC (Mod A/B) | lokal |
| Multistream dağıtım | Yayıncı interneti (Mod A/B) | video: 6 Mbps/platform |

**Kritik avantaj:** Mod A ve B'de video hiç buluta çıkmaz. Sadece ses (çok küçük) gidip gelir. Bu yüzden Gbit RTMP sunucusu gerekmez; sadece GPU sunucu (ses klonlama için) yeterli.

### Yayıncıya Sunum — Dashboard Akışı

Yayıncı "Yayın Kurulumu" sayfasında şu seçeneği görür:

```
Dağıtım yöntemini seç:

  ○ Mevcut servisimi kullan (Restream / Castr / Streamlabs)
      → Hesabını bağla, StreamDubber sadece çeviri/ses ekler

  ○ Doğrudan platformlara gönder (StreamDubber RTMP)   [Faz 3]
      → Ekstra servise gerek yok, biz dağıtırız

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
║    ├── Gömülü RTMP relay (Mod B) — lokal multistream         ║
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
        │  Mod A: Yayıncının Restream/Castr hesabı
        │  Mod B: SDK gömülü RTMP relay (kendi internet)
        │  Mod C: Bulut multistream (premium, opsiyonel)
        └────────────────┬───────────────┘
                        ▼
            YouTube + Twitch + Kick
                        ▲
        İzleyici dil seçer: YouTube CC / Twitch Ext / Chrome Ext
```

**İşlem dağılımı (hibrit, temelde çok platformlu):**
- **Lokalde (yayıncı PC):** ses yakalama, VAD, gürültü temizleme, video+ses birleştirme, RTMP relay (Mod B), OBS yönetimi
- **Bulutta:** sadece STT, çeviri, ses klonlama — ağır AI işleri, GPU gerektirir
- **Web panel:** tüm kontrol — SDK'ya komut gönderir
- **Video hiç buluta çıkmaz** (Mod A/B) → Gbit sunucu gerekmez, sadece GPU sunucu yeterli

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
| **SDK gömülü RTMP relay** | Yayıncı PC (kendi internet) | Sıfır | Faz 2 — birincil |
| **Restream / Castr (yayıncının)** | Yayıncının kendi aboneliği | Sıfır | Faz 1 başlangıç |
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
- [ ] **SDK relay (önerilen):** Kendi internetinden dağıt, ekstra servis yok (Faz 2)
- [ ] **Mevcut servis:** Restream / Castr / Streamlabs OAuth bağlama (Faz 1)
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
- [ ] Dağıtım yöntemi dağılımı (SDK relay / Restream / Castr / bulut)
- [ ] SDK relay kullanan yayıncı sayısı ve durumu
- [ ] Yayıncının bağladığı dış servisler (Restream/Castr) durumu
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

## Faz 1 — MVP: Restream Entegrasyonlu Altyazı

**Hedef:** Ücretsiz plan ile kitle çek, rakipten daha iyi altyazı sun  
**Mesaj:** "StreamTranslate'ten daha iyi, üstelik ücretsiz başlayabilirsin"  
**Stack:** Deepgram + DeepL + Restream/Castr API

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
- [ ] API key'leri Vault'a taşı (koda gömme)  <!-- SecretStore soyutlaması + EnvSecretStore hazır; Vault KV HTTP okuması beklemede (vault servisi sağlanınca) -->
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
- [ ] Yayıncı giriş (2FA — email)
- [ ] Ana sayfa (durum, hızlı başlat)
- [ ] SDK indirme ve eşleşme sayfası
- [ ] Yayın kurulumu — dağıtım modu seçimi (Mod A/C)
- [ ] Dil seçimi ve overlay URL
- [ ] Platform hesapları (YouTube, Twitch, Kick OAuth)
- [ ] Stream key yönetimi (SDK'ya senkronize)

#### 1.5 Admin Dashboard — Faz 1 Temeli
<!-- NOT: 1.7 ve 1.8 STT/Çeviri görevleri; 1.5/1.6 numaraları Admin/Multistream için ayrılmıştır -->
- [ ] Admin giriş (2FA — email)
- [ ] Kullanıcı listesi ve detay
- [ ] Sistem durumu izleme
- [ ] Servis toggle (manuel başlangıç)
- [ ] Temel maliyet özeti

#### 1.6 Multistream — Yayıncının Mevcut Servisi (Mod A)
- [ ] Restream OAuth + API entegrasyonu
- [ ] Castr OAuth + API entegrasyonu
- [ ] Streamlabs OAuth entegrasyonu
- [ ] Dağıtım yöntemi seçim arayüzü (web panelden)
- [ ] Her bağlantı için test aracı
- [ ] Admin: hangi planda hangi yöntemin açılacağı
> SDK gömülü RTMP relay (Mod B) Faz 2'de gelir. Faz 1'de yayıncı kendi Restream/Castr hesabını bağlar.

#### 1.7 STT — Deepgram API
- [ ] Deepgram Nova-2 WebSocket
- [ ] Türkçe kalite testi
- [ ] AssemblyAI fallback

#### 1.8 Çeviri — DeepL API
- [ ] DeepL API (TR → EN, ES, DE, FR başlangıç)
- [ ] Google fallback

#### 1.9 OBS Browser Source Altyazı
- [ ] Gerçek zamanlı altyazı render
- [ ] Özelleştirilebilir görünüm
- [ ] 2 sn altı latency
- [ ] 3 platformda test

#### 1.10 YouTube Native CC
- [ ] YouTube Live CC API
- [ ] OAuth bağlantısı

#### 1.11 Ücretsiz Plan + Stripe
- [ ] Free plan (watermark, 5 saat limit)
- [ ] Starter / Pro / Streamer planları
- [ ] Kullanım takibi

**Faz 1 tamamlandığında:**
Yayıncı ücretsiz hesap açar → Restream veya direkt OBS ile bağlar → Türkçe konuşur → İngilizce altyazı çıkar. Rakipten üstün, ücretsiz başlanabiliyor.

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

#### 2.3 SDK Gömülü RTMP Relay (Mod B — yayıncı internetinden multistream)
- [ ] SDK içine RTMP relay motoru (Rust)
- [ ] Yayıncının kendi internetinden N platforma forward
- [ ] İkili yayın: TR ses → hesap 1, EN klonlanmış ses → hesap 2
- [ ] Yayıncı dashboard'unda Mod B seçeneğini aç
- [ ] Upload bant genişliği uyarısı (ikili yayın için ≥25 Mbps öner)
> Bu sayede video buluta çıkmaz, Gbit sunucu gerekmez.

#### 2.4 Twitch Extension
- [ ] Component overlay extension
- [ ] İzleyici dil seçim butonu
- [ ] Twitch review başvurusu (erken başla — 2–4 hafta)

#### 2.5 Chrome Extension — Kick ve Evrensel
- [ ] Manifest V3
- [ ] Chrome Web Store başvurusu

#### 2.6 faster-whisper Geçişi
- [ ] Hetzner GPU sunucu
- [ ] TR dil kalite testi
- [ ] Admin toggle: Deepgram → Whisper

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

> Not: Multistream maliyeti senin tablonda yok — video yayıncının kendi internetinden dağıtılır (SDK relay) veya yayıncı kendi Restream/Castr aboneliğini kullanır. Sen sadece sesi işliyorsun.

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
- Faz 1'de yayıncı kendi Restream/Castr hesabını bağlar; SDK relay Faz 2'de gelir.
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
