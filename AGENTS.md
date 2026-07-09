# StreamDubber — Agent Kuralları (Faz 1 MVP)

## BAŞLARKEN — Regression Testleri uygula hangi alanda calsimaya basliyacakisen



## BİTERKEN — Regression ve yeni Testleri Uygulamayi unutma



## Proje
Yayıncıya altyazı + çeviri + ses klonlama + multistream sunan SaaS.
Video buluta çıkmaz — sadece ses (~50-128 Kbps) buluta gider.
Faz 1 hedefi: Deepgram + DeepL + kendi SDK multistream relay'iyle (+ opsiyonel Streamlabs) ücretsiz altyazı.

## Stack
- Backend: Rust + Axum + Tokio (WebSocket ses pipeline)
- Frontend: Next.js 15 + Tailwind + shadcn/ui
- SDK: Tauri 2 + Rust (Windows önce, Mac/Linux adaptör iskeleti hazır)
- DB: PostgreSQL + Redis
- Şifreleme: TLS 1.3 (ses akışı için yeterli, üstüne AES ekleme)
- Key yönetimi: HashiCorp Vault
- STT: Deepgram Nova-2 → (Faz 2: faster-whisper)
- Çeviri: DeepL → (Faz 2: HY-MT1.5)
- Altyapı: Railway (MVP) → Hetzner GPU (Faz 2)

## Komutlar
```bash
cargo build          # backend derle
cargo test           # backend testleri
cargo clippy         # lint
npm run dev          # frontend geliştirme
npm run lint         # frontend lint
npm run test         # frontend testleri
tauri dev            # SDK geliştirme
tauri build          # SDK release build
docker compose up    # local dev ortamı
```

## Klasör Yapısı
```
/backend        → Rust/Axum (WebSocket, API gateway, auth)
/frontend       → Next.js (yayıncı + admin dashboard)
/sdk            → Tauri/Rust (yayıncı agent)
/sdk/src/core   → platform-bağımsız çekirdek (VAD, RTMP, şifreleme, WebSocket)
/sdk/src/audio  → platform adaptörleri (wasapi / coreaudio / pipewire)
/infra          → Dockerfile'lar, docker-compose, Vault config
/docs           → mimari kararlar ve referans dökümanlar
```

## Task Tamamlama Protokolü

Bir task bittiğinde şu sırayı takip et — atlamadan:

**1. Roadmap'te tiki at**
`streamdubber_roadmap_final.md` içinde ilgili satırı bul:
```
- [ ] Task adı   →   - [x] Task adı
```

**2. Değişiklikleri incele ve commit oluştur**
```bash
git status
git diff
```
- Yaptığın değişiklikleri kısaca özetle.
- Testler ve lint temizse, ilgili değişiklikleri anlamlı bir commit mesajıyla commit'le.
- Kullanıcı açıkça istemedikçe `git push` çalıştırma.

**Kural:** Her commit tek sorumluluk taşımalı ve repo kurallarındaki mesaj formatına uymalıdır.

---

## Genel Kurallar

### Kod Kalitesi
- Önce test yaz, sonra implement et (TDD — red → green → refactor)
- Kritik path'ler (ses pipeline, auth, şifreleme) için minimum %80 test coverage
- Her PR tek bir sorumluluğu değiştirir
- `unwrap()` kullanma — `?` veya explicit `match`/`if let`
- `console.log` / `println!` commit etme — tracing/logger kullan
- Commit mesajı: `feat:`, `fix:`, `chore:`, `test:` prefix zorunlu

### Güvenlik
- Ses akışı TLS 1.3 ile taşınır — üstüne AES ekleme (double encryption, gereksiz CPU)
- AES-256 sadece at-rest için: DB, ses modelleri, stream key'ler (lokal)
- API key, stream key, JWT secret → Vault veya env var, koda asla gömme
- `.env` commit'e girmez — `.env.example` girer
- SDK ↔ bulut mTLS doğrulaması zorunlu

### Rust Kuralları
- `async fn` handler'larda `?` ile error propagation
- `cfg(target_os)` sadece `/sdk/src/audio/` altında kullanılır
- Ses pipeline'ında lifetime hatası görürsen borrow checker'a uy, workaround yazma

