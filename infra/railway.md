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
  - `NEXT_PUBLIC_API_URL` = backend servisinin public URL'i (örn.
    `https://api.streamdubber.app`)
  - `NEXT_PUBLIC_OVERLAY_WS_URL` = aynı host `wss://` şemasıyla (örn.
    `wss://api.streamdubber.app`)

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
