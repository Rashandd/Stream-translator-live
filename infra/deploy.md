# Deploy Rehberi — Vercel (frontend) + VPS (backend) (Faz 1 MVP)

> Karar (2026-07-05): Faz 1 deploy hedefi **frontend → Vercel**,
> **backend → VPS (Docker)**. Önceki Railway planı `infra/railway.md`'de
> arşiv olarak duruyor; içerik güncelliğini yitirdi, bu dosya geçerlidir.

Video buluta çıkmaz; bu deploy yalnızca ses/altyazı API'sini ve panelleri
taşır. Frontend statik/SSR sayfa render eder, **tüm** API/WebSocket trafiği
tarayıcıdan doğrudan `api.strovak.com`'a gider (aşağıdaki kritik kural).

## Mimari özet

| Parça | Nerede | Domain |
| --- | --- | --- |
| Next.js frontend | Vercel | `strovak.com`, `www.strovak.com` |
| Rust backend (Docker) | VPS | `api.strovak.com` |
| PostgreSQL + Redis | VPS (aynı compose) | iç ağ |
| Twitch Ext EBS (Faz 2) | aynı backend | `ext.strovak.com` |

## 1. Frontend — Vercel

- New Project → GitHub repo → **Root Directory: `frontend`** (monorepo).
  Framework: Next.js, build komutları varsayılan.
