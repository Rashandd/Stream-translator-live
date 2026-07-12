# StreamDubber — Agent Kuralları

> Bu dosya `CLAUDE.md` ile kanonik olarak senkron tutulur (Codex bu dosyayı, Claude
> `CLAUDE.md`'yi okur). İçerik birebir aynı olmalı — birinde değişiklik yaparsan
> diğerini de güncelle.

## BAŞLARKEN
Çalışmaya başlamadan önce hangi alandaysan onun regression testlerini çalıştır:
- Backend'e dokunacaksan: `cargo test` (+ değiştirdiğin crate'e özel `cargo test -p streamdubber-backend <modül>`)
- Frontend'e dokunacaksan: `cd frontend && npm run test && npm run lint`
- SDK'ya dokunacaksan: `cargo test -p streamdubber-sdk` (veya `cd sdk/src-tauri && cargo test`)
- `docs/SESSION.md`'yi oku — nerede kalındığını ve son kararları gösterir.

## BİTERKEN
- Değiştirdiğin alanın regression testlerini (yukarıdaki liste) + eklediğin yeni testleri çalıştır, hepsi yeşil olmadan bitirme.
- `docs/SESSION.md`'yi güncelle (Neredeyim / Son Kararlar / Sonraki Adım).
- Roadmap'te ilgili satırı tikle (bkz. "Task Tamamlama Protokolü").

## Proje
Yayıncıya altyazı + çeviri + ses klonlama + multistream sunan SaaS.
Video buluta çıkmaz — sadece ses (~50-128 Kbps) buluta gider.
Faz 1 hedefi: Deepgram + DeepL + kendi SDK multistream relay'iyle (+ opsiyonel Streamlabs) ücretsiz altyazı.

## Stack
- Backend: Rust + Axum + Tokio (WebSocket ses pipeline)
- Frontend: Next.js 15 + Tailwind + shadcn/ui
- SDK: Tauri 2 + Rust (Windows: WASAPI aktif; macOS/Linux: cpal aktif — üçü de indirilebilir, "yakında" değil)
- DB: PostgreSQL (opsiyonel — `DATABASE_URL` yoksa backend dosya-tabanlı `*_FILE` store'larla çalışır, DB best-effort ayna)
- Redis: compose'da tanımlı ama backend HENÜZ bağlanmıyor/kullanmıyor (hardening planı BH-04/06'ya bağlı — bkz. `docs/backend-hardening-captions-plan.md`)
- Şifreleme: TLS 1.3 (ses akışı için yeterli, üstüne AES ekleme yok); mTLS (istemci sertifikası) ERTELENDİ — bkz. `docs/security.md`
- Key yönetimi: admin panel (`SECRETS_FILE`, AES-256-GCM) — HashiCorp Vault opsiyonel taban store
- STT: Deepgram (prerecorded REST) + AssemblyAI fallback → self-hosted faster-whisper (roadmap 2.6, elle aktive edilir)
- Çeviri: DeepL + Google fallback → (Faz 3: HY-MT1.5)
- Altyapı: Frontend Vercel, Backend VPS (Docker) — bkz. `infra/deploy.md` (`infra/railway.md` ARŞİV, kullanılmıyor)

## Komutlar
```bash
cargo build                                                    # backend derle (workspace kökünden)
cargo test                                                     # backend + sdk-tauri testleri
cargo clippy                                                   # lint
cd frontend && npm run dev                                     # frontend geliştirme
cd frontend && npm run lint                                    # frontend lint
cd frontend && npm run test                                    # frontend testleri (vitest)
cd sdk && npm run tauri:dev                                    # SDK geliştirme
cd sdk && npm run tauri:build                                  # SDK release build
docker compose -f infra/docker-compose.yml --env-file .env up  # local dev ortamı
```

