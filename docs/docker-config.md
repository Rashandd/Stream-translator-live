# Docker ve Konfigürasyon Kararları

## Docker Yapısı
```
/backend/Dockerfile      → multi-stage (cargo build → distroless runtime)
/frontend/Dockerfile     → multi-stage (npm ci → next build → node standalone)
/sdk/                    → Tauri native build, Docker yok
/infra/docker-compose.yml → local dev ortamı
/infra/deploy.md         → cloud deploy rehberi (Vercel + VPS, MVP)
/infra/railway.md        → eski Railway rehberi (ARŞİV)
```
> Not: Frontend runtime'ı nginx değil **node standalone** — SSR/route
> handler'lar node gerektirir; `next.config.mjs` `output: "standalone"`.
> `NEXT_PUBLIC_*` değerleri tarayıcıya gömüldüğü için **build-arg**'dır.

## docker-compose (local dev)
```yaml
services:
  backend, frontend, postgres, redis, vault
volumes:
  pgdata, sddata (backend /data: SECRETS_FILE + LOG_FILE kalıcılığı)
```
Tüm servisler `.env` dosyasından konfigüre edilir.
`.env.example` commit'e girer, `.env` .gitignore'da.

## Konfigürasyon Katmanı
Admin panelden toggle edilebilen her şey DB'de `service_config` tablosunda:
```
key: "stt.provider"     value: "selfhosted"
key: "tts.provider"     value: "xtts"
key: "translate.provider" value: "deepl"    # veya "hy-mt1"
```
Kod her zaman `config::get("stt.provider")` ile okur, restart gerektirmez.

## Fallback Kuralları (config API, `PUT /admin/config` — ayrı bir panel sayfası yok)
Ürün kararıyla kullanılacak motorlar:
```
STT:     selfhosted faster-whisper-large-v3
Çeviri:  deepl → google
TTS:     self-hosted XTTS-v2 (alternatif araştırma: Pocket TTS, StyleTTS2)
```
TTS motoru henüz üretim hattına bağlı değil; API tabanlı TTS sağlayıcıları
ürün kapsamı dışındadır. Kural değişince anında devreye girer.

## Env Var Listesi (hard-coded olmaması gerekenler)
```
DATABASE_URL, REDIS_URL, VAULT_ADDR, VAULT_TOKEN
DEEPL_KEY, ... (Vault'tan VEYA admin panelden)
HETZNER_API_KEY (VPS/GPU otomasyonu gerekirse)
CORS_ALLOWED_ORIGINS (frontend origin listesi — Vercel↔VPS cross-origin)
JWT_SECRET (env/Vault — panelden yönetilmez)
ADMIN_EMAIL, ADMIN_PASSWORD_HASH (bootstrap admin girişi)
SECRETS_FILE, SECRETS_ENC_KEY (panel secret'larının şifreli kalıcılığı)
LOG_FILE (merkezi log deposu)
NEXT_PUBLIC_API_URL, NEXT_PUBLIC_OVERLAY_WS_URL (frontend build-arg)
```

## Secret Girişi — İki Yol
1. **Admin panel (MVP varsayılanı):** deploy sonrası `/admin/secrets` —
   restart'sız devreye girer, sadece baş+son maskeli okunur, `SECRETS_FILE`
   ile AES-256 şifreli kalıcı.
2. **Vault:** `VAULT_ADDR`+`VAULT_TOKEN` set edilir, `scripts/vault-seed.sh`
   ile `.env`'den yüklenir. Panel girişleri Vault'un önüne geçer.
