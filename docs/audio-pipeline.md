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
- STT: <300ms | Çeviri: <100ms | Toplam: <2sn

## STT Seçenekleri
| Sağlayıcı | Tür | Latency | Maliyet (1000 kullanıcı/ay) | Faz |
|---|---|---|---|---|
| Deepgram Nova-2 | API | <300ms | ~$1,032 | Faz 1 |
| faster-whisper large-v3 | Local GPU | ~500ms | ~$80 sabit | Faz 2 |
| AssemblyAI | API | ~400ms | ~$1,200 | Deepgram fallback |
| whisper.cpp | Local CPU | ~800ms | ~$20 | Düşük maliyet yedek |

## Çeviri Seçenekleri
| Sağlayıcı | Tür | Dil | Maliyet (1000 kullanıcı/ay) | Faz |
|---|---|---|---|---|
| DeepL | API | 100+ | ~$150 | Faz 1 |
| HY-MT1.5-7B | Local GPU | 33 | ~$0 (GPU paylaşımlı) | Faz 2 |
| Google Translate | API | 130+ | ~$120 | DeepL fallback |
| LibreTranslate | Local | 30+ | ~$0 | Faz 2 yedek |

## TTS / Ses Klonlama Seçenekleri
| Sağlayıcı | Tür | Latency | Maliyet (1000 kullanıcı/ay) | Faz |
|---|---|---|---|---|
| ElevenLabs | API | ~800ms | ~$600 | Faz 2 başlangıç |
| Cartesia | API | ~200ms | ~$240 | Hız alternatifi |
| PlayHT | API | ~500ms | ~$200 | Ucuz API |
| XTTS-v2 (Coqui) | Local GPU | ~600ms | ~$80 sabit | Faz 3 |
| StyleTTS2 | Local GPU | ~400ms | ~$80 sabit | Faz 3 araştırma |

## Admin Toggle Sırası
Tüm sağlayıcılar admin panelden anlık değiştirilebilir (restart yok).
Fallback: Deepgram → AssemblyAI → whisper.cpp
