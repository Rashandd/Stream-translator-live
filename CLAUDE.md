# StreamDubber — Claude Code Kuralları (Faz 1 MVP)

## Proje
Yayıncıya altyazı + çeviri + ses klonlama + multistream sunan SaaS.
Video buluta çıkmaz — sadece ses (~50-128 Kbps) buluta gider.
Faz 1 hedefi: Deepgram + DeepL + Restream/Castr entegrasyonuyla ücretsiz altyazı.

## Stack
- Backend: Rust + Axum + Tokio (WebSocket ses pipeline)
- Frontend: Next.js 15 + Tailwind + shadcn/ui
- SDK: Tauri 2 + Rust (Windows önce, Mac/Linux adaptör iskeleti hazır)
- DB: PostgreSQL + Redis
- Şifreleme: TLS 1.3 (transit) + AES-256 (at-rest: DB, ses modelleri, stream key'ler)
- Key yönetimi: HashiCorp Vault
- STT: Deepgram Nova-2 → (Faz 2: faster-whisper)
- Çeviri: DeepL → (Faz 2: HY-MT1.5)
- Altyapı: Railway (MVP) → Hetzner GPU (Faz 2)

## Komutlar
```bash
cargo build          # backend
cargo test           # backend testleri
cargo clippy         # lint (hata yoksa commit)
npm run dev          # frontend
npm run lint         # frontend lint
npm run test         # frontend testleri
tauri dev            # SDK geliştirme
tauri build          # SDK release build
```

## Klasör Yapısı
```
/backend       → Rust/Axum (WebSocket, API gateway, auth)
/frontend      → Next.js (yayıncı + admin dashboard)
/sdk           → Tauri/Rust (yayıncı agent)
/sdk/src/core  → platform-bağımsız çekirdek (ses, VAD, RTMP, şifreleme)
/sdk/src/audio → platform adaptörleri (wasapi/coreaudio/pipewire)
/infra         → Docker, Vault config, Railway deploy
```

## Kurallar

### Genel
- Önce test yaz, sonra implement et (TDD)
- Her PR tek sorumluluk taşır
- `unwrap()` yasak — `?` veya explicit error handling
- Koda API key / secret gömme — Vault'tan çek
- `console.log` commit etme — logger kullan

### Rust
- `async fn` handler'larda `?` ile error propagation
- Ses pipeline'ı: lifetime hataları için borrow checker'a dikkat
- Platform-bağımsız çekirdek: `cfg(target_os)` sadece `/sdk/src/audio/` altında

### Güvenlik (Faz 1'de kur, sonraya bırakma)
- Ses akışı TLS 1.3 ile taşınır — üstüne ayrıca AES şifreleme yapma (double encryption, gereksiz CPU)
- API key'ler, stream key'ler, JWT secret → Vault veya env var, koda asla gömme
- `.env` asla commit'e girmesin — `.env.example` girer
- SDK ↔ bulut mTLS zorunlu

### Docker
- Her servis kendi Dockerfile'ına sahip (`/backend`, `/frontend`, `/infra`)
- `docker-compose.yml` ile local dev (backend + frontend + postgres + redis + vault)
- Tüm servisler env var ile konfigüre edilir, image içine hiçbir değer gömülmez
- Multi-stage build: builder → runtime (küçük final image)
- Health check her container'da zorunlu

### Konfigürasyon — Hiçbir Şey Hard-coded Olmaz
- STT sağlayıcı (Deepgram/Whisper), çeviri (DeepL/HY-MT1.5), TTS (ElevenLabs/XTTS) admin panelden toggle edilebilir
- Her toggle DB'de saklanır, restart gerektirmez
- Yeni sağlayıcı eklemek için kod değil, sadece DB kaydı yeterli olmalı
- Fallback kuralları da admin panelden: "XTTS meşgulse → Cartesia"
- Feature flag pattern: `config::get("stt.provider")` → hardcoded string değil

### SDK Mimarisi
- `sdk/src/core/` → platform-bağımsız (VAD, RTMP relay, şifreleme, WebSocket)
- `sdk/src/audio/wasapi.rs` → Windows (aktif)
- `sdk/src/audio/coreaudio.rs` → macOS (iskelet, devre dışı)
- `sdk/src/audio/pipewire.rs` → Linux (iskelet, devre dışı)
- Video lokal kalır, buluta sadece ses gider

### Faz 1 Sınırları
- Ses klonlama YOK (Faz 2)
- SDK RTMP relay YOK (Faz 2) — yayıncı Restream/Castr kullanır
- XTTS/HY-MT1.5 YOK (Faz 3)
- Twitch Extension / Chrome Extension YOK (Faz 2)
- Faz 1 dışı özellik sormadan implement etme

## Context Sıfırlarken
Yeni session açmadan önce şunu çalıştır:
```bash
cat docs/SESSION.md   # mevcut plan ve kararlar
```

## Daha Fazla Bağlam Gerekirse
- Güvenlik detayları: `@docs/security.md`
- Ses pipeline detayları: `@docs/audio-pipeline.md`
- Multistream kararları: `@docs/multistream.md`
- Dashboard spec: `@docs/dashboard.md`
- Docker ve konfigürasyon: `@docs/docker-config.md`
