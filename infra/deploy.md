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
    tls {
        protocols tls1.3
    }
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
| `FOUNDING_APPS_FILE` | `/data/founding-apps.json` — founding beta başvuruları + onay durumu (düz JSON); boşsa onaylar restart'ta kaybolur |
| `STT_SERVERS_FILE` | `/data/stt-servers.enc` — self-hosted GPU STT worker kayıtları (endpoint + bearer token) şifreli kalıcı; boşsa her backend redeploy'unda worker'ı admin panelden yeniden eklemen/provizyon etmen gerekir (bkz. §8) |
| `STREAM_SETTINGS_FILE` | `/data/stream-settings.json` — yayıncının konuşma/hedef dil seçimi (2.0) düz JSON kalıcı; boşsa restart'ta yeniden seçilmesi gerekir |
| `VOICE_MODELS_FILE` | `/data/voice-models.json` — ses klonlama model kayıtları (2.1) düz JSON kalıcı; boşsa restart'ta kayıtlar kaybolur |
| `TTS_SETTINGS_FILE` | `/data/tts-settings.json` — yerel TTS modu + benchmark sonucu (2.2b) düz JSON kalıcı; boşsa restart'ta varsayılana döner |
| `VOICE_SAMPLES_FILE` | `/data/voice-samples.enc` — ham ses örneği (2026-07-06 kararı) AES-256-GCM şifreli kalıcı; boşsa restart'ta kaybolur (dev/test kabul edilebilir, betada mutlaka set et) |
| `RUST_LOG` | `info` |

Sağlayıcı API key'lerini (`DEEPGRAM_KEY`, `DEEPL_KEY`, Stripe, OAuth client
kimlikleri...) env'e KOYMAK ZORUNDA DEĞİLSİN — deploy sonrası admin panelden
girilir (adım 4). Env'e koyarsan da çalışır; panel girişleri env'in önüne geçer.

**Not — `REDIS_URL`:** compose'da `redis` servisi hazır bekliyor ama backend
HENÜZ ona bağlanmıyor (kodda hiçbir referans yok) — hardening planı BH-04/06
(`docs/backend-hardening-captions-plan.md`) bağlanınca zorunlu env listesine
eklenecek. Bugün ayarlaman gerekmiyor.

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
4. `providers.mock_fallback` → `false` (üretimde key eksikse mock çıktı
   yerine açık hata) — bu anahtarın artık panelde yüzeyi yok, config API'den
   ayarlanır (bkz. §8'deki `PUT /admin/config` örneği; `key` olarak
   `providers.mock_fallback`, `value` olarak `false` gönder).
