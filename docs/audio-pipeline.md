# Ses Pipeline Kararları

## Veri Akışı
```
Mikrofon → VAD (lokal) → gürültü temizleme (lokal)
→ TLS 1.3 → WebSocket → bulut
→ STT → Çeviri → altyazı
→ SDK'ya geri → video + altyazı birleştir (lokal)
→ SDK RTMP relay (Faz 1, birincil) / Streamlabs (Faz 1, opsiyonel)
```
Ses akışı TLS 1.3 ile taşınır. Üstüne AES yok (double encryption = gereksiz CPU).

## Yerel Ses Yakalama (SDK)
"Mikrofon" adımı platforma göre iki farklı implementasyon:
- **Windows** — `sdk/src/audio/wasapi.rs`: hand-rolled WASAPI (shared-mode + event-driven,
  `AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM` ile motor 16kHz mono s16le'ye çeviriyor).
- **macOS + Linux** — `sdk/src/audio/cpal_capture.rs`: [cpal](https://github.com/RustAudio/cpal)
  crate'i (CoreAudio/ALSA arkasında). cpal resample/downmix yapmıyor — cihazın
  native config'ini (hız/kanal/format) açıp kendi doğrusal enterpolasyon
  resampler'ı + kanal-ortalama downmix'iyle 16kHz mono i16'ya çeviriyor.
  Hand-rolled FFI yerine cpal tercih edildi çünkü macOS kodu bu repoda hiç
  derleme kontrolünden geçemiyor (darwin target/SDK yok) — olgun bir crate'e
  yaslanmak, hiç çalıştırılamayan/test edilemeyen bir platform için daha güvenli.
  Linux tarafı gerçek donanımda (mikrofon) uçtan uca doğrulandı.

Referans: [meetily](https://github.com/Zackriya-Solutions/meetily) — Tauri +
Rust ile aynı sınıf bir cross-platform ses yakalama + yerel STT (whisper.cpp)
mimarisi; ileride whisper.cpp bağlanacağı zaman (2.6b, local-STT) da bakılabilir.

## Bant Genişliği
- Ses buluta: ~50-128 Kbps / yayıncı
- Video hiç buluta çıkmaz (Mod A/A')
- 1000 eşzamanlı = ~128 Mbps ses → standart sunucu yeterli

## VAD
- Lokalde çalışır (boş ses buluta gitmez)
- STT maliyetini ~%30-40 düşürür

## Latency Hedefi
- STT: <500ms | Çeviri: <100ms | Toplam: <2sn
- STT, VAD ile tamamlanmış ses kesitlerini self-hosted GPU worker'a gönderir.
  Gerçek gecikme kesit uzunluğu, GPU kapasitesi ve ağ round-trip'ine bağlıdır;
  üretimde canlı ölçümle doğrulanmalıdır.

## STT Seçenekleri
| Motor | Tür | Latency | Maliyet (1000 kullanıcı/ay) | Durum |
|---|---|---|---|---|
| faster-whisper large-v3 | Self-hosted GPU | ~500ms hedef | GPU altyapısı | Tek STT motoru |

Kaynak dil kapsamı, [`Systran/faster-whisper-large-v3`](https://huggingface.co/Systran/faster-whisper-large-v3)
modelinin 100 dilidir. Uygulama, modelin kanonik Whisper dil kodlarını kullanır;
sağlayıcıya özgü ayrı bir STT dil listesi bulunmaz.

## Çeviri Seçenekleri
| Sağlayıcı | Tür | Dil | Maliyet (1000 kullanıcı/ay) | Faz |
|---|---|---|---|---|
| DeepL | API | 100+ | ~$150 | Faz 1 |
| HY-MT1.5-7B | Local GPU | 33 | ~$0 (GPU paylaşımlı) | Faz 2 |
| Google Translate | API | 130+ | ~$120 | DeepL fallback |
| LibreTranslate | Local | 30+ | ~$0 | Faz 2 yedek |

## TTS / Ses Klonlama Seçenekleri
| Motor | Tür | Latency | Maliyet | Durum |
|---|---|---|---|---|
| Pocket TTS (Kyutai) | Local CPU / Python | ~200ms ilk chunk | CPU altyapısı | Faz 3 araştırma |
| XTTS-v2 (Coqui) | Local GPU | ~600ms hedef | GPU altyapısı | Faz 3 |
| StyleTTS2 | Local GPU | ~400ms hedef | GPU altyapısı | Faz 3 araştırma |

Not: [kyutai-labs/pocket-tts](https://github.com/kyutai-labs/pocket-tts)
CPU'da çalışacak kadar küçük bir TTS motoru olarak değerlendirilmeli. Repo
100M parametre, audio streaming, yaklaşık 200ms ilk ses parçası, CPU'da
real-time'dan hızlı çalışma, Python API/CLI, voice cloning ve EN/FR/DE/PT/IT/ES
dil desteği iddia ediyor. Faz 2'de API sağlayıcı maliyeti yükselirse, Faz 3
self-hosted TTS hattı için XTTS-v2/StyleTTS2 yanında ucuz seçenek adayı.

## Admin Toggle Sırası
Tüm motor seçimleri config API'den (`PUT /admin/config`) restart'sız değiştirilir; ayrı
bir panel sayfası yok (eski `/admin/routing` kaldırıldı, commit `00a7c90` — örnek
curl: `infra/deploy.md` §8b).
STT için tek çalışma yolu self-hosted faster-whisper large-v3 worker'ıdır.
