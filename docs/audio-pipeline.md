# Ses Pipeline Kararları

## Veri Akışı
```
Mikrofon → VAD (lokal) → gürültü temizleme (lokal)
→ TLS 1.3 → WebSocket → bulut
→ STT → Çeviri → altyazı
→ SDK'ya geri → video + altyazı birleştir (lokal)
→ Restream/Castr (Faz 1) / SDK RTMP relay (Faz 2)
```
Ses akışı TLS 1.3 ile taşınır. Üstüne AES yok (double encryption = gereksiz CPU).

## Bant Genişliği
- Ses buluta: ~50-128 Kbps / yayıncı
- Video hiç buluta çıkmaz (Mod A/B)
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
