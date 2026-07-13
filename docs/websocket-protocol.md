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
  },
  "stt_source": "cloud",
  "tts_source": "cloud"
}
```
**Yanıt:** `stream_ack` veya `error` (kod `4007` — plan lokal STT'ye izin vermiyor;
`4008`/`4009` — hedef dil sayısı/kodu reddi; `4010` — founding-beta onayı bekliyor)

`audio_format.codec` değerleri: `"opus"` (tercih) veya `"pcm_s16le"`  
`sample_rate`: `16000` sabit (STT için yeterli, 48kHz gereksiz)

`stt_source` (Faz 2.6b, opsiyonel — varsayılan `"cloud"`):
- `"cloud"` → ses binary frame'lerle buluta gider, STT backend'de (Deepgram) çalışır
- `"local"` → SDK sesi lokalde yazıya döker, ses yerine `transcript` mesajı gönderir;
  backend STT adımını atlayıp direkt çevirir. Hangi planların lokal STT
  kullanabileceği `stt.local.plans` config'iyle belirlenir (admin panel).
  `local` session'da binary ses frame'i göndermek de geçerlidir (bulut STT
  fallback'i) — tersi geçerli değildir (aşağıya bkz.).

`tts_source` (Faz 2.2b, opsiyonel — varsayılan `"cloud"`):
- `"cloud"` → final altyazı çevirisi ElevenLabs ile sentezlenir, `subtitle`
  mesajının `dubbed_audio` alanında SDK'ya döner (bkz. aşağıda)
- `"local"` → SDK kendi XTTS-v2 motoruyla sentezler, backend ses baytı hiç
  üretmez/göndermez (`dubbed_audio` boş kalır)

---

### `transcript` (sadece `stt_source: "local"` session'larında)
```json
{
  "type": "transcript",
  "session_id": "<sdk_session_id>",
  "text": "Bugün çok iyi bir gün.",
  "is_final": true
}
```
**Yanıt:** `subtitle` (backend STT'yi atlar, metni doğrudan çevirir) veya `error`

`is_final` opsiyoneldir, varsayılan `true` — lokal VAD bitmiş cümle yollar.
Kurallar:
- `stream_start` gelmeden `transcript` → `error 4002`
- Session `stt_source: "cloud"` ile açıldıysa → `error 4007` (`stt_source_not_local`) —
  plan gating'i baypas etmemek için mod, yayın başında bildirilmek zorunda
- `text` uzunluğu `stt.local.max_chars` config sınırını aşarsa → `error 4007`
  (`transcript_too_long`); boş/sadece boşluk metin sessizce atlanır

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
[5]     → flags: bit0=utterance_end (cümle sonu işareti); 0x00 = işaret yok (eski "reserved", geriye uyumlu)
[6..7]  → payload_len: uint16 big-endian
[8..]   → ses verisi (payload)
```

VAD lokalde çalıştığından backend sessiz frame almaz — gelen her binary frame'i STT tamponuna ekle.

**flags/utterance_end (Bölge A):** SDK VAD cümle sonunu (hangover kapanışı) gördüğünde
o utterance'ın son frame'ini `flags` bit0 (`utterance_end`) ile işaretler; backend bu
işareti görünce biriktirdiği frame'leri tek bir STT isteğine flush eder. `0x00` frame'ler
(işaretsiz/eski SDK) geriye uyumludur — backend süre/boşluk temelli fallback ile flush eder.
Bilinmeyen üst bitler yok sayılır (ileri uyumluluk).

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
  "dubbed_audio": [
    { "lang": "en", "audio_base64": "<mp3_44100_128 base64>" }
  ],
  "ts_start_ms": 1719600003120,
  "ts_end_ms": 1719600004850
}
```

`is_final: false` → geçici (Deepgram interim result), SDK overlay'de italik gösterebilir  
`is_final: true` → kesinleşmiş, overlay'e yaz  
`sequence` → SDK sıra dışı gelen paketi algılar (drop etmez, sıraya koyar)  
`dubbed_audio` (Faz 2.2, opsiyonel — yalnız `tts_source: "cloud"` ve final çeviri
varsa dolu) → hedef dil başına sentezlenmiş dublaj sesi (ElevenLabs `mp3_44100_128`,
base64). Eski SDK'lar bu alanı yok sayar (serde default davranışı).

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
| `4007` | Lokal STT reddi (`local_stt_not_allowed` / `stt_source_not_local` / `transcript_too_long`) | Planı kontrol et; `stream_start`'ı `stt_source:"local"` ile aç; metni cümle cümle gönder |
| `4008` | `target_langs` sayısı planın `max_languages` sınırını aşıyor | Hedef dil sayısını azalt veya plan yükselt |
| `4009` | `target_langs` içinde admin'in kapattığı/katalogda olmayan bir kod var | `GET /api/translate/catalog`'tan güncel listeyi çek |
| `4010` | "Founding 5" kapalı beta: `founding.gate.enabled=true` iken admin henüz onaylamadı | Kullanıcıya "başvurun onay bekliyor" göster, `stream_start` gönderme |
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
cloud STT (varsayılan):                local STT (Faz 2.6b):
binary frame gelir (VAD kesiti)        transcript mesajı gelir (text)
  → header parse (magic, codec, len)     → session stt_source=local mi? değilse 4007
  → Deepgram REST /v1/listen'e POST      → uzunluk sınırı (stt.local.max_chars)
    (prerecorded — kesit tamamlanınca)     → STT adımı ATLANIR
  → Deepgram → transcript JSON      ─────┘
  → DeepL API
  → subtitle mesajı paketle         ← iki yol da aynı Subtitle çıktısını üretir
  → SDK WebSocket'e + overlay'e gönder
```

**Not:** STT bugün gerçek zamanlı streaming WebSocket değil, VAD-kesiti başına
prerecorded REST çağrısıdır (`gateway::deepgram::DeepgramStt`). Gerçek zamanlı
streaming STT ayrı bir trait+pipeline değişikliği gerektirir ve henüz planlı
değil — gecikme kesit uzunluğu + HTTP round-trip'e bağlıdır, "<2sn" hedefi
canlı ölçülene kadar doğrulanmamış sayılmalı.

Deepgram ve DeepL çağrıları Tokio task'larıyla paralel — çeviri beklenirken STT bloklanmaz.
Kullanım (plan saati) takibi iki yolda da duvar saatiyle işlenir; fark tarifededir:
ses yolu 1.0×, transcript yolu `stt.local.usage_multiplier` (seed 0.5 — "kotan yarı
hızda tükenir") ile indirimli. Çarpan yol bazlı uygulandığından bulut fallback
dakikaları otomatik olarak tam tarifeye döner (bkz. `docs/local-stt-plan.md` §3).

---

## Notlar

- mTLS ERTELENDİ — sunucu tarafı hazır ama SDK istemci sertifikası sunmuyor;
  bugünkü karşılıklı doğrulama TLS 1.3 + cihaz JWT'sidir (bkz. `docs/security.md`)
- JWT `exp` kontrolü her mesajda değil, sadece `auth` ve `stream_start`'ta yapılır (overhead'i azaltır)
- Binary frame boyutu maksimum 4096 byte (Opus frame için yeterli, büyükse chunk'la)