## Klasör Yapısı
```
/backend        → Rust/Axum (WebSocket, API gateway, auth)
/frontend       → Next.js (yayıncı + admin dashboard)
/sdk            → Tauri/Rust (yayıncı agent)
/sdk/src/core   → platform-bağımsız çekirdek (VAD, RTMP, mux, şifreleme, WebSocket)
/sdk/src/audio  → platform adaptörleri: wasapi.rs (Windows), cpal_capture.rs (macOS+Linux)
/stt-server     → self-hosted GPU STT worker (Python/FastAPI + faster-whisper, roadmap 2.6)
/extension      → Twitch component overlay extension (viewer bar, roadmap 2.4)
/scripts        → session/smoke/vault-seed yardımcı script'leri
/infra          → Dockerfile'lar, docker-compose, TLS/Vault config, deploy rehberleri
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
- Commit mesajı: `feat:`, `fix:`, `chore:`, `test:`, `docs:`, `merge:` prefix zorunlu

### Güvenlik
- Ses akışı TLS 1.3 ile taşınır — üstüne AES ekleme (double encryption, gereksiz CPU)
- AES-256 sadece at-rest için: dosya store'ları (secrets/tokens/relay/users/voice/…), DB `_encrypted` kolonları
- API key, stream key, JWT secret → admin panel (`SECRETS_FILE`) veya env var/Vault, koda asla gömme
- `.env` commit'e girmez — `.env.example` girer
- SDK ↔ bulut mTLS: sunucu tarafı hazır (`TLS_CLIENT_CA_PATH`), SDK istemci sertifikası sunmuyor — bugünkü
  karşılıklı doğrulama TLS 1.3 + cihaz JWT'sidir (pairing/claim, 30 gün geçerli). mTLS bağlanana kadar bunu
  "zorunlu" diye anma, `docs/security.md`'deki durum notuna bak.

### Rust Kuralları
- `async fn` handler'larda `?` ile error propagation
- `cfg(target_os)` / `cfg(unix)` / `cfg(windows)` öncelikle `/sdk/src/audio/` altında kullanılır; backend'de
  yalnız dosya-izni gibi gerekçeli, dar istisnalar kabul edilir (örn. `backend/src/stt_servers/provision.rs`
  SSH private-key chmod'u) — yeni bir istisna eklersen neden gerekli olduğunu yorum satırıyla belirt
- Ses pipeline'ında lifetime hatası görürsen borrow checker'a uy, workaround yazma

### Docker
- Her servis kendi Dockerfile'ına sahip (multi-stage: builder → runtime)
- Tüm servisler env var ile konfigüre edilir, image içine değer gömülmez
- Health check her container'da zorunlu
- Local dev: `docker compose -f infra/docker-compose.yml --env-file .env up` ile backend + frontend + postgres
  + vault (redis compose'da hazır ama backend henüz kullanmıyor — bkz. Stack notu)

### Konfigürasyon — Hiçbir Şey Hard-coded Olmaz
- STT, çeviri, TTS sağlayıcıları restart'sız değiştirilebilir — config API üzerinden (`PUT /admin/config`);
  ayrı bir "servis toggle" panel sayfası YOK (eski `/admin/routing` kaldırıldı, commit `00a7c90`)
- Her ayar `service_config`'te saklanır: `config::get("stt.provider")` pattern kullan
- Fiyat planları, limitler, özellikler de DB'den/`PlanRegistry`'den — koda gömme
- Fallback kuralları config API'den: örn. `stt.fallback=assemblyai`

### SDK Mimarisi
- `sdk/src/core/` → platform-bağımsız (her şey burada)
- `sdk/src/audio/wasapi.rs` → Windows (aktif, hand-rolled WASAPI)
- `sdk/src/audio/cpal_capture.rs` → macOS + Linux (aktif, cpal crate; Linux gerçek mikrofonla doğrulandı,
  macOS bu repoda hiç derleme kontrolünden geçemiyor — darwin SDK/toolchain yok)
- Video lokal kalır, buluta sadece ses gider (SDK cloud client yalnız binary ses frame'i gönderir —
  bkz. `sdk/src/core/cloud.rs`)

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

Bu repo ayni anda Codex ve Claude gibi birden fazla agent ile calisilabilir. Merkezi koordinasyon dosyasi
`docs/SESSION.md`'dir — agent-bazli session dosyasi (`docs/sessions/*.md`) KULLANILMAZ, o dosyalar artik
pasif/arsiv. Yeni oturuma baslarken `docs/SESSION.md`'yi oku, biterken orayi guncelle (bkz. BAŞLARKEN/BİTERKEN).

Handoff gerekiyorsa calisma sonunda final cevapta kisa ve somut ozet ver: neredesin, son kararlar, sonraki adim ve blokerlar.

## Faz Durumu — Şu An Kapsam Dışı Olanlar

Faz 1 çekirdeği (1.1–1.12) kod olarak tamam; gerçek launch kapısı roadmap'teki **"Faz 1 — MVP Öncesi Canlı
Doğrulama Listesi"**dir (gerçek key/OAuth/Stripe/sertifika testleri) — bu liste bitmeden production'a açılmaz.

Faz 2'nin büyük bölümü ZATEN KODLANDI (ses klonlama/ElevenLabs, SDK relay'e dub entegrasyonu, DeepL dil
matrisi, local STT/TTS protokolü, Twitch Extension backend+overlay, Founding-5 beta) — bunları "Faz 1 dışı"
diye reddetme, roadmap'i kontrol etmeden "Faz 1 dışı" sanma.

Gerçekten kapsam dışı / henüz yok:
- Kick OAuth login (PKCE gerektiriyor, kullanıcı kararıyla ayrı bir göreve ertelendi)
- Chrome Extension (roadmap 2.5 — hiç başlanmadı)
- Twitch Extension Hosted Test + review başvurusu (2.4 — kod hazır, insan/ops adımları kalan)
- SDK'ya gömülü local TTS motoru — XTTS-v2/Coqui native bind (2.2b KALAN)
- HY-MT1.5 çeviri, XTTS-v2 self-hosted TTS, bulut multistream Mod C/MediaMTX (Faz 3)
- YouTube multi-audio, mobil uygulama (Faz 4)
- macOS ses adaptörü *code-signing/notarization*'ı (yakalama kodu — cpal — zaten aktif ve indirilebilir;
  eksik olan yalnız imzalı/notarized dağıtım)

Yukarıdaki listede olmayan büyük bir özellik eklemeden önce roadmap'i kontrol et; burada "yok" denen bir şeyi
kodda bulursan önce bu listeyi güncelle, koddan geri alma.

## Referans Dökümanlar
Aşağıdaki dosyalar ilgili göreve başlamadan önce okunmalıdır:

| Konu | Dosya |
|---|---|
| Ses pipeline, STT/TTS/çeviri seçenekleri | `docs/audio-pipeline.md` |
| Güvenlik kararları | `docs/security.md` |
| Multistream modları (A/B/C) | `docs/multistream.md` |
| Dashboard sayfaları ve admin spec | `docs/dashboard.md` |
| Frontend runtime config kuralları (hard-code yasağı) | `docs/frontend-config.md` |
| Frontend görsel dil / DESIGN sistemi | `DESIGN.md` (repo kökü) |
| Docker yapısı, env var listesi | `docs/docker-config.md` |
| Cloud deploy (Vercel + VPS) rehberi | `infra/deploy.md` |
| WebSocket protokol (SDK ↔ Backend mesaj formatları) | `docs/websocket-protocol.md` |
| PostgreSQL şeması + migration stratejisi | `docs/schema.md` |
| Backend hardening + statik altyazı birleşik plan (Postgres/Redis/mTLS sertleştirme + GPU worker → YouTube CC / Twitch ext / Kick overlay) | `docs/backend-hardening-captions-plan.md` |
| Self-hosted STT (GPU worker) planı | `docs/selfhosted-stt-plan.md` |
| Yayıncı PC'sinde local STT (donanım/kota) planı | `docs/local-stt-plan.md` |
| SDK dağıtım (code signing, auto-update, release) | `docs/sdk-distribution.md` |
| Twitch Extension policy + uygulama planı | `docs/twitch-extension-policy.md`, `docs/twitch-extension-plan.md`, `docs/twitch-extension-production-readiness-plan.md` |
| Overlay builder (altyazı görünüm özelleştirme) | `docs/overlay-builder-plan.md` |
| Aktif multi-agent koordinasyon durumu | `docs/SESSION.md` |

## Tekrarlayan Görevler
Bu görevler için hazır iş akışları var — kullanmadan önce oku:

- GitHub issue düzeltme: `docs/commands/fix-issue.md`
- Deploy öncesi güvenlik kontrolü: `docs/commands/security-check.md`
- Pipeline uçtan uca test: `docs/commands/test-pipeline.md`
