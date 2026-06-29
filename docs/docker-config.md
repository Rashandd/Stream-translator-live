# Docker ve Konfigürasyon Kararları

## Docker Yapısı
```
/backend/Dockerfile      → multi-stage (cargo build → distroless runtime)
/frontend/Dockerfile     → multi-stage (npm build → nginx)
/sdk/                    → Tauri native build, Docker yok
/infra/docker-compose.yml → local dev ortamı
```

## docker-compose (local dev)
```yaml
services:
  backend, frontend, postgres, redis, vault
```
Tüm servisler `.env` dosyasından konfigüre edilir.
`.env.example` commit'e girer, `.env` .gitignore'da.

## Konfigürasyon Katmanı
Admin panelden toggle edilebilen her şey DB'de `service_config` tablosunda:
```
key: "stt.provider"     value: "deepgram"   # veya "whisper"
key: "tts.provider"     value: "elevenlabs" # veya "xtts"
key: "translate.provider" value: "deepl"    # veya "hy-mt1"
key: "stt.fallback"     value: "assemblyai"
```
Kod her zaman `config::get("stt.provider")` ile okur, restart gerektirmez.

## Fallback Kuralları (admin panelden)
```
STT:     deepgram → assemblyai → whisper
Çeviri:  deepl → google
TTS:     elevenlabs → cartesia → xtts
```
Kural değişince anında devreye girer.

## Env Var Listesi (hard-coded olmaması gerekenler)
```
DATABASE_URL, REDIS_URL, VAULT_ADDR, VAULT_TOKEN
DEEPGRAM_KEY (Vault'tan), DEEPL_KEY (Vault'tan)
ELEVENLABS_KEY (Vault'tan)
RAILWAY_TOKEN, HETZNER_API_KEY
JWT_SECRET (Vault'tan)
```
