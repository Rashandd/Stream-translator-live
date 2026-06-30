# Claude Session

## Agent
Claude (claude-sonnet-4-6)

## Neredeyim?
Faz 1.1 (6/6) + Faz 1.2 (güvenlik) + Faz 1.3 ilk 3 task bitti.
Backend: 54 unit + 3 integration. SDK: 18 unit. Workspace clippy `-D warnings` temiz.
TLS 1.3 + mTLS curl ile doğrulandı.

## Faz 1.3 — Yapılan (bu oturum, Codex iskeletinin üstüne)
- Codex'in 1.3 task-1'i (Tauri kurulumu + platform-bağımsız çekirdek) doğrulandı: builds, 7 test, clippy temiz.
- `core/capture.rs` — CaptureSource trait + CaptureConfig (16k mono, 20ms frame) + MockCapture
- `audio/open_capture` — platform dispatch; WASAPI aktif nokta (NotImplemented stub, Windows'ta yazılacak), CoreAudio/PipeWire iskelet
- `core/vad.rs` — RMS enerji VAD + hangover (VadGate), boş ses tespiti
- `core/gate.rs` — CapturePipeline: capture→VAD→backend frame, sessizlik dropped (STT maliyeti ↓), GateStats

## Faz 1.3 Açık / Dikkat
- Gerçek WASAPI cihaz I/O yazılmadı (Linux'ta test edilemez) — capture trait + MockCapture ile akış tam test edildi; cihaz binding Windows'ta (cpal/windows crate).
- Sıradaki: gürültü temizleme/ön-işleme, video+ses birleştirme, bulut WebSocket client (sadece ses).
- ÖNEMLİ — multi-agent: SDK çalışma ağacında commit'siz. Codex iskeleti + Claude'un capture/vad/gate eklemeleri + workspace Cargo.toml member + backend dosyalarında rustfmt diff'leri karışık. Commit koordinasyonu kullanıcı/Codex ile yapılmalı.

## Faz 1.2 — Yapılan (bu oturum)
- `crypto.rs` — AES-256-GCM at-rest (Cipher: encrypt/decrypt, nonce||ct base64), 6 test
- `secrets/mod.rs` — SecretStore trait + EnvSecretStore + VaultSecretStore skeleton; JWT secret artık SecretStore'dan
- `tls.rs` — rustls TLS 1.3-only ServerConfig + opsiyonel mTLS (WebPkiClientVerifier); explicit aws-lc-rs provider (ring+aws-lc-rs çakışması için)
- `main.rs` — TLS/plain serve dallanması (axum-server bind_rustls), `--health-check` self-probe (distroless), JWT secret SecretStore'dan
- `config.rs` — jwt_secret çıkarıldı (artık SecretStore'da); bind_addr + database_url kaldı
- `tests/ws_integration.rs` — gerçek tokio-tungstenite client: full flow + bad auth + audio-before-start
- Infra: `infra/docker-compose.yml` (backend+postgres+redis+vault), `backend/Dockerfile` (multi-stage distroless + healthcheck), `.env.example`, `infra/tls/gen-dev-certs.sh`

## 1.2 Eksik / kısmi
- "API key'leri Vault'a taşı": SecretStore + EnvSecretStore hazır, ama Vault KV v2 HTTP okuması yazılmadı (VaultSecretStore skeleton). Vault servisi compose'da var ama kod ondan okumuyor — env kullanıyor.
- AES at-rest primitifi hazır; _encrypted kolonlara fiili uygulama (TOTP/OAuth token/ses modeli/stream key) ilgili özellikler gelince yapılacak.

## Faz 1.1 — Yapılan (bu oturum)
- `Cargo.toml` (workspace) + `backend/` crate — Axum 0.8, Tokio, serde, sqlx, jsonwebtoken
- `ws/frame.rs` — binary SDAU frame parser (8B header, max 4096), TDD
- `ws/protocol.rs` — Client/Server mesaj serde tagged enum'ları + error kodları
- `ws/mod.rs` — auth handshake (5sn timeout) → session_ready → stream_start/stop → audio→subtitle
- `auth/jwt.rs` — HS256 issue/verify, exp doğrulama (secret env'den)
- `auth/authz.rs` — Role(broadcaster/admin), AuthUser/AdminUser Axum extractor'ları
- `gateway/` — SttProvider/TranslateProvider trait'leri, config-driven seçim + fallback, paralel çeviri (JoinSet), Mock impl'ler
- `pipeline/mod.rs` — StreamSession orchestration (frame→gateway→subtitle, sequence sayacı)
- `config.rs` — Config (env: BIND_ADDR/JWT_SECRET/DATABASE_URL) + ServiceConfig (config::get deseni)
- `db/mod.rs` + `migrations/0001_init.sql` — schema.md'deki 9 tablo, sqlx migrate
- `app.rs`/`main.rs` — router (/health, /ws/stream, /admin/ping), state kurulumu, graceful shutdown

## Son Kararlar
- STT/DeepL gerçek entegrasyonu YOK (1.7/1.8 taskları) — şimdilik MockStt/MockTranslate, aynı trait'lerle takılacak.
- session_ready.remaining_seconds = -1 (bilinmiyor/sınırsız) — usage tracking 1.11'de gelince hesaplanacak.
- Transit AES yok (TLS 1.3 kararıyla uyumlu); JWT secret env'den, koda gömülmedi.

## Degistirdigim Alanlar
- `CLAUDE.md` — şifreleme stack satırı düzeltildi (AES-256-GCM → TLS 1.3 transit + AES-256 at-rest)
- `AGENTS.md` — paralel görev stratejisi, error recovery tablosu eklendi; referans tablosuna yeni dökümanlar eklendi
- `streamdubber_roadmap_final.md` — tüm "Süre: X ay" satırları kaldırıldı; 1.5/1.6 numara tekrarı düzeltildi (STT→1.7, Çeviri→1.8, YouTube CC→1.10, Stripe→1.11); RTMP relay Faz 1 görev listesinden çıkarıldı; güvenlik tablosundan ses akışı AES satırı kaldırıldı
- `docs/commands/security-check.md` — AES kararıyla senkronize edildi, adım numaraları düzeltildi
- `docs/commands/fix-issue.md` — `disable-model-invocation: true` kaldırıldı
- `docs/commands/test-pipeline.md` — `disable-model-invocation: true` kaldırıldı
- `docs/websocket-protocol.md` — yeni oluşturuldu (tam SDK↔Backend protokol spec)
- `docs/schema.md` — yeni oluşturuldu (9 tablo, migration stratejisi)

## Son Kararlar
- Transit şifreleme: TLS 1.3 yeterli, üstüne AES eklenmez. AES-256 sadece at-rest (DB, ses modelleri, lokal stream key'ler).
- RTMP relay kesinlikle Faz 2 — Faz 1'de yayıncı kendi Restream/Castr hesabını bağlar.
- WebSocket binary frame: 8 byte header (magic + codec + len) + Opus payload.
- DB primary key: UUID. Refresh token DB'de hash olarak saklanır, JWT stateless.
- `service_config` tablosu başlangıç değerleriyle hazır — admin toggle için.

## Sonraki Adim
Faz 1.2 Güvenlik Altyapısı veya 1.3+ devam. Backend için paralel iş:
- `infra/docker-compose.yml` (backend + frontend + postgres + redis + vault)
- `.env.example` (BIND_ADDR, JWT_SECRET, DATABASE_URL, sağlayıcı key'leri)
- Backend Dockerfile (multi-stage)
- WS integration testi (gerçek tokio websocket client ile auth→subtitle akışı)

## Blokerlar / Dikkat
- Migration canlı Postgres'e karşı çalıştırılmadı (yerel DB yok) — SQL schema.md'den birebir, sqlx derleme zamanı gömme başarılı.
- Gerçek STT/DeepL key'leri yok — entegrasyon (1.7/1.8) için Deepgram/DeepL hesabı gerekli.
- Vault local dev kurulumu (`vault server -dev`) hâlâ dökümente değil; JWT_SECRET şimdilik düz env var.
- `docs/commands/test-pipeline.md` adım 7'deki "AES şifreleme aktif mi" ifadesi TLS 1.3 kararıyla çelişiyor, düzeltilmeli.
