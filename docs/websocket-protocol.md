# WebSocket Protokol Spec — SDK ↔ Backend

## Bağlantı

```
wss://api.strovak.com/ws/stream
```

Bağlantı kurulunca ilk mesaj `auth` olmalı. 5 saniye içinde `auth` gelmezse backend bağlantıyı kapatır.

## Frame Türleri

| Tür | WebSocket frame | Kullanım |
|---|---|---|
| Kontrol mesajları | Text (JSON) | auth, config, subtitle, error, ping/pong |
| Ses verisi | Binary | ham ses paketi (PCM veya Opus) |

---

## SDK → Backend Mesajları

### `auth`
```json
{
  "type": "auth",
  "token": "<JWT access token>"
}
```
**Yanıt:** `session_ready` veya `error` (kod `4001`)

---

### `stream_start`
```json
{
  "type": "stream_start",
  "session_id": "<sdk_session_id>",
  "source_lang": "tr",
  "target_langs": ["en", "de"],
  "audio_format": {
    "codec": "opus",
    "sample_rate": 16000,
    "channels": 1
  }
}
```
**Yanıt:** `stream_ack`

`audio_format.codec` değerleri: `"opus"` (tercih) veya `"pcm_s16le"`  
`sample_rate`: `16000` sabit (STT için yeterli, 48kHz gereksiz)

---

### `stream_stop`
```json
{
  "type": "stream_stop",
  "session_id": "<sdk_session_id>"
}
```

---

### `ping`
```json
{
  "type": "ping",
  "ts": 1719600000000
}
```
Her 15 saniyede bir SDK gönderir. 30 saniye cevap gelmezse SDK yeniden bağlanır.

---

### Audio Binary Frame

Text frame değil, binary frame gönderilir. Header 8 byte sabittir:

```
[0..3]  → magic: 0x53 0x44 0x41 0x55  ("SDAU")
[4]     → codec: 0x01=opus, 0x02=pcm_s16le
[5]     → reserved: 0x00
[6..7]  → payload_len: uint16 big-endian
[8..]   → ses verisi (payload)
```

VAD lokalde çalıştığından backend sessiz frame almaz — gelen her binary frame'i direkt STT'ye ilet.

---

## Backend → SDK Mesajları

### `session_ready`
```json
{
  "type": "session_ready",
  "session_id": "<sdk_session_id>",
  "user_id": "<uuid>",
  "plan": "free",
  "remaining_seconds": 18000
}
```
`remaining_seconds`, `plans::PlanRegistry`'deki `monthly_hours` ve
`usage::UsageTracker`'daki bu ayki kullanım üzerinden hesaplanır (bkz. Faz
1.11); `-1` sınırsız plan demektir.

---

### `stream_ack`
```json
{
  "type": "stream_ack",
  "session_id": "<sdk_session_id>",
  "status": "streaming",
  "overlay_token": "<opaque_token>"
}
```
`overlay_token` — OBS Browser Source / dashboard canlı önizlemenin bağlandığı
`GET /overlay/{token}` WebSocket'i için capability token'ı (bkz. Faz 1.9,
`backend/src/overlay`). Auth gerektirmez; token'ın kendisi yetkidir — stream
key'le aynı güven modeli.

---

### `subtitle`
```json
{
  "type": "subtitle",
  "session_id": "<sdk_session_id>",
  "sequence": 42,
  "source": {
    "lang": "tr",
    "text": "Bugün çok iyi bir gün.",
    "is_final": true
  },
  "translations": [
    { "lang": "en", "text": "Today is a very good day." },
    { "lang": "de", "text": "Heute ist ein sehr guter Tag." }
  ],
  "ts_start_ms": 1719600003120,
  "ts_end_ms": 1719600004850
}
```

`is_final: false` → geçici (Deepgram interim result), SDK overlay'de italik gösterebilir  
`is_final: true` → kesinleşmiş, overlay'e yaz  
`sequence` → SDK sıra dışı gelen paketi algılar (drop etmez, sıraya koyar)

---

### `pong`
```json
{
  "type": "pong",
  "ts": 1719600000000
}
```

---

### `usage_update`
```json
{
  "type": "usage_update",
  "used_seconds": 3600,
  "remaining_seconds": 14400
}
```
Her 5 dakikada bir veya `stream_stop` sonrasında gönderilir. **KALAN:** Şu an
bu mesaj gönderilmiyor — kullanım her ses frame'inde arka planda takip
ediliyor ve `session_ready`'e yansıyor, ama limite ulaşılana kadar periyodik
bir bildirim yok. Limite ulaşınca doğrudan `error` (4003) gönderilip session
kapatılıyor (bkz. `pipeline::StreamSession::process_audio`).

---

### `error`
```json
{
  "type": "error",
  "code": 4003,
  "message": "plan_limit_reached",
  "detail": "Monthly hour limit exceeded"
}
```

| Kod | Anlam | SDK Aksiyonu |
|---|---|---|
| `4001` | Geçersiz / süresi dolmuş JWT | Token yenile, yeniden bağlan |
| `4002` | `stream_start` gelmeden ses paketi | `stream_start` gönder |
| `4003` | Plan saati doldu | Kullanıcıya bildir, bağlantıyı kapat |
| `4004` | Desteklenmeyen audio codec | `opus` kullan |
| `4005` | STT sağlayıcı hatası | Log'a yaz, 3 saniye bekle, yeniden dene |
| `4006` | Çeviri sağlayıcı hatası | Log'a yaz, fallback devreye giriyor |
| `5000` | Beklenmedik sunucu hatası | 5 saniye bekle, yeniden bağlan |

---

## Akış Diyagramı

```
SDK                          Backend
 |                              |
 |---- (TLS handshake) -------->|
 |---- auth {JWT} ------------->|
 |<--- session_ready -----------|
 |                              |
 |---- stream_start ----------->|
 |<--- stream_ack --------------|
 |                              |
 |==== [binary] audio_chunk ===>|  (sürekli)
 |<--- subtitle (interim) ------|
 |<--- subtitle (final) --------|
 |<--- subtitle (final) --------|
 |                              |
 |---- ping ------------------->|
 |<--- pong --------------------|
 |                              |
 |---- stream_stop ------------>|
 |<--- usage_update ------------|
 |                              |
 |----- (disconnect) -----------|
```

---

## Backend İçi Akış (Referans)

```
binary frame gelir
  → header parse (magic check, codec, len)
  → Deepgram WebSocket'e ilet (ham ses)
  → Deepgram → transcript JSON
  → DeepL API (tüm target_langs için paralel)
  → subtitle mesajı paketle
  → SDK WebSocket'e gönder
```

Deepgram ve DeepL çağrıları Tokio task'larıyla paralel — çeviri beklenirken STT bloklanmaz.

---

## Notlar

- mTLS Faz 1'den itibaren zorunlu — SDK ve backend karşılıklı sertifika doğrular
- JWT `exp` kontrolü her mesajda değil, sadece `auth` ve `stream_start`'ta yapılır (overhead'i azaltır)
- Binary frame boyutu maksimum 4096 byte (Opus frame için yeterli, büyükse chunk'la)