5. `stripe_price_id`'leri **Admin → Production Setup**'tan gir (Stripe
   Dashboard'da ürün/fiyat oluşturduktan sonra).
6. Durumu **Admin → Production Setup** kartlarından doğrula (hepsi yeşil).

### 4b. "Founding 5" kapalı beta operasyonu (Stripe öncesi ilk test)

Stripe'sız, tek GPU STT worker'lı ilk test için akış:

1. **Admin → Production Setup → Founding beta** kartından:
   - `founding.gate.enabled` → **açık** (o andan itibaren onaysız kullanıcı
     yayın başlatamaz — `founding_approval_pending` hatası alır; dashboard'ı
     gezebilir, SDK'yı kurabilir, başvurusunu yapabilir).
   - `promo.founding.enabled` → **açık** + mesajı düzenle (pricing sayfasında
     fiyatların üstü çizilir, banner bu mesajı gösterir). Restart gerekmez.
2. Yayıncı `/signup` ile hesap açar, dashboard ana sayfasındaki **Founding
   beta access** kartından platform + kanal linkiyle başvurur.
3. **Admin → Users → Founding beta applications** tablosundan başvuruları
   incele, seçtiklerini **Approve** et — onay anında etkilidir (kullanıcının
   yeniden login olması gerekmez), kullanıcı `founding` planının limitleriyle
   (varsayılan: 50 saat/ay, 3 dil, watermark'sız — `PUT /admin/plans` ile
   ayarlanır) ücretsiz yayın açar. **Revoke** de aynı şekilde anında keser.
4. Beta bitince: `founding.gate.enabled` → kapalı, `promo.founding.enabled`
   → kapalı. Onaylı founding kayıtları durur ama gate kapalıyken etkisizdir.

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

## 8. GPU worker (self-hosted STT) — OBS overlay için 3. makine

Bu bölüm, yayıncının OBS'e Browser Source olarak yapıştırdığı overlay
URL'inde (`https://strovak.com/o/{token}`) canlı altyazı görmesi için
gereken üçüncü makineyi (NVIDIA GPU'lu bir sunucu) uçtan uca anlatır.
Overlay zincirinin kendisi (SDK → backend WS → `OverlayHub` → `/overlay/{token}`
→ bu frontend sayfası) kod olarak hazır ve tek backend instance'ı için
çalışır durumda; eksik olan yalnızca bu worker'ın devreye alınması ve
backend'e onu kullanmasının söylenmesidir.

### 8a. Worker makinesi

- NVIDIA GPU'lu bir sunucu (Vast.ai, Hetzner GPU, veya benzeri), SSH erişimi.
- Admin panelden ekle: **Admin → STT servers** (`/admin/stt-servers`) →
  yeni sunucu → host/SSH bilgileri. Backend, `infra/stt-server/bootstrap.sh`'ı
  SSH üzerinden çalıştırıp Docker (varsayılan) veya bare-venv modunda
  `stt-server/main.py` (FastAPI + faster-whisper) kurar ve `mark_ready` ile
  endpoint + bearer token'ı kaydeder (`STT_SERVERS_FILE`'a şifreli yazılır —
  §"Zorunlu env değişkenleri" tablosunda, bu env boşsa kayıt her backend
  redeploy'unda kaybolur).
- NAT arkasındaki sağlayıcılar (ör. Vast.ai port mapping) için sunucu
  ekleme formundaki **Public endpoint override** alanını doldur — backend
  bu adresi SSH'tan gördüğü iç adres yerine kullanır.
- Worker varsayılan olarak `8443` portunda dinler (`STT_PORT`,
  `bootstrap.sh`). Bootstrap script kendi `ufw allow ${STT_PORT}/tcp`
  kuralını ekler — bu varsayılan olarak dünyaya açıktır (erişim bearer
  token'la korunur ama savunma derinliği için) — **öneri:** bulut
  sağlayıcının güvenlik grubunda bu portu yalnızca VPS'in IP'sine kısıtla.

### 8b. Backend'e worker'ı kullanmasını söyle (elle adım — otomatik değil)

Worker `Ready` durumuna geçtikten sonra backend onu **otomatik kullanmaya
başlamaz** — varsayılan `stt.provider=deepgram`'dır ve
`docs/backend-hardening-captions-plan.md`'de tarif edilen otomatik geçiş
(`stt.selfhosted.autoactivate`) henüz implemente edilmedi. Eskiden bu
anahtar **Admin → Service routing** panelinden değiştirilirdi; o sayfa
kaldırıldığı için (tek karşılığı yoktu, config API'nin üstünde ince bir
katmandı) artık doğrudan config API'den değiştirilir:

```bash
# Tarayıcısız (curl) çağrılar Origin göndermez, bu yüzden login klasik
# bearer token döner (browser-only cookie akışının aksine) — script/CI
# için tam uygun.
TOKEN=$(curl -fsS -X POST https://api.strovak.com/api/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"email":"<ADMIN_EMAIL>","password":"<ADMIN_PASSWORD>"}' \
  | jq -r .token)

curl -fsS -X PUT https://api.strovak.com/admin/config \
  -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  -d '{"key":"stt.provider","value":"selfhosted"}'
```

`stt.fallback` (varsayılan `assemblyai`) worker çökerse devreye girer —
o sağlayıcının key'i **Admin → Secrets**'te yoksa ve
`providers.mock_fallback=false` ise (bkz. §4 adım 4) altyazı worker
kesintisinde tamamen durur; kabul edilebilir bir MVP riski, ama bilerek.

### 8c. Doğrulama

1. `curl -H "Authorization: Bearer <worker-token>" https://<worker>:8443/health`
   → GPU adı/VRAM döner.
2. Admin panelde sunucu durumu **Ready**.
3. `stt.provider=selfhosted` ayarlandıktan sonra bir test yayını başlat
   (SDK ile) ve worker loglarında (`docker compose logs -f` worker'da veya
   `stt-server.log`) `/transcribe` isteklerinin düştüğünü doğrula.
4. `https://strovak.com/o/{token}` sayfasını (ve aynı URL'i OBS Browser
   Source olarak) açıp canlı altyazının worker'dan geldiğini teyit et.

### 8d. Bilinen MVP sınırları (kod değişikliği gerektirmez, bilerek kabul edilmiş)

- **Overlay token her stream'de yeniden üretilir** (`ws/mod.rs`, UUID) —
  yayıncı her yayın başlangıcında `/o/{token}` URL'ini OBS'e yeniden
  yapıştırmalı. Kalıcı token Postgres'e taşınana kadar (hardening planı
  Bölge D1) böyle kalacak.
- **Overlay/aktif-stream durumu bellek-içi, tek instance'a bağlı** —
  backend restart'ı canlı yayınları düşürür; yatay ölçekleme öncesi
  Redis'e taşınması gerekir (hardening planı §6.1/BH-04/BH-06).
- **Worker'a periyodik health check yok** — yalnız provizyon anında bir
  kez kontrol edilir; worker yayın ortasında çökerse sessizce
  `stt.fallback`'e düşer (ya da fallback yoksa altyazı durur).
