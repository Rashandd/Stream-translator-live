---
name: verify
description: StreamDubber backend'ini gerçek binary olarak ayağa kaldırıp HTTP/WS yüzeylerinden uçtan uca doğrulama reçetesi (admin login, config/secret panel uçları, pairing, SDK ses WS, overlay, Twitch Extension EBS).
---

# StreamDubber runtime doğrulama reçetesi

## Backend'i ayağa kaldır

```bash
cargo build -p streamdubber-backend   # binary: target/debug/streamdubber-backend
env JWT_SECRET=dev-secret \
    ADMIN_EMAIL=ops@dev.local ADMIN_PASSWORD=dev-password \
    BIND_ADDR=127.0.0.1:18090 \
    ./target/debug/streamdubber-backend
# DATABASE_URL/TLS/SECRETS_FILE opsiyonel — hepsi warn'la atlanır, plain HTTP.
# Hazır olunca: curl http://127.0.0.1:18090/health → "ok"
```

STT/çeviri key'i gerekmez: `providers.mock_fallback=true` (varsayılan) mock
transcript üretir — ses frame'i gönderince `"[N bytes transcribed]"` döner.

## Yüzeyleri sürme kalıpları

- **Admin oturumu**: `POST /api/auth/login {email,password}` → `{token}`.
  Config: `PUT /admin/config {key,value}` · Secret: `PUT /admin/secrets/{NAME}
  {value}` (ikisi de restart'sız devreye girer).
- **SDK cihaz token'ı (gerçek yol)**: `POST /api/pairing/code` (auth) → 6 haneli
  kod → `POST /api/pairing/claim {pairing_code, device_name}` (auth'suz) → JWT.
- **SDK ses WS**: `ws://…/ws/stream` → ilk mesaj `{"type":"auth","token"}` →
  `stream_start` → binary frame: `"SDAU" + 0x01 0x00 + u16BE len + payload`.
- **Node 21+ ile WS sürmek**: global `WebSocket` yeterli, paket gerekmez.
  Reddedilen WS upgrade'inin HTTP kodunu görmek için `node:http` ile
  `Connection: Upgrade` başlıklı ham istek at (`response` event'i 4xx'i verir).
- **Twitch Extension JWT üretimi**: HS256, secret **base64** — Node'da
  `crypto.createHmac("sha256", rawSecret)` + base64url; claim'ler
  `{exp, channel_id, role, opaque_user_id}`.

## Hazır E2E akışı (Twitch Extension, 21 adım)

Tam driver örneği bir kez yazıldı ve şu akışı sürer: gate 503 → panel toggle +
secret → kod üret → bind → 404/409/403/401 probe'ları → pairing → SDK subtitle
→ viewer state/WS (snapshot replay + canlı) → stream_stop → unlink → log
redaction taraması → toggle off. Yeniden gerekirse aynı sırayı izle; kritik
assert'ler: viewer mesajında `overlay_token` OLMAMALI, `/admin/logs` çıktısında
secret/JWT OLMAMALI.

## Bilinenler / tuzaklar

- Kabuk fish'tir: `for … end` bash döngüleri `bash -c '…'` içinde çalıştır.
- Port çakışmasına karşı 18090 gibi yüksek port seç; işi bitince
  `pkill -f target/debug/streamdubber-backend`.
- Frontend (Next.js) dashboard kartları admin token'ını `localStorage
  "sd.admin.token"` anahtarında arar — tarayıcısız doğrulamada kartın çağırdığı
  uçları (ör. `/api/ext/twitch/code|binding`) doğrudan sürmek eşdeğerdir.