- Environment Variables (build'e gömülür — değişince redeploy gerekir):
  - `NEXT_PUBLIC_API_URL` = `https://api.strovak.com`
  - `NEXT_PUBLIC_OVERLAY_WS_URL` = `wss://api.strovak.com`
- Custom domain: `strovak.com` + `www.strovak.com` (Vercel DNS yönergesi).
- `next.config.mjs`'teki `output: "standalone"` Vercel'de zararsızdır
  (Vercel kendi çıktısını üretir); lokal Docker/compose için gerekli, silme.

> **KRİTİK KURAL — frontend API proxy'si OLMAZ.** Dashboard/panel istekleri
> (fetch + overlay WebSocket) tarayıcıdan **doğrudan `api.strovak.com`'a**
> gider — Next.js sunucusu (Vercel fonksiyonları) üzerinden **asla** geçmez:
> - `next.config.mjs`'e `rewrites()`/`redirects()` ile backend'e proxy
>   EKLEME; `frontend/app/**` altına backend'e istek yönlendiren `route.ts`
>   EKLEME.
> - Tüm API/WebSocket çağrıları `"use client"` bileşenlerde kalmalı
>   (`src/lib/api.ts` — `adminFetch`/`userFetch`/`API_URL` deseni).
> - Sebep: ses/API trafiği (özellikle altyazı WebSocket akışı) Vercel
>   fonksiyonlarından geçerse hem Vercel bandwidth/invocation faturasına
>   yansır hem de maliyet izolasyonu bozulur. `api.strovak.com` VPS'te
>   olduğu için trafik oradan sabit maliyetle ölçülür.

### CORS (frontend ↔ API ayrı origin)

Tarayıcı istekleri cross-origin olduğundan backend preflight'ı
`cors.allowed_origins` config'ine göre yanıtlar. İlk deploy'da env ile
tohumla (panel de CORS'a tabidir — env olmadan panele giremezsin):

```
CORS_ALLOWED_ORIGINS=https://strovak.com,https://www.strovak.com
```

Vercel preview'larını da test edeceksen preview domain'ini listeye ekle
(admin panelden `cors.allowed_origins` restart'sız güncellenir).

## 2. Backend — VPS (Docker)

Öneri: 2 vCPU / 4 GB RAM / NVMe'li herhangi bir VPS (Hetzner CX/CPX gibi).
Faz 2 GPU işleri zaten Hetzner'e planlı; aynı sağlayıcıda kalmak ağ
basitliği sağlar.

### 2a. Sunucu hazırlığı (bir kez)

```bash
# Docker + compose eklentisi (Ubuntu/Debian)
curl -fsSL https://get.docker.com | sh
# Güvenlik duvarı: yalnız SSH + HTTP(S)
ufw allow OpenSSH && ufw allow 80/tcp && ufw allow 443/tcp && ufw enable
```

- DNS: `api.strovak.com` A kaydı → VPS IP'si. (Faz 2'de `ext.strovak.com`
  aynı IP'ye eklenir — ayrı deploy gerekmez, bkz. §7.)

### 2b. Reverse proxy — Caddy (TLS otomatik)

Backend container'ı düz HTTP dinler (`TLS_CERT_PATH`/`TLS_KEY_PATH` boş);
TLS'i Caddy Let's Encrypt ile üstlenir ve WebSocket'i şeffaf geçirir:

```caddyfile
# /etc/caddy/Caddyfile — host'ta veya compose'a caddy servisi olarak
api.strovak.com {
    reverse_proxy 127.0.0.1:8080
}
```

Nginx tercih edersen `proxy_set_header Upgrade/Connection` (WebSocket)
ayarlarını unutma; Caddy bunları otomatik yapar. SDK↔bulut mTLS gerektiğinde
backend'in kendi TLS dinleyicisi (TLS_* env'leri) devreye alınır ve Caddy
o port için `tcp` passthrough'a çevrilir — Faz 1'de gerekmez.

### 2c. Servisleri başlat

```bash
git clone <repo> && cd Stream-translator-live
cp .env.example .env   # aşağıdaki tabloyu doldur
docker compose -f infra/docker-compose.yml up -d --build backend postgres redis
```

Compose'daki `frontend` servisi VPS'te ÇALIŞTIRILMAZ (frontend Vercel'de);
sadece lokal dev içindir. Migration'lar backend açılışında otomatik uygulanır
(`sqlx::migrate!`).

### Zorunlu env değişkenleri (backend)

| Değişken | Değer / Not |
| --- | --- |
| `BIND_ADDR` | `0.0.0.0:8080` |
| `JWT_SECRET` | En az 32 byte rastgele dize |
| `DATABASE_URL` | compose içi: `postgres://streamdubber:...@postgres:5432/streamdubber` |
| `REDIS_URL` | compose içi: `redis://redis:6379` |
| `ADMIN_EMAIL` | Panel girişi için bootstrap admin e-postası |
| `ADMIN_PASSWORD_HASH` | `cargo run --bin hash_password` çıktısı (argon2id) |
| `CORS_ALLOWED_ORIGINS` | `https://strovak.com,https://www.strovak.com` |
| `SECRETS_FILE` | `/data/secrets.enc` — panel secret'ları şifreli kalıcı |
| `SECRETS_ENC_KEY` | 32 byte (64 hex) — boşsa JWT_SECRET'tan türetilir |
| `LOG_FILE` | `/data/logs.jsonl` — /admin/logs kalıcı deposu |
| `USAGE_FILE` | `/data/usage.jsonl` — plan limiti sayaçları kalıcı |
| `TOKENS_FILE` | `/data/tokens.enc` — OAuth token'ları şifreli kalıcı |
| `RELAY_FILE` | `/data/relay.enc` — SDK relay hedefleri (stream key) şifreli |
| `USERS_FILE` | `/data/users.enc` — yayıncı hesapları (1.4) şifreli kalıcı |
| `SDK_SESSIONS_FILE` | `/data/sdk-sessions.enc` — eşleşmiş cihazların canlı presence kaydı (heartbeat secret) şifreli kalıcı |
| `RUST_LOG` | `info` |

Sağlayıcı API key'lerini (`DEEPGRAM_KEY`, `DEEPL_KEY`, Stripe, OAuth client
kimlikleri...) env'e KOYMAK ZORUNDA DEĞİLSİN — deploy sonrası admin panelden
girilir (adım 4). Env'e koyarsan da çalışır; panel girişleri env'in önüne geçer.

### 2d. Kalıcılık ve yedek

- `sddata` volume'u (`/data`) şifreli store'ları taşır — VPS yedeğine dahil et.
- Postgres: `docker compose exec postgres pg_dump -U streamdubber streamdubber`
  çıktısını cron ile günlük al (örn. 02:00, 7 gün rotasyon).
- `SECRETS_ENC_KEY`/`JWT_SECRET`'ı yedekten AYRI, güvenli bir yerde sakla —
  anahtar kaybolursa şifreli dosyalar açılamaz.

### 2e. Güncelleme (deploy)

```bash
git pull
docker compose -f infra/docker-compose.yml up -d --build backend
```

Health: `curl https://api.strovak.com/health` → `ok`.

## 3. Postgres + Redis

Compose içindeki `postgres` ve `redis` servisleri kullanılır (healthcheck'li).
Dışa port açma (5432/6379 UFW'de kapalı kalır); yalnız compose iç ağından
erişilir. Yönetilen DB'ye geçiş istenirse `DATABASE_URL`'i değiştirmek yeterli.

## 4. Deploy sonrası ilk kurulum (sıralı)

1. `https://strovak.com/en/admin/login` → `ADMIN_EMAIL` + şifre ile gir.
2. **Admin → Secrets**: Deepgram/DeepL (+ istersen AssemblyAI, Google,
   Stripe, Streamlabs/YouTube OAuth, SMTP) key'lerini gir — restart gerekmez,
   değerler sadece maskeli okunur.
3. Her key'i satırdaki **Test** butonuyla canlı doğrula; SMTP için
   **Admin → Production Setup**/mail testi kullan. SMTP girildiği anda
   yayıncı login'i otomatik e-posta 2FA'ya geçer (`auth.email_2fa=auto`).
4. **Admin → Service routing**: `providers.mock_fallback` → `false`
   (üretimde key eksikse mock çıktı yerine açık hata).
5. `stripe_price_id`'leri **Admin → Production Setup**'tan gir (Stripe
   Dashboard'da ürün/fiyat oluşturduktan sonra).
6. Durumu **Admin → Production Setup** kartlarından doğrula (hepsi yeşil).

## 5. Vault (opsiyonel yol)

Secret'ları panel yerine Vault'tan yönetmek istersen: `VAULT_ADDR` +
`VAULT_TOKEN` (+`VAULT_MOUNT`, varsayılan `secret`) env'lerini ver ve
`scripts/vault-seed.sh` ile `.env`'deki değerleri Vault KV v2'ye yükle.
Vault set edildiğinde backend taban store olarak Vault'u kullanır; panel
girişleri yine önceliklidir.

## 6. Sağlık ve loglar

- `GET /health` → `ok` (Caddy/uptime izlemesi buna bağlanır).
- Uygulama logları: **Admin → Logs** (kalıcı: `LOG_FILE`), seviye
  `log.level` config anahtarından restart'sız değişir.
- Container logları: `docker compose logs -f backend`.

## 7. Twitch Extension EBS domaini (Faz 2.4 — A4 kararı)

Extension viewer trafiği `api.strovak.com`'dan AYRI bir hostname'den servis
edilir — hem Twitch URL allowlist'ine dar, extension'a özel bir origin yazmak
hem de viewer fan-out maliyetini ayrı ölçmek için:

| Ortam | Hostname | Not |
|---|---|---|
| Production | `ext.strovak.com` | Twitch allowlist: `https://ext.strovak.com` + `wss://ext.strovak.com` |
| Staging | `ext-staging.strovak.com` | İç test |
| Hosted Test | `ext-test.strovak.com` | Twitch Hosted Test CSP doğrulaması |

Uygulama: aynı backend servisi (route ailesi `/ext/twitch/*`); VPS'te Caddy'ye
ek bir site bloğu (`ext.strovak.com { reverse_proxy 127.0.0.1:8080 }`) ve DNS
A kaydı yeterli — ayrı deploy gerekmez. `extension/js/shared.js` içindeki
`EBS_BASE_PRODUCTION` sabiti ve Twitch console allowlist'i birlikte
güncellenmelidir (her version submission'da birebir aynı olmak zorunda).

Kurulum sırası (Faz 2 aktivasyonu):
1. Admin → Secrets: `TWITCH_EXT_SECRET` (Twitch console'daki base64 secret).
2. Admin → Config: `ext.twitch.enabled` → `true` (restart yok).
3. Rotasyonda eski secret `TWITCH_EXT_SECRET_PREVIOUS`'a taşınır.
