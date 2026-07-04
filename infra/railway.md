# Railway Deploy Rehberi (Faz 1 MVP)

İki servis + iki eklenti: `backend` (Rust, Dockerfile), `frontend` (Next.js,
Dockerfile), Railway **PostgreSQL** ve **Redis** eklentileri. Video buluta
çıkmaz; bu deploy yalnızca ses/altyazı API'sini ve panelleri taşır.

## 1. Backend servisi

- New Service → GitHub repo → **Root Directory: `/`** (workspace kökü),
  **Dockerfile Path: `backend/Dockerfile`**.
- Volume ekle: mount path **`/data`** (panelden girilen secret'lar, loglar).
- Health check: Railway ayarlarında path **`/health`** (container içi
  HEALTHCHECK de tanımlı).

### Zorunlu env değişkenleri

| Değişken | Değer / Not |
| --- | --- |
| `BIND_ADDR` | `0.0.0.0:8080` (Railway `PORT` atarsa `0.0.0.0:$PORT`) |
| `JWT_SECRET` | En az 32 byte rastgele dize |
| `DATABASE_URL` | Railway Postgres eklentisinin `DATABASE_URL`'i |
| `REDIS_URL` | Railway Redis eklentisinin `REDIS_URL`'i |
| `ADMIN_EMAIL` | Panel girişi için bootstrap admin e-postası |
| `ADMIN_PASSWORD_HASH` | `cargo run --bin hash_password` çıktısı (argon2id) |
| `SECRETS_FILE` | `/data/secrets.enc` — panel secret'ları şifreli kalıcı |
| `SECRETS_ENC_KEY` | 32 byte (64 hex) — boşsa JWT_SECRET'tan türetilir |
| `LOG_FILE` | `/data/logs.jsonl` — /admin/logs kalıcı deposu |
| `USAGE_FILE` | `/data/usage.jsonl` — plan limiti sayaçları kalıcı |
| `TOKENS_FILE` | `/data/tokens.enc` — OAuth token'ları şifreli kalıcı |
| `RELAY_FILE` | `/data/relay.enc` — SDK relay hedefleri (stream key) şifreli |
| `RUST_LOG` | `info` |

Sağlayıcı API key'lerini (`DEEPGRAM_KEY`, `DEEPL_KEY`, Stripe, OAuth client
kimlikleri...) env'e KOYMAK ZORUNDA DEĞİLSİN — deploy sonrası admin panelden
girilir (aşağıda adım 4). Env'e koyarsan da çalışır; panel girişleri env'in
önüne geçer.