### Docker
- Her servis kendi Dockerfile'ına sahip (multi-stage: builder → runtime)
- Tüm servisler env var ile konfigüre edilir, image içine değer gömülmez
- Health check her container'da zorunlu
- Local dev: `docker compose up` ile backend + frontend + postgres + redis + vault

### Konfigürasyon — Hiçbir Şey Hard-coded Olmaz
- STT, çeviri, TTS sağlayıcıları admin panelden toggle edilebilir (restart yok)
- Her ayar DB'de saklanır: `config::get("stt.provider")` pattern kullan
- Fiyat planları, limitler, özellikler de DB'den — koda gömme
- Fallback kuralları admin panelden: örn. "ElevenLabs meşgulse → Cartesia"

### SDK Mimarisi
- `sdk/src/core/` → platform-bağımsız (her şey burada)
- `sdk/src/audio/wasapi.rs` → Windows (aktif)
- `sdk/src/audio/coreaudio.rs` → macOS (iskelet, devre dışı — Faz 4)
- `sdk/src/audio/pipewire.rs` → Linux (iskelet, devre dışı — Faz 4)
- Video lokal kalır, buluta sadece ses gider

## Paralel Görev Stratejisi

Aşağıdaki görev grupları birbirinden bağımsız — aynı anda paralel yürütülebilir:

| Grup A | Grup B | Grup C |
|---|---|---|
| Backend (Rust/Axum) | Frontend (Next.js) | SDK (Tauri/Rust) |
| WebSocket, JWT, API gateway | Dashboard sayfaları, UI | Ses yakalama, VAD |
| Deepgram/DeepL entegrasyonu | Stripe entegrasyonu | OBS WebSocket |

**Bağımlılıklar (sıralı olmak zorunda):**
- Backend WebSocket API tamamlanmadan → SDK ses akışı test edilemez
- Auth (JWT) tamamlanmadan → Dashboard login çalışmaz
- Vault kurulumu → API key'ler koda gömülmeden önce

## Error Recovery

Komut başarısız olursa ne yapılır:

| Hata | Aksiyon |
|---|---|
| `cargo clippy` hata verir | Clippy hatalarını düzelt |
| `cargo test` başarısız | Testi geçir, workaround yazma |
| `npm run lint` başarısız | Lint hatalarını düzelt |
| Vault bağlantısı yok | Local dev için `.env` kullan, prod'a asla gömme |
| Deepgram timeout | AssemblyAI fallback'e geç, log'a yaz |

## Multi-Agent Handoff Protokolü

Bu repo ayni anda Codex ve Claude gibi birden fazla agent ile calisilabilir. Session dosyasi kullanilmaz; agentlar baglami git durumu, son commitler, ilgili dokumanlar ve kullanici mesajlarindan kurar.

Handoff gerekiyorsa calisma sonunda final cevapta kisa ve somut ozet ver: neredesin, son kararlar, sonraki adim ve blokerlar.

## Faz 1 Sınırları — Bu Fazda Yok
- Ses klonlama → Faz 2
- SDK gömülü RTMP relay → Faz 2
- faster-whisper / HY-MT1.5 / XTTS → Faz 2-3
- Twitch Extension / Chrome Extension → Faz 2
- Mac/Linux ses adaptörü aktivasyonu → Faz 4

Faz 1 dışı özellik sormadan implement etme.

## Referans Dökümanlar
Aşağıdaki dosyalar ilgili göreve başlamadan önce okunmalıdır:

| Konu | Dosya |
|---|---|
| Ses pipeline, STT/TTS/çeviri seçenekleri | `docs/audio-pipeline.md` |
| Güvenlik kararları, Vault yapısı | `docs/security.md` |
| Multistream modları (A/B/C) | `docs/multistream.md` |
| Dashboard sayfaları ve admin spec | `docs/dashboard.md` |
| Docker yapısı, env var listesi | `docs/docker-config.md` |
| WebSocket protokol (SDK ↔ Backend mesaj formatları) | `docs/websocket-protocol.md` |
| PostgreSQL şeması (tüm tablolar, migration stratejisi) | `docs/schema.md` |

## Tekrarlayan Görevler
Bu görevler için hazır iş akışları var — kullanmadan önce oku:

- GitHub issue düzeltme: `docs/commands/fix-issue.md`
- Deploy öncesi güvenlik kontrolü: `docs/commands/security-check.md`
- Pipeline uçtan uca test: `docs/commands/test-pipeline.md`
