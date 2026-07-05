# Yerel STT — Sunum, Donanım Gereksinimi ve Fiyat Teşviki Planı (Faz 2.6b)

> Karar (2026-07-05): Yayıncıya **açık seçim** sunulur — "Bulut" / "Cihazında (Yerel)".
> Yerel mod yayıncı için daha ucuzdur (kota çarpanı), karşılığında donanım şartı vardır.
> Teknik altyapı (protokol + pipeline + plan gating) hazır: bkz. `docs/websocket-protocol.md`
> `transcript` bölümü ve `stt.local.*` config anahtarları.

## 1. Yayıncıya Sunum — Dashboard "Altyazı işleme" kartı

Ayarlar sayfasında tek kart, iki seçenek:

| | **Bulut** (varsayılan) | **Cihazında (Yerel)** |
|---|---|---|
| Kısa açıklama | "Her bilgisayarda çalışır." | "Kendi donanımını kullanırsın — kotan yarı hızda tükenir." |
| Kota | 1 dakika yayın = 1 dakika kota | 1 dakika yayın = **0.5 dakika** kota (çarpan config'ten) |
| Şart | Yok | Donanım testi geçmeli (aşağıda) |
| Rozet | — | "Deneysel" (ilk sürüm) |

Kopya metin ilkeleri:
- Fayda açık söylenir: **"Kendi donanımını kullandığın için daha az kota harcarsın."**
  (Dürüst ve basit — bizim bulut STT maliyetimizden bahsedilmez, gerek de yok.)
- Ek faydalar ikinci satırda: "Sesin bilgisayarından çıkmaz (gizlilik), gecikme daha düşük."
- Zorunlu dipnot: **"Donanımın yetişemezse yayın kesilmez — otomatik olarak buluta geçer;
  bu dakikalar normal (1×) sayılır."** Fallback dakikalarının normal sayılması hem adil
  hem de kötüye kullanımı (zayıf PC'de 'yerel' seçip sürekli buluttan hizmet alma) kapatır.
- "Cihazında" seçilebilmesi için **[Donanımımı test et]** butonundan geçer not almak şarttır;
  test geçilmediyse seçenek kilitli görünür ve gereksinim listesi gösterilir.

## 2. Donanım Gereksinimleri (ilk sürüm — NVIDIA only)

Model: **faster-whisper large-v3-turbo** (CTranslate2, int8_float16). TR kalitesi large-v3'e
çok yakın, VRAM ~2 GB. CPU yolu (whisper.cpp) ilk sürümde SUNULMAZ — oyun + OBS yanında
kalite/gecikme riski yüksek, destek yükü büyük. AMD/Intel GPU (CUDA yok) ilk sürümde kapsam dışı.

| | Minimum | Önerilen |
|---|---|---|
| GPU | NVIDIA GTX 1660 / RTX 2060 (6 GB VRAM) | RTX 3060 ve üstü (8+ GB VRAM) |
| Boş VRAM (yayın anında) | ≥ 2.5 GB | ≥ 3 GB |
| Sürücü | CUDA 12 destekli güncel sürücü | aynı |
| RAM | 16 GB | 16 GB |
| Disk | 2 GB (model dosyası ~1.5 GB) | aynı |

**Etkinleştirme testi (SDK içinde, [Donanımımı test et]):**
1. Donanım yoklama: NVIDIA GPU + sürücü + toplam/boş VRAM okunur. Şart sağlanmıyorsa test
   koşulmaz, eksik madde listelenir ("6 GB VRAM gerekli, sende 4 GB").
2. Model indirilir (ilk sefer, aşağıda §4) ve 15 sn'lik gömülü örnek TR ses parçasıyla
   gerçek inference koşulur.
3. Geçer not: **RTF ≤ 0.5** (15 sn ses ≤ 7.5 sn'de çözülür) VE segment başına p95 ≤ 1.2 sn.
   Geçemezse sonuç saklanır, kart "donanım yetersiz" der, Bulut'ta kalınır.
4. Test sonucu (geçti/kalmadı + ölçümler) backend'e raporlanır (destek + istatistik).

**Yayın sırasında watchdog (SDK):** boşta geçen test oyun yükünü ölçemez; asıl koruma runtime'da:
- Son 30 sn'de transkript üretim p95 > **1.5 sn** VEYA art arda 3 segment > 2 sn → SDK ses
  frame'i göndermeye başlar (bulut fallback — backend'de hazır: local session'da binary frame
  geçerli, sequence paylaşılır, overlay'de kesinti olmaz). Yayıncıya sessiz bir bildirim:
  "Donanım yoğun — geçici olarak buluta alındı."
- **5 dk** sonra yerel tekrar denenir (histerezis; flip-flop engellenir).
- Eşikler sunucudan iner (§3 policy endpoint'i) — SDK güncellemeden ayarlanabilir.

## 3. Fiyat Teşviki — kota çarpanı (yol bazlı, otomatik adil)

Mekanizma: kullanım tick'i hangi pipeline yolundan geldiyse o çarpanla işlenir.
- `transcript` yolu (gerçekten yerel çalışıyor) → elapsed × **`stt.local.usage_multiplier`** (seed: `0.5`)
- ses frame yolu (bulut veya fallback) → elapsed × 1.0

Böylece "yerel seçip buluttan hizmet alma" kendiliğinden imkânsız: indirim yalnız gerçekten
yerel işlenen dakikalara uygulanır. Plan saat tablosu değişmez (free 5 / starter 20 / pro 40 /
streamer 100) — pazarlama "Cihazında modunda saatlerin 2 katına kadar gider" der.

Maliyet notu (admin kararı): yerel mod Deepgram maliyetini sıfırlar ama DeepL çeviri maliyeti
karakter başına devam eder; 0.5 çarpanı 2× yayın süresi = 2× çeviri karakteri demektir.
Çarpan bu yüzden config'tedir — marj sıkışırsa admin panelden 0.6–0.75'e çekilir (restart'sız).
Faz 3'te HY-MT1.5 (lokal çeviri) devreye girince bu endişe kalkar, çarpan agresifleşebilir.

Yeni config anahtarları (seed + admin panel `PUT /admin/config`):

| Anahtar | Seed | Anlam |
|---|---|---|
| `stt.local.usage_multiplier` | `0.5` | transcript yolunda kota çarpanı |
| `stt.local.min_vram_mb` | `6144` | etkinleştirme şartı (SDK policy'den okur) |
| `stt.local.benchmark_rtf_max` | `0.5` | etkinleştirme testi geçer notu |
| `stt.local.watchdog_p95_ms` | `1500` | runtime fallback eşiği |
| `stt.local.retry_minutes` | `5` | fallback sonrası yerel tekrar deneme |

Mevcut anahtarlar: `stt.local.plans` (plan gating), `stt.local.max_chars` (satır sınırı).

**Policy endpoint'i:** `GET /api/stt/policy` (SDK pairing token'ı veya panel oturumu) →
```json
{
  "local_allowed": true,
  "usage_multiplier": 0.5,
  "requirements": { "min_vram_mb": 6144, "benchmark_rtf_max": 0.5 },
  "watchdog": { "p95_ms": 1500, "retry_minutes": 5 },
  "max_chars": 500
}
```
Tüm eşikler sunucu-taraflı: SDK sürümü yayınlamadan politika değiştirilebilir.

## 4. Model Dağıtımı

- Model **installer'a gömülmez** (boyut + imzalama derdi). "Cihazında" ilk etkinleştirildiğinde
  SDK, model dosyasını (~1.5 GB, CT2 large-v3-turbo int8) CDN'imizden indirir; SHA-256
  manifest'le doğrulanır; `%LOCALAPPDATA%/StreamDubber/models/` altına açılır.
- İndirme linki + hash policy endpoint'ine benzer bir manifest'ten gelir → model sürümü
  sunucudan yükseltilebilir (SDK bir sonraki başlangıçta yeni modeli çeker).
- Rust entegrasyonu: `whisper-rs` yerine **CT2 tabanlı sidecar süreç** değerlendirilecek
  (faster-whisper CT2 formatı whisper.cpp GGML'den farklı). Karar SDK task'ında verilecek;
  iki aday: (a) whisper.cpp + GGML turbo modeli (whisper-rs bind, tek process),
  (b) CT2 sidecar (ayrı process, crash izolasyonu). Varsayılan eğilim: **(a)** — tek binary,
  GGML large-v3-turbo kalitesi yeterli; (b) yalnız (a) TR kalite testinde kalırsa.

## 5. İş Kalemleri (sıra ile, her biri ayrı commit)

1. **Backend — kota çarpanı:** `StreamSession` transcript yolunda `stt.local.usage_multiplier`
   uygular (ses yolu 1.0 kalır); config seed + testler (çarpanlı tick, fallback 1×, çarpan
   admin'den değişince yeni session'a yansır).
2. **Backend — `GET /api/stt/policy`:** config'ten okuyan endpoint (SDK token + panel); testler.
3. **Frontend — "Altyazı işleme" kartı:** Bulut/Cihazında seçimi, gereksinim listesi, policy
   fetch, test-sonucu durumu; seçim SDK'ya pairing kanalından iner.
4. **SDK — yerel model (roadmap 2.6b madde 4):** model indirme+doğrulama, benchmark komutu,
   whisper entegrasyonu, watchdog + otomatik bulut fallback'i, `stt_source: local` akışı.
5. **Fiyatlandırma sayfası + docs:** "Cihazında modu" anlatımı, SSS ("donanımım yetmezse ne
   olur?"), gereksinim tablosu.

Doğrulama: 1-2 mock/birim + e2e; 3 frontend sözleşme testleri; 4 gerçek Windows + RTX cihazında
canlı smoke (user) — TR kalite karşılaştırması (aynı ses: Deepgram vs turbo) dahil.