TLS: Railway edge TLS terminasyonu yapar → `TLS_CERT_PATH`/`TLS_KEY_PATH`
boş bırakılır (backend düz HTTP dinler, dışarısı HTTPS'tir). SDK↔bulut mTLS
kendi TLS dinleyicimizi gerektirdiğinde Hetzner'e (Faz 2) taşınır.

## 2. Frontend servisi

- New Service → aynı repo → **Root Directory: `frontend`**, Dockerfile
  otomatik bulunur.
- Build arg / env (build sırasında gömülür, sonradan değiştirmek yeniden
  build ister):
  - `NEXT_PUBLIC_API_URL` = `https://api.strovak.com`
  - `NEXT_PUBLIC_OVERLAY_WS_URL` = `wss://api.strovak.com`

> **KRİTİK KURAL — frontend API proxy'si OLMAZ.** Dashboard/panel istekleri
> (fetch + overlay WebSocket) tarayıcıdan **doğrudan `api.strovak.com`'a**
> gider — Next.js sunucusu (Railway'deki frontend servisi) üzerinden
> **asla** geçmez. Bu yüzden:
> - `next.config.mjs`'e `rewrites()`/`redirects()` ile backend'e proxy
>   EKLEME; `frontend/app/**` altına backend'e istek yönlendiren bir
>   `route.ts` (Next.js Route Handler) EKLEME.
> - Tüm API/WebSocket çağrıları `"use client"` bileşenlerde kalmalı
>   (`src/lib/api.ts`'teki `adminFetch`/`API_URL` deseni — bkz.
>   `admin-login-form.tsx`, `admin-live-status.tsx`,
>   `app/o/[token]/page.tsx`), sunucu bileşeninde backend'e fetch YOK.
> - Sebep: ses/API trafiği (özellikle altyazı WebSocket akışı) Railway'in
>   frontend servisinden geçerse Railway faturasına (bandwidth) yansır ve
>   maliyet kontrolden çıkar. `api.strovak.com` ayrı bir backend
>   servisi/domain'i olduğu için trafik oradan ölçülür, frontend sadece
>   statik/SSR sayfa render eder.

## 3. Postgres + Redis

- Add Plugin → PostgreSQL → `DATABASE_URL`'i backend'e referansla.
- Add Plugin → Redis → `REDIS_URL`'i backend'e referansla.
- Migration'lar backend açılışında otomatik uygulanır (`sqlx::migrate!`).

## 4. Deploy sonrası ilk kurulum (sıralı)

1. `https://<frontend>/en/admin/login` → `ADMIN_EMAIL` + şifre ile gir.
2. **Admin → Secrets**: Deepgram/DeepL (+ istersen AssemblyAI, Google,
   Stripe, Streamlabs/YouTube OAuth) key'lerini gir — restart gerekmez,
   değerler sadece maskeli okunur.
3. Her key'i "Test" ile canlı doğrula (V5 sağlayıcı testi geldiğinde).
4. **Admin → Service routing**: `providers.mock_fallback` → `false`
   (üretimde key eksikse mock çıktı yerine açık hata).
5. `plans` tablosundaki `stripe_price_id`'leri panelden gir (Stripe
   Dashboard'da ürün/fiyat oluşturduktan sonra).

## 5. Vault (opsiyonel yol)

Secret'ları panel yerine Vault'tan yönetmek istersen: `VAULT_ADDR` +
`VAULT_TOKEN` (+`VAULT_MOUNT`, varsayılan `secret`) env'lerini ver ve
`scripts/vault-seed.sh` ile `.env`'deki değerleri Vault KV v2'ye yükle.
Vault set edildiğinde backend taban store olarak Vault'u kullanır; panel
girişleri yine önceliklidir.

## 6. Sağlık ve loglar

- `GET /health` → `ok` (her iki healthcheck'in kaynağı).
- Uygulama logları: **Admin → Logs** (kalıcı: `LOG_FILE`), seviye
  `log.level` config anahtarından restart'sız değişir.

## 7. Twitch Extension EBS domaini (Faz 2.4 — A4 kararı)

Extension viewer trafiği `api.strovak.com`'dan AYRI bir hostname'den servis
edilir — hem Twitch URL allowlist'ine dar, extension'a özel bir origin yazmak
hem de viewer fan-out maliyetini ayrı ölçmek için (frontend proxy yasağıyla
aynı izolasyon ilkesi):

| Ortam | Hostname | Not |
|---|---|---|
| Production | `ext.strovak.com` | Twitch allowlist: `https://ext.strovak.com` + `wss://ext.strovak.com` |
| Staging | `ext-staging.strovak.com` | İç test |
| Hosted Test | `ext-test.strovak.com` | Twitch Hosted Test CSP doğrulaması |

Uygulama: aynı backend servisi (route ailesi `/ext/twitch/*`), Railway'de ek
custom domain olarak bağlanır — ayrı deploy gerekmez; ölçüm domain bazında
yapılır. DNS bu hostname'lere işaret edene kadar `extension/js/shared.js`
içindeki `EBS_BASE_PRODUCTION` sabiti ve Twitch console allowlist'i birlikte
güncellenmelidir (her version submission'da birebir aynı olmak zorunda).

Kurulum sırası (Faz 2 aktivasyonu):
1. Admin → Secrets: `TWITCH_EXT_SECRET` (Twitch console'daki base64 secret).
2. Admin → Config: `ext.twitch.enabled` → `true` (restart yok).
3. Rotasyonda eski secret `TWITCH_EXT_SECRET_PREVIOUS`'a taşınır.
