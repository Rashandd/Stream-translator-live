# Yayıncı İş Akışları — Kullanım Amacına Göre

Bu klasör, StreamDubber/Strovak'ın potansiyel müşterileri (yayıncılar) için
**kullanım amacına göre uçtan uca iş akışlarını** belgeler: keşiften yayına,
yayından fatura/yükseltmeye kadar her adım — hangi dashboard sayfasında,
hangi API uç noktasıyla, bugün ne kadar hazır.

Bu dokümanlar aynı zamanda **ürün içi onboarding sihirbazının tek içerik
kaynağıdır** (ayrı bir görevde kodlanacak): her adımın sabit bir kimliği var
(`O-01`, `A-02`, `C-03`, `D-04`, `T-05` gibi), sihirbaz ekranları bu
kimliklere 1:1 eşlenecek şekilde tasarlanabilir.

## Segment seçici — "Hangi yayıncı tipisin?"

| Segment | Kimin için | Gereken plan | Ana çıktı | Doküman |
|---|---|---|---|---|
| **Sadece altyazı** | Kendi dilinde erişilebilirlik/sessiz izleyici isteyen yayıncı | Free / Starter | Kendi dilinde overlay altyazı | [`01-sadece-altyazi.md`](./01-sadece-altyazi.md) |
| **Altyazı + çeviri** | Uluslararası kitle hedefleyen yayıncı | Starter / Pro | Çok dilli, izleyicinin seçtiği dilde altyazı | [`02-altyazi-ceviri.md`](./02-altyazi-ceviri.md) |
| **Dublaj / ses klonlama** | Kendi sesiyle başka dilde "konuşmak" isteyen yayıncı | Pro / Streamer | İkinci hesapta klonlanmış sesle dublajlı yayın | [`03-dublaj-ses-klonlama.md`](./03-dublaj-ses-klonlama.md) |
| **Tam paket** | Profesyonel/ajans — dublaj + çok platforma eşzamanlı dağıtım | Streamer | Dublaj + N platforma multistream | [`04-tam-paket.md`](./04-tam-paket.md) |

Her segment dokümanı, tüm segmentlerin paylaştığı çekirdek akışa
([`00-ortak-adimlar.md`](./00-ortak-adimlar.md)) link verir ve yalnız
**kendi farkını** anlatır — kayıt/SDK eşleştirme gibi ortak adımları tekrar
etmez. Platform bazlı (Twitch/YouTube/Kick) detaylar tek bir dosyada
toplanır: [`05-platform-notlari.md`](./05-platform-notlari.md).

## Durum rozetleri sözlüğü

Her adım başlığında bu 4 rozetten biri kullanılır — dürüstlük mekanizması,
henüz canlı olmayan bir özellik asla "yapın" diliyle anlatılmaz:

| Rozet | Anlamı |
|---|---|
| `[HAZIR]` | Bugün kodlanmış, otomatik testlerle doğrulanmış ve gerçek kullanıcı tarafından ürün üzerinden ulaşılabilir. |
| `[KOD HAZIR — CANLI DOĞRULAMA BEKLİYOR]` | Kod tamam ve testli, ama gerçek 3. parti hesap/donanımla (Windows/macOS SDK, gerçek OAuth, gerçek ödeme) hiç doğrulanmadı. |
| `[YAKINDA]` | Kod var ama uçtan uca bağlı değil veya bir insan/ops adımı (ör. Twitch review, gerçek sertifika) bekliyor. |
| `[PLANLANDI]` | Henüz başlanmadı. |

> **Genel çekince:** Faz 1 çekirdeği kod olarak tamamdır, ama gerçek launch
> kapısı `streamdubber_roadmap_final.md`'deki **"Faz 1 — MVP Öncesi Canlı
> Doğrulama Listesi"**dir (gerçek key/OAuth/Stripe/sertifika/donanım
> testleri). `[HAZIR]` rozeti bu listenin bittiği anlamına gelmez — sadece
> özelliğin kodlanıp otomatik testlerle doğrulandığı anlamına gelir.

## Adım şablonu

Her adım şu 5 alanla yazılır (sihirbaz ekranlarına doğrudan eşlenebilsin diye):

1. **Ne yapıyorsun** — adımın amacı, tek cümle.
2. **Nerede** — dashboard rotası veya SDK ekranı.
3. **Nasıl** — numaralı, somut aksiyonlar.
4. **Arka planda** — ilgili API uç nokta(ları) (wizard implementasyonu için).
5. **Sorun olursa** — 1-2 satır inline sorun giderme veya ilgili bölüme link.

## STT dil kapsamı ve çeviri — ortak kural

- **Kaynak dil (konuşma dili) kapsamı** referansı:
  [`Systran/faster-whisper-large-v3`](https://huggingface.co/Systran/faster-whisper-large-v3)
  (Whisper large-v3 ailesi, ~99 dil). Roadmap'in kendi Faz 2 self-hosted STT
  hedefiyle uyumlu (`streamdubber_roadmap_final.md` §"STT (Ses → Metin)").
  Bugün canlı bulut STT sağlayıcısı Deepgram (+ AssemblyAI fallback)'tir;
  bu model referansı hem self-hosted bulut GPU STT (roadmap 2.6) hem de
  cihazda/yerel STT (2.6b) için dil-kapsamı kaynağıdır — detay için bkz.
  `00-ortak-adimlar.md`'nin son bölümü.
- **Hedef dil (çeviri/altyazı) kapsamı:** yayıncı, çeviri kataloğundaki
  **her dile** çevrilebilir (`backend/src/translate_languages.rs`, DeepL +
  Google fallback, ~30 dil — güncel liste `GET /api/translate/catalog`).
  Plan limiti (`max_languages`) yalnız **eşzamanlı seçili hedef dil
  sayısını** sınırlar, kataloğun kendisini daraltmaz.
- **Kendi cihazında STT seçeneği her akışın EN SON aşamasıdır.** Varsayılan
  yol her zaman bulut STT'dir; cihazda/yerel seçenek yalnızca her segment
  dokümanının sonunda "İleri seviye" bölümü olarak anlatılır, erken
  adımlarda hiç önerilmez.

## Plan × özellik matrisi

> Kaynak: `backend/src/plans/mod.rs` (`PlanRegistry`). Bu tablo
> bilgilendirmedir — gerçek fiyat/limit her zaman admin panelden
> (`PUT /admin/plans`) değiştirilebilir, koda gömülü değildir.

| Plan | Fiyat | Saat/ay | Maks. eşzamanlı dil | Maks. platform | Watermark | Ses klonlama |
|---|---|---|---|---|---|---|
| **free** | $0 | 5 | 1 | 1 | var | yok |
| **starter** | $9.99 | 30 | 3 | 3 | yok | yok |
| **pro** | $24.99 | 80 | 10 | sınırsız | yok | var (dublör ses/ucuz-tier TTS) |
| **streamer** | $99.99 | 200 | sınırsız | sınırsız | yok | var (orijinal ses klonlama/üst-tier TTS) |
| **founding** *(gizli kapalı beta)* | $0 | 50 | 3 | 3 | yok | yok |

`-1` (sınırsız) `max_languages`/`max_platforms` alanlarında admin panelde
"sınırsız" olarak görünür.

## Dosya haritası

| Dosya | İçerik |
|---|---|
| [`00-ortak-adimlar.md`](./00-ortak-adimlar.md) | Tüm segmentlerin paylaştığı çekirdek akış: keşif → kayıt → SDK eşleştirme → ses yakalama → go-live → yayın sırasında → kullanım/fatura + STT'nin "ileri seviye" bölümü |
| [`01-sadece-altyazi.md`](./01-sadece-altyazi.md) | Segment 1 — Free/Starter |
| [`02-altyazi-ceviri.md`](./02-altyazi-ceviri.md) | Segment 2 — Starter/Pro |
| [`03-dublaj-ses-klonlama.md`](./03-dublaj-ses-klonlama.md) | Segment 3 — Pro/Streamer |
| [`04-tam-paket.md`](./04-tam-paket.md) | Segment 4 — Streamer |
| [`05-platform-notlari.md`](./05-platform-notlari.md) | Twitch/YouTube/Kick özet matrisi ve platforma özel notlar |
