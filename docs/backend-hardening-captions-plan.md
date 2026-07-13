# StreamDubber Backend — Hardening + Statik Altyazı Birleşik Uygulama Planı

> **Durum:** Planlandı — implementasyon başlamadı
>
> **Bu belge neyin yerine geçiyor:** `docs/backend-production-hardening-agent-plan.md` ve
> `docs/static-captions-plan.md`, 2026-07-10'da çakışma analizinden sonra bu tek dosyada
> birleştirildi. O iki dosya artık yok — bu belge onların tüm içeriğini kapsar.
>
> **Birleştirme tarihi:** 2026-07-10
>
> **Revizyon:** 2026-07-12 — BH-09 ve BH-12'nin browser auth ile çakışan bölümleri,
> koddaki `auth::session` (commit `c76149d`) ve uygulanmakta olan
> `docs/auth-workflow-hardening-plan.md` esas alınarak daraltıldı (bkz. §0.1).
>
> **Kapsam:** Backend (Rust/Axum) + SDK (Tauri/Rust) + Frontend (Next.js) + stt-server (Python)
>
> **Repo kökü:** `/home/adem/Projects/GitHub/Stream-translator-live`

## 0. Bu birleştirmenin nedeni

İki plan bağımsız hazırlandı (biri backend'i production'a sertleştirmek için, diğeri
statik altyazı zincirini uçtan uca tamamlamak için) ve birbirini hiç referans almıyordu.
Karşılaştırmalı analizde üç somut çakışma bulundu; bu belge bu üçünü çözüyor, geri kalan
her iki planın içeriğini olduğu gibi koruyor.

| # | Çakışma | Çözüm |
|---|---|---|
| 1 | **STT autoactivate vs feature-gate-off:** Statik altyazı planı Bölge B, GPU worker provision'ı bitince `stt.provider=selfhosted`'ı **otomatik** (seed `true`) açmak istiyor. Hardening planı BH-13, `feature.self_hosted_stt.enabled` ve `feature.gpu_provisioning.enabled`'ı production'da **varsayılan kapalı** yapmak istiyor; kapalıyken provisioner SSH süreci bile başlatmamalı. | Bölge B1 artık BH-13'ün flag'lerinin İÇİNDE çalışır: flag kapalıyken provizyon denemesi reddedilir, autoactivate hiçbir etki yapmaz. Bkz. §8 Faz P2 ve Bölge B1 (güncellendi). |
| 2 | **Overlay/relay state mimarisi:** Statik altyazı planı D1, kalıcı overlay token'ı `RelayTargetStore::with_file` (AES-256-GCM şifreli **dosya**) deseniyle klonlamayı öneriyor. Hardening planı §6.1 hedef mimarisi bunun tam tersini istiyor: overlay/active-stream state Redis'e (BH-04), kalıcı domain verisi Postgres'e (BH-03) taşınmalı; "instance-local dosya store" ve "sınırsız HashMap/VecDeque" production'da yasak. | D1/D2 yeniden tasarlandı: token Postgres'te (BH-03 kapsamında yeni tablo), canlı kanal/abone semantiği BH-06'nın Redis-backed `register/subscribe/unregister` API'sinde. Dosya store klonu YOK. Bkz. §8 Faz P2 ve Bölge D1/D2 (güncellendi). |
| 3 | **Paylaşılan hot path'ler:** `ws/mod.rs`'in `stream_start`/handshake bölümüne captions planından Bölge D ve E, hardening planından BH-07 (listener ayrımı) ve BH-09 (JWT/session) dokunuyor. `pipeline/mod.rs`'teki `process_audio`/gateway çağrı zincirine captions planından Bölge A3 (utterance assembler), hardening planından BH-10 (provider timeout + WS 4005/4006 hata sözleşmesi) dokunuyor. Hiçbiri diğerinden haberdar değildi. | Açık sıralama kuralı kondu: `stream_start` için BH-07 → BH-09 → Bölge D → Bölge E; `process_audio` için BH-10 → Bölge A2/A3. Aynı dosyaya iki ajan paralel atanmaz. Bkz. §8. |

Bunların dışında **çakışma bulunmadı** — iki plan da usage/kota hesap algoritmasını ve
gerçek streaming STT'yi (Deepgram WS) açıkça kapsam dışı bırakıyor ve birbirine dokunmuyor;
Twitch Extension tarafında captions planı flag'i açmaya çalışmıyor (zaten `ext.twitch.enabled=false`),
BH-13 ile çelişmiyor.

### 0.1 Revizyon — 2026-07-12: browser auth çakışması giderildi

Birleştirmeyle aynı gün, bu plan yazılırken görülmeyen bir commit merge edildi:
`c76149d` (2026-07-10) — HttpOnly `sd_access`/`sd_refresh`/`sd_csrf` cookie'leri,
15 dk access + 30 gün refresh, her refresh'te rotation ve reuse'da family revoke
(`backend/src/auth/session.rs` + `backend/migrations/0005_browser_sessions.sql`).
Yani BH-09'un "kullanıcı token modeli" bölümünün sıfırdan tasarladığı sistem browser
tarafında **zaten var**. Browser kimlik doğrulamasının kalan işleri (OAuth'un cookie
session'a taşınması, opaque 2FA challenge, consent ledger, güvenilir logout) ayrı ve
**uygulanmakta olan** `docs/auth-workflow-hardening-plan.md`'nin sahipliğindedir.
Bu revizyonda:

- **BH-09** canlı entitlement + SDK token modeli + güvenlik olayı revoke
  entegrasyonuna daraltıldı; browser session mekaniğini yeniden kurmaz (bkz. §20).
- **BH-12**'nin consent alt bölümü auth-workflow planının A-06/Faz 4 consent
  ledger'ına devredildi (bkz. §23).
- **BH-04**'ün Redis'e taşıyacağı auth state listesi, auth-workflow planının
  ürettiği OAuth-login state ve 2FA challenge store'unu da kapsar (bkz. §15).
- **BH-02** ConfigSpec notuna auth-workflow planının ekleyeceği config key'leri
  eklendi (bkz. §13).

---

## 1. Belgenin amacı

Bu belge iki hedefi tek uygulama sırasında birleştirir:

1. **Backend'i** tek-instance geliştirme/MVP yapısından güvenli, gözlemlenebilir ve yatay
   ölçeklenebilir production mimarisine taşımak (eski adıyla "hardening" planı, BH-01..BH-15).
2. **Statik altyazı zincirini** (GPU worker → YouTube CC / Twitch Extension / Kick overlay)
   uçtan uca çalışır hale getirmek (eski adıyla "static captions" planı, Bölge A..G).

Yeni bir ajan bu belge dışında eski oturum bağlamına ihtiyaç duymadan çalışabilmelidir.

Bu çalışma yeni ürün özelliği geliştirme yetkisi vermez (Bölge A-G kendi içinde zaten
tanımlı özellik işidir, ama sınırları önceden ürün sahibiyle netleşmiştir — bkz. §6).
Roadmap aktif fazı farklı görünse bile repo kökündeki `CLAUDE.md`/`AGENTS.md` kuralları ve
kullanıcı tarafından verilen görev sınırı uygulanır.

## 2. Kesin kapsam sınırı

### 2.1 Bu planın içinde

- Önceki backend analizindeki tüm yüksek ve orta seviye bulgular (BH-01..BH-15).
- Statik altyazı uçtan uca zincirinin beş kopukluk noktası (Bölge A..G, bkz. §5.3).
- Bu bulguların gerektirdiği migration, test, dokümantasyon ve CI işleri.
- Geriye uyumlu ve kontrollü veri cutover araçları.
- Güvenlik değişiklikleri için frontend/SDK'da zorunlu olan minimal sözleşme güncellemeleri.

### 2.2 Bu planın dışında kalan P0 işler

Aşağıdaki dört kritik konu ayrı görev/PR olarak ele alınmalıdır:

1. Backend Docker image'ın workspace üyesi eksikliği nedeniyle derlenememesi.
2. Deepgram için gerçek streaming STT WebSocket pipeline'ı.
3. Usage süresinin frame başına yuvarlanması ve yanlış kota hesabı.
4. Stripe webhook'un gerçek kullanıcı planını yükseltmemesi/düşürmemesi.

Bu plan bu alanlarla temas edebilir, fakat P0 davranışı aynı commit içinde düzeltmeye
çalışmamalıdır. Örnekler:

- Canlı entitlement katmanı (BH-09) DB'deki plan değişikliğini anında görmelidir;
  Stripe'ın o planı değiştirmesi ayrı P0 işidir.
- Provider timeout ve hata kodları (BH-10) düzeltilebilir; prerecorded STT'nin
  streaming'e dönüştürülmesi ayrı P0 işidir.
- Usage repository hazırlanabilir (BH-03); süre hesaplama algoritması bu planın işi
  değildir. Bölge A3'ün utterance-flush'a geçişi de mevcut `tick_usage` davranışını
  (duvar saati bazlı) korumak zorundadır — frame-başına-yuvarlama hatasını **düzeltmez**,
  sadece bozmaz (test şartıyla doğrulanır).
- CI'a Docker build adımı ancak P0 Dockerfile düzeltildikten sonra zorunlu kapı yapılır.

### 2.3 Değiştirilemez güvenlik kuralları

- Ses akışına TLS 1.3 üstüne AES ekleme. Transport için TLS/mTLS kullanılacak.
- AES-256-GCM yalnız at-rest veriler için kullanılacak.
- Secret, JWT, API key, stream key veya voice sample loglanmayacak.
- Production'da eksik güvenlik ayarı için sessiz fallback yapılmayacak.
- Migration sırasında legacy dosyalar doğrulama tamamlanmadan silinmeyecek.
- Kullanıcı açıkça istemedikçe `git push` çalıştırılmayacak.
- İlgisiz çalışma ağacı değişiklikleri commit'e alınmayacak.

### 2.4 Statik altyazı — kapsam dışı kalanlar

- Gerçek zamanlı streaming STT (Deepgram WS / Voxtral — roadmap'te Faz 3 araştırma notu).
- SDK'da Opus encode (PCM s16le kalır).
- Dublaj/mux değişiklikleri (dub sink'lerin geç bağlanması dahil).
- mTLS client-cert connector'ın SDK tarafı — **BH-07** bu işi zaten kapsıyor, burada
  tekrar tanımlanmaz (bkz. §8 Faz P3).
- `usage_update` periyodik mesajı (bilinen eksik, ayrı küçük iş — `docs/websocket-protocol.md:199`).
- Twitch Hosted Test / review (insan adımları).
- Gerçek GPU kutusunda canlı smoke + TR kalite testi (insan/donanım gerektirir; bu plan
  kod tarafını hazır eder).

## 3. Projeyi sıfırdan tanıma

### 3.1 Backend mimarisi

Backend Rust, Axum ve Tokio kullanır. Ana parçalar:

| Alan | Dosya/dizin | Görev |
|---|---|---|
| Giriş ve dependency wiring | `backend/src/main.rs` | Env, DB, secret, provider ve listener kurulumu |
| Router | `backend/src/app.rs` | Public, kullanıcı ve admin endpoint'leri |
| Paylaşılan state | `backend/src/state.rs` | Store ve servislerin `AppState` içinde taşınması |
| Auth | `backend/src/auth/`, `backend/src/users/` | JWT, kullanıcı, OAuth login, 2FA/reset |
| SDK pairing/session | `backend/src/pairing/`, `backend/src/sdk_session/` | Cihaz eşleme ve heartbeat |
| Realtime | `backend/src/ws/`, `backend/src/pipeline/` | Ses/transcript WebSocket pipeline'ı |
| Provider gateway | `backend/src/gateway/` | STT, çeviri ve TTS sağlayıcıları |
| Overlay | `backend/src/overlay/` | Altyazı fan-out ve snapshot |
| Config/plan | `backend/src/config.rs`, `backend/src/plans/` | Runtime config ve entitlement katalogları |
| Usage | `backend/src/usage/` | Kota ve kullanım |
| Billing | `backend/src/billing/` | Stripe checkout/webhook |
| Secret yönetimi | `backend/src/secrets/` | Env/Vault/managed secret resolver |
| Kalıcılık | `backend/migrations/`, `backend/src/db/` | PostgreSQL schema ve pool |
| Log | `backend/src/logging/` | Bellek/JSONL log deposu |
| GPU worker yönetimi | `backend/src/stt_servers/` | Self-hosted STT sunucu provizyonu (Bölge B) |
| Entegrasyonlar | `backend/src/integrations/` | OAuth, YouTube caption client (Bölge E) |
| Relay | `backend/src/relay/` | RTMP relay hedef store'u (`RelayTargetStore`) |
| Twitch Extension EBS | `backend/src/ext_twitch/` | Extension backend servisi (Bölge F) |

SDK/Frontend'de bu plana konu olan ek parçalar:

| Alan | Dosya/dizin | Görev |
|---|---|---|
| VAD/capture çekirdeği | `sdk/src/core/vad.rs`, `sdk/src/core/gate.rs` | Ses yakalama, sessizlik tespiti (Bölge A/C) |
| Cloud session | `sdk/src/core/cloud_session.rs` | Backend'e ses akışı |
| Relay servisi | `sdk/src/core/relay_service.rs` | OBS→RTMP fan-out (Bölge C) |
| Shell orkestrasyon | `sdk/src/lib.rs` | `run_desktop_shell` (Bölge C) |
| GPU worker paneli | `frontend/src/components/admin-stt-servers-panel.tsx` | Bölge B3 |
| Overlay builder paneli | `frontend/src/components/caption-overlay-builder.tsx` | Bölge D3 |
| stt-server | `stt-server/main.py` | faster-whisper FastAPI servisi (Bölge A4) |

### 3.2 Başlamadan önce okunacak belgeler

Zorunlu:

1. `CLAUDE.md` / `AGENTS.md`
2. `streamdubber_roadmap_final.md`
3. `docs/security.md`
4. `docs/schema.md`
5. `docs/docker-config.md`
6. `docs/websocket-protocol.md`
7. `docs/audio-pipeline.md`
8. `infra/deploy.md`

İlgili göreve göre:

- Pipeline: `docs/commands/test-pipeline.md`
- Güvenlik: `docs/commands/security-check.md`
- GitHub issue: `docs/commands/fix-issue.md`
- Twitch Extension ops: `docs/twitch-extension-plan.md`
- Twitch Extension production çıkışı: `docs/twitch-extension-production-readiness-plan.md`
- Self-hosted STT tasarımı: `docs/local-stt-plan.md`, `docs/selfhosted-stt-plan.md`
- Dashboard/admin spec: `docs/dashboard.md`

## 4. Doğrulanmış başlangıç durumu

Plan hazırlanırken doğrulanan durum:

- Backend yaklaşık 24 bin satır Rust kodu içeriyor.
- Router yaklaşık 70 route içeriyor.
- 25 backend integration test dosyası var.
- `cargo test -p streamdubber-backend` başarılı.
- Dört gerçek PostgreSQL testi DB gerektirdiği için ignored.
- `cargo clippy -p streamdubber-backend --all-targets --all-features -- -D warnings`
  başarılı.
- Önceki audit'te `cargo fmt --check` 67 backend dosyasında fark buldu.
- CI test ve Clippy çalıştırıyor; fmt, dependency audit, coverage, gerçek DB/Redis
  ve backend Docker build kapıları eksik.
- Redis compose/dokümanda var, backend runtime kodunda fiilen kullanılmıyor.
- `cargo-audit`, `cargo-deny` ve `cargo-llvm-cov` local ortamda kurulu değildi;
  bu nedenle advisory ve gerçek coverage yüzdesi henüz bilinmiyor.
- `OverlayHub`/`ActiveStreams` (`backend/src/overlay/mod.rs`) bugün `Mutex<HashMap<...>>`
  + sınırsız `VecDeque` snapshot — process-local, TTL'siz (BH-04/BH-06/Bölge D'nin ortak
  başlangıç noktası; 2026-07-10'da koddan doğrulandı).
  `RelayTargetStore::with_file` (`backend/src/relay/mod.rs:98`) AES-256-GCM şifreli
  **dosya** store deseni — Bölge D1'in eski (artık geçersiz) tasarımının referans aldığı,
  BH-03/04'ün production'da elemeyi hedeflediği desen.
- `backend/src/config.rs`'te `ServiceConfig::get/set/set_existing` düz string
  key-value store — tipli/validasyonlu registry yok (BH-02'nin öncülü; Bölge A/B/E'nin
  yeni config key'leri bu registry'ye göre yazılmalı, bkz. §8).
- `stt_servers/mod.rs`'te `mark_ready`/`provision_server` var, `stt.selfhosted.autoactivate`
  diye bir config key henüz yok (Bölge B1'in net başlangıç noktası).
- `backend/src/auth/session.rs` + migration `0005_browser_sessions.sql` (commit
  `c76149d`, 2026-07-10): browser için HttpOnly cookie session, refresh rotation ve
  reuse'da family revoke ZATEN VAR — BH-09'un başlangıç varsayımı buna göre
  güncellendi (bkz. §0.1 ve §20). Browser auth'un kalan işleri
  `docs/auth-workflow-hardening-plan.md`'de.

Bu bilgiler başlangıç varsayımıdır. Yeni ajan ilk turda tekrar doğrulamalıdır.

## 5. Bilinen risk özeti

### 5.1 Yüksek seviye riskler (hardening)

1. Kalıcı state parçalıdır: PostgreSQL, memory ve şifreli/şifresiz dosya store'ları
   aynı anda kaynak gibi davranır.
2. Redis kullanılmadığı için rate-limit, OAuth state, komut kuyruğu ve active
   stream verisi instance-local'dır.
3. Mevcut mTLS doğrulaması aynı listener'daki bütün route'lara uygulanır; public
   API ile SDK trafiği birlikte güvenli biçimde sunulamaz.
4. Overlay ve OAuth state bilinmeyen/public girdilerle sınırsız büyüyebilir.
5. Client IP çözümlemesi istemcinin gönderdiği `X-Forwarded-For` değerine güvenir.
6. Mock provider, email 2FA ve TLS yapılandırmasında production fail-open yolları var.
7. JWT plan/rol claim'leri uzun süre stale kalabilir. Browser tarafında refresh
   rotation + family revoke artık var (`auth::session`, bkz. §0.1); SDK cihaz
   JWT'lerinde refresh/merkezi revoke hâlâ yok ve plan/rol hâlâ token claim'inden
   okunuyor.
8. WebSocket provider hataları protokolde tanımlı `4005/4006` kodlarıyla
   iletilmemekte veya sessizce yutulmaktadır.
9. Hesap silme, voice-data cleanup, key-use audit ve retention akışı eksiktir.

### 5.2 Orta seviye riskler (hardening)

1. Public `/config` bütün internal config snapshot'ını döndürür.
2. Config ve plan güncellemelerinde semantik validation yoktur.
3. `/health` dependency durumundan bağımsız `ok` döndürür.
4. Global request ID, timeout, concurrency, load-shed ve body limit politikası yoktur.
5. Faz 1/2/3 yüzeyleri aynı runtime'da yeterince güçlü feature gate olmadan bulunur.
6. Production kodunda lock/system-time varsayımlarına dayalı `expect()` çağrıları vardır.
7. JSONL log dosyaları sınırsız büyür ve request path'te senkron yazılabilir.
8. Format, dependency audit, coverage ve gerçek DB/Redis testleri CI tarafından
   zorlanmaz.

### 5.3 Statik altyazı — mevcut zincirin koptuğu yerler

Uçtan uca akış (pairing → SDK → mikrofon → backend pipeline → çıkışlar) kod üzerinden
izlendi. Zincir mimari olarak bağlı ve testli, ancak beş noktada kopuk:

1. **STT frame problemi (en kritik):** SDK her 20ms'lik voiced frame'i ayrı binary frame
   olarak gönderiyor (`sdk/src/core/gate.rs:67` `CapturePipeline::next_frame`,
   `sdk/src/core/cloud_session.rs:200`), backend her frame'de ayrı STT çağrısı yapıyor
   (`backend/src/ws/mod.rs:426` `send_for_audio` → `backend/src/pipeline/mod.rs:142`
   `process_audio` → `gateway.transcribe`). Konuşma sırasında saniyede ~50 STT + hedef
   dil başına ~50 çeviri isteği; 20ms'lik parçadan anlamlı transkript çıkmaz. Hiçbir
   yerde utterance birleştirme yok. GPU worker (`stt-server/main.py:75` `/transcribe`)
   batch faster-whisper — cümle bazlı istekle birebir uyumlu, 20ms parçayla kullanılamaz.
2. **Mikrofon OBS'e koşullu:** `run_desktop_shell` (`sdk/src/lib.rs:128-149`) OBS lokal
   relay'e publish edene kadar bloklar; mic capture + cloud session ancak ondan sonra
   başlar. Backend YouTube'u relay hedefi olarak **reddediyor**
   (`backend/src/relay/mod.rs:105` — YouTube OBS'ten doğrudan yayın alır). Yani YouTube
   yayıncısı relay kullanmaz → mikrofon hiç başlamaz → hiçbir altyazı üretilmez.
3. **GPU worker aktivasyonu manuel:** Provision akışı var
   (`backend/src/stt_servers/mod.rs:634`, `provision.rs:164`) ama başarı sonrası
   `stt.provider` config'i elle değiştirilmeli. Health check sadece provision anında bir
   kez (`mark_ready`, `stt_servers/mod.rs:316`); periyodik izleme yok. Panel
   (`admin-stt-servers-panel.tsx`) aktiflik/sağlık göstermiyor.
4. **YouTube caption push bağlı değil:** OAuth + refresh hazır
   (`backend/src/integrations/mod.rs:375` `get_fresh_access_token`, scope
   `youtube.force-ssl`), caption client hazır ve testli
   (`backend/src/integrations/youtube.rs` — `fetch_ingestion_url:72`, `push_caption:102`),
   ama: aktif broadcast id keşfi yok, `push_caption`'ın production çağıranı yok,
   `YouTubeCaptions` AppState'te kurulmuyor.
5. **Overlay token her yayında değişiyor:** `stream_start`'ta yeni uuid
   (`backend/src/ws/mod.rs:344`) → OBS Browser Source URL'i her yayında farklı;
   yayıncı her yayın öncesi OBS'i güncellemek zorunda. Liveness sinyali token
   varlığına gömülü (`ActiveStreams.get`, `backend/src/overlay/mod.rs:173`).

Twitch Extension backend'i + extension frontend'i kod olarak tamam
(`backend/src/ext_twitch/mod.rs`, `extension/js/`); kalan config/secret + insan-işi
Hosted Test.

### 5.4 Statik altyazı — spesifik riskler

1. **YouTube `lang` param:** Çok-track push davranışı canlıda doğrulanmadı — Bölge E4
   izolasyon şartına uy (tek fonksiyon, kolay tek-track fallback).
2. **Gap fallback ayarı:** `stt.utterance.gap_ms=400` konuşma temposuna göre ayar
   isteyebilir — config-driven olduğu için admin panelden düzeltilebilir.
3. **Kalıcı token + stale snapshot:** Bölge D2'deki snapshot-reset atlanırsa yeni
   yayının başında eski yayının altyazıları görünür — D1 ile D2 aynı PR'da gitmeli.
4. **Provision task'inde config set (Bölge B1):** `ServiceConfig` Arc'ının spawn'a
   taşınması gerekir; DB-persist eden config yolunda async dikkat.
5. **Bölge C1 sıralaması:** Cloud session erken başlayınca kota sayacı OBS'siz de
   işler — bu bilinçli (mikrofon açık = hizmet veriliyor); istenmezse `stream_start`'ı
   VAD ilk konuşmayı görünce göndermek ayrı iyileştirme (kapsam dışı).
6. **(Birleştirmede eklendi) Feature-gate sırası:** Bölge B1, BH-13'ten önce
   uygulanırsa `stt.provider=selfhosted` production'da hiçbir gate olmadan otomatik
   açılır — §8 Faz P2 sırası zorunludur.
7. **(Birleştirmede eklendi) Overlay mimarisi geç değişirse:** Bölge D, BH-04/BH-06'dan
   önce dosya-store deseniyle uygulanırsa, BH-04/06 geldiğinde o kod tamamen yeniden
   yazılır (iki kat iş). §8 Faz sırası zorunludur.

## 6. Kesinleşmiş tasarım kararları (statik altyazı, ürün sahibiyle 2026-07-10'da netleşti)

1. **GPU worker otomatik aktifleşir — ama artık feature gate'in içinde (birleştirmede
   eklenen şart):** Provision "ready" olur olmaz, `feature.gpu_provisioning.enabled` VE
   `feature.self_hosted_stt.enabled` açıksa `stt.provider=selfhosted` otomatik set
   edilir; `stt.fallback=deepgram` korunur. `stt.selfhosted.autoactivate` config'i
   (seed `true`) bu iki flag açıkken anlamlıdır; flag'ler kapalıyken hiçbir etkisi
   yoktur. Production'da flag'lerin ikisi de varsayılan **kapalı** (BH-13).
2. **Utterance birleştirme: SDK işaretler + backend tamponlar.** Latency gerekçesi:
   sadece-backend çözümü sessizliği "frame gelmemesinden" anlamak zorunda (+300-400ms
   yapay bekleme/cümle); SDK'daki VAD ise cümle sonunu kesin bilir (hangover kapanışı)
   → işaretle ek bekleme ~0ms. Binary header'ın reserved byte'ı flag olur — protokol
   geriye uyumlu. Backend'de işaretsiz akışlar için gap/süre fallback'i kalır. Proje
   canlıya çıkmadığı için sahada eski SDK kısıtı yok.
3. **YouTube caption dili: kaynak + TÜM hedef diller ayrı track.** Ingestion URL `lang`
   (+`name`) query parametreleriyle dil başına ayrı caption track kabul eder; izleyici
   dili YouTube player'ın kendi menüsünden seçer. `lang` param davranışı canlı yayında
   smoke testle doğrulanacak (Bölge E4).
4. **(Birleştirmede eklenen karar) Kalıcı overlay token'ın kaynağı Postgres'tir, dosya
   store değil.** Statik altyazı planının orijinal D1 tasarımı (`RelayTargetStore::with_file`
   klonu) terk edildi çünkü hardening planının hedef mimarisiyle (§7.1) doğrudan
   çelişiyordu. Bkz. Bölge D1 (güncellendi).

## 7. Hedef mimari (hardening)

### 7.1 Veri sahipliği

| Veri | Tek kaynak | Cache/dağıtım | Yasaklanan production davranışı |
|---|---|---|---|
| Users, role, plan | PostgreSQL | Kısa TTL Redis cache | File store'u source of truth yapmak |
| Plans ve service config | PostgreSQL | Redis invalidation/cache | Yalnız code default ile yaşamak |
| Subscriptions | PostgreSQL | Gerekiyorsa Redis | Webhook sonucu yalnız memory'de tutmak |
| SDK sessions/devices | PostgreSQL | Redis presence | Yalnız encrypted JSON kullanmak |
| OAuth/platform accounts | PostgreSQL, at-rest encrypted | Kısa TTL cache | Raw token dosyası |
| Relay/settings/bindings | PostgreSQL | Gerekiyorsa Redis | Instance-local snapshot |
| **Overlay token (kalıcı, birleştirmede eklendi)** | **PostgreSQL** | **Redis (canlılık/TTL)** | **Instance-local şifreli dosya store** |
| Usage ledger | PostgreSQL | Redis aggregation opsiyonel | Dosya JSONL'ı tek kaynak yapmak |
| OAuth CSRF state | Redis TTL | Yok | TTL'siz memory map |
| 2FA/reset/pairing code | Redis TTL | Yok | Multi-instance memory map |
| Rate-limit counters | Redis TTL | Yok | Process-local güvenlik limiti |
| Active stream | Redis TTL | Pub/Sub | Process-local map |
| Overlay snapshot | Redis bounded list | Pub/Sub | Sınırsız HashMap/VecDeque |
| Audit log | PostgreSQL/harici audit sink | Yok | Secret içeren JSONL |
| Operational log | Structured stdout/log backend | Bounded local ring | Sınırsız senkron dosya |

> Overlay token satırı birleştirmede eklendi — bu, Bölge D1'in yeni tasarımının
> doğrudan referans aldığı satırdır.

### 7.2 Ağ topolojisi

```text
Internet/browser/Stripe/OAuth
        |
        v
Public edge (Caddy/TLS)
        |
        v
Public Axum listener
  - dashboard/public API
  - Stripe webhook
  - OAuth callback
  - overlay
  - health

SDK
  |
  v
SDK Axum listener (TLS 1.3 + mandatory client certificate)
  - /ws/stream
  - SDK-specific device endpoints when required
```

Public listener client certificate istemez. SDK listener client certificate olmadan
TLS handshake'i tamamlamaz. Sertifika device/session kimliğiyle eşleştirilir.

---

## 8. Birleşik uygulama sırası (YENİ — bu bölüm çakışmaları çözer)

Bu bölüm, hardening planının kendi iç sırası (§9'un eski hali) ile captions planının
kendi iç sırası ("A → B → C → D → E → F") arasındaki üç kesişim noktasını çözer.
**Aşağıdaki fazlar dışında** her iki planın kendi iç bağımlılık grafiği (BH için aşağıdaki
9.1, Bölge için 9.2) geçerliliğini korur — burada sadece iki plan arasındaki gerçek
kesişimler yeniden sıralanıyor.

### Faz P0 — Bağımsız temel işler (paralel yapılabilir)

- **BH-01** Public config allowlist
- **BH-02** Typed config/plan validation → yeni `ConfigSpec` registry. **Bu noktadan
  itibaren Bölge A/B/E'nin ekleyeceği tüm yeni config key'leri (`stt.utterance.max_ms`,
  `stt.selfhosted.autoactivate`, `integrations.youtube.captions.enabled` vb.) doğrudan bu
  registry formatında (tip + validator + visibility) yazılır — sonradan retrofit
  edilmez.**
- **Bölge C1, C2** SDK mikrofon/relay, OBS'ten bağımsız (tamamen SDK-side, backend'e
  dokunmuyor)
- **Bölge A1** Protokol flag byte'ı (salt wire format)
- **Bölge A4** stt-server docstring notu

### Faz P1 — Paylaşılan state mimarisi (hardening; Bölge B/D'nin önkoşulu)

- **BH-03** PostgreSQL tek kaynak (en az 03A/03B — repository temeli + şema; 03B'nin
  migration listesine **`overlay_tokens` tablosu** eklenir, bkz. Bölge D1)
- **BH-04** Redis paylaşılan ephemeral state (overlay snapshot + active-stream Redis'e
  taşınır)
- **BH-05** Trusted proxy / client identity (bağımsız, bu fazda paralel yapılabilir)
- **BH-06** Bounded overlay/OAuth/WS kaynakları — `OverlayHub`'a
  `register(token, owner, expires_at)/subscribe/unregister` eklenir
- **BH-13** Feature gate'ler — `feature.gpu_provisioning.enabled`,
  `feature.self_hosted_stt.enabled` vb. tanımlanır (BH-02'den sonra, bağımsız)

### Faz P2 — Statik altyazının P1'e bağımlı bölümleri (ÇÖZÜLEN ÇAKIŞMALAR)

- **Bölge B** (GPU worker autoactivate + health) — B1 artık BH-13'ün flag'lerinin
  içinde çalışır: `feature.gpu_provisioning.enabled=false` iken `provision_server` SSH
  süreci başlatmadan reddeder; `feature.self_hosted_stt.enabled=false` iken
  `stt.selfhosted.autoactivate=true` olsa bile `stt.provider` değişmez. Prod varsayılanı
  ikisi de kapalı — bu görevin "otomatik aktivasyon" davranışı yalnız admin bu flag'leri
  açtıktan sonra devreye girer.
- **Bölge D** (kalıcı overlay token) — D1 artık BH-03'ün Postgres tablosu + BH-04/06'nın
  Redis-backed hub'ı üzerine kurulur. Dosya store klonu YOK.

### Faz P3 — SDK mTLS + oturum güvenliği (`ws/mod.rs stream_start`'a dokunan büyük değişiklikler)

- **BH-07** Ayrı SDK mTLS listener
- **BH-08** Fail-closed startup
- **BH-09** Canlı entitlement + SDK token revoke (browser refresh/rotation zaten
  `auth::session`'da — bkz. §0.1/§20)

> **Sıra kuralı:** `ws/mod.rs`'in `stream_start`/handshake bölümüne bu üçü **önce**
> dokunur (yapısal/güvenlik değişikliği), Bölge D ve E **sonra** üstüne biner. Aynı
> dosyaya iki ajan paralel atanmaz; her adım bir öncekinin üzerine rebase edilir.

### Faz P4 — `stream_start` üzerine kurulan özellik işi

- **Bölge D** (eğer P2'de henüz bitmediyse burada tamamlanır — P2'deki bağımlılık
  BH-03/04/06'yadır, BH-07/09'a değil; pratikte P2 ile P3 paralel ilerleyebilir, ama D'nin
  `stream_start` değişikliği P3 bitmeden merge edilmez)
- **Bölge E** (YouTube caption pusher) — captions planının kendi kuralı korunur: **D
  bitmeden E'ye başlanmaz** (ikisi de `stream_start`'a dokunuyor)
- **Bölge F** (Twitch extension smoke) — BH-13'ün gerçek gate mekanizmasına karşı
  test edilir (kendi bespoke flag kontrolü yazılmaz)

### Faz P5 — Provider dayanıklılığı ve utterance birleştirme (aynı hot path, sıralı)

- **BH-10** Provider timeout + WS 4005/4006 hata sözleşmesi — **ÖNCE**
- **Bölge A2/A3** VAD utterance-end + backend UtteranceAssembler — **SONRA**, BH-10'un
  tanımladığı hata kodlarını "frame başına" değil "flush başına" korur

### Faz P6 — Gözlemlenebilirlik/ops

- **BH-11** Liveness/readiness
- **BH-12** Hesap silme/audit/retention
- **BH-14** Async I/O, log, panic temizliği

### Faz P7 — Doğrulama ve kalite kapıları (son)

- **Bölge G** Uçtan uca doğrulama, smoke script, dok güncellemeleri
- **BH-15** CI kapıları (fmt, DB/Redis integration, audit, coverage)

### 8.1 Neden bu sıra — kısa gerekçe

- P0/P1 önce gelir çünkü P2'deki her iki Bölge de (B ve D) P1'in ürettiği ilkellere
  (feature flag, Postgres tablo, Redis-backed hub) bağımlıdır; tersi sırada
  uygulanırlarsa baştan yazılırlar (§5.4 madde 6-7).
- P3, P4'ten önce çünkü P4'teki D ve E, `stream_start`'ın **son güvenlik/yönlendirme
  halini** üzerine bina eder; P3 sonradan gelirse D/E'nin kodu BH-07/09 ile çakışır ve
  rebase gerekir.
- P5'te BH-10 önce çünkü A3, gateway çağrı zincirini restructure ederken BH-10'un
  tanımladığı hata kontratını (4005/4006, timeout) bozmadan üstüne inşa etmeli; A3 önce
  giderse BH-10 aynı kod yolunu tekrar refactor etmek zorunda kalır.
- Geri kalan BH maddeleri (03'ün geri kalanı, 05, 08, 09, 11, 12, 14, 15) captions
  planıyla dosya çakışması taşımadığı için hardening planının kendi iç sırasında
  (§9.1) kalır — burada yeniden sıralanmadı.

### 9.1 BH'nin kendi iç bağımlılık grafiği (değişmedi)

```text
Hazırlık/ADR
    |
    +--> BH-01 --> BH-02 ------------------------------+
    |                                                   |
    +--> BH-03 --> BH-04 --> BH-05 --> BH-06           |
             |        |          |          |           |
             |        |          |          +--> BH-10-+
             |        |          |                      |
             |        +----------+--> BH-08 <-- BH-07  |
             |                              |           |
             +--> BH-09 --------------------+           |
                     |                                  |
                     +--> BH-12                         |
                                                        |
                                  BH-11 <---------------+

BH-13 bağımsız, fakat BH-02 sonrasında yapılmalı.
BH-14 büyük refactorlar bittikten sonra yapılmalı.
BH-15 son kalite kapısıdır.
```

BH-03, BH-04, BH-07 ve BH-09 tek commit/tek PR yapılmayacak kadar büyüktür.
Alt adımlara bölünmelidir.

### 9.2 Bölge'nin kendi iç sırası (değişmedi, §8 ile birlikte okunur)

Bölge sırası önerisi: **A → B → C → D → E → F** (A ve B paralel yapılabilir; C SDK
tarafı bağımsız; E, D'den bağımsız ama ikisi de `ws/mod.rs stream_start`'a dokunur —
çakışmayı önlemek için D'yi önce bitir). **§8'deki Faz P1-P4 bu sıranın üstüne binen,
hardening planıyla kesişen ek bağımlılıklardır; birbirini geçersiz kılmaz, daraltır.**

---

## 10. Genel uygulama yöntemi

Her iş paketinde şu sıra zorunludur:

1. Kapsamla ilgili referans belge ve kodu oku (§3.2 + ilgili Bölge'nin "Bağlı dokümanlar"ı).
2. `git status` ve `git diff` ile başkalarının değişikliklerini belirle.
3. İlgili regresyon testlerini çalıştır.
4. Başarısız test ekle (TDD — red).
5. Minimal implementasyon yap (green).
6. Testi geçir.
7. Refactor et.
8. Migration/cutover etkisini doğrula (varsa).
9. İlgili dokümanı güncelle (`docs/websocket-protocol.md`, `docs/audio-pipeline.md`,
   `docs/dashboard.md` vb. — ilgili göreve göre).
10. Backend test, Clippy ve fmt çalıştır.
11. Risk yüksekse workspace testlerini çalıştır.
12. Bu belgedeki İlerleme listesinde (§11) yalnız tamamlanan maddeyi tikle + varsa
    `> Durum (tarih): ...` blockquote'u ekle; roadmap'te ilgili satır varsa tikle.
13. Diff'i incele ve tek sorumluluklu commit oluştur (`feat:`/`fix:`/`test:`/`chore:`).
14. Push yapma.

`unwrap()` yasak — `?` veya explicit `match`/`if let`. Config değerleri
`ServiceConfig`/`ConfigSpec` deseninden okunur, hard-code yok. Kritik path %80+ coverage.

## 11. Birleşik İlerleme / Roadmap Checklist

`streamdubber_roadmap_final.md` içine "Backend Hardening + Statik Altyazı" başlığı
altında, §8'deki faz sırasına göre gruplanmış şu checklist eklenmelidir:

```markdown
### Backend Hardening + Statik Altyazı (bkz. docs/backend-hardening-captions-plan.md)

Faz P0
- [ ] BH-01 Public config yüzeyini güvenli allowlist ile sınırla
- [ ] BH-02 Config ve plan güncellemelerine semantik validation ekle
- [ ] A1. Protokol: reserved byte → flags
- [ ] A4. stt-server dokümantasyon notu
- [ ] C1. Mikrofon/cloud session'ı OBS'ten bağımsız başlat
- [ ] C2. OBS yeniden bağlanma (relay accept döngüsü)

Faz P1
- [ ] BH-03 PostgreSQL'i production state için tek kaynak yap (+ overlay_tokens tablosu)
- [ ] BH-04 Redis tabanlı paylaşılan ephemeral state kur
- [ ] BH-05 Trusted proxy ve güvenilir client IP çözümlemesi ekle
- [ ] BH-06 Rate-limit, OAuth state, overlay ve WS kaynaklarını sınırla
- [ ] BH-13 Faz dışı backend özelliklerini feature gate arkasına al

Faz P2
- [ ] B1. Provision başarısında otomatik STT aktivasyonu (feature-gate'li)
- [ ] B2. Periyodik GPU worker health-check döngüsü
- [ ] B3. Admin panel: aktif sağlayıcı + sağlık görünürlüğü
- [ ] D1. Kullanıcı başına kalıcı overlay token (Postgres + Redis-backed hub)
- [ ] D2. Liveness'i token varlığından ayır + snapshot reset

Faz P3
- [ ] BH-07 Public API ile SDK mTLS listener'larını ayır
- [ ] BH-08 Production startup kontrollerini fail-closed yap
- [ ] BH-09 Canlı entitlement + SDK token revoke ekle (browser refresh/rotation mevcut — bkz. §20)

Faz P4
- [ ] D3. Panel: sabit overlay URL + token yenileme
- [ ] E1. YouTube aktif broadcast keşfi
- [ ] E2. CaptionPusher (OverlayHub → YouTube CC, dil başına track)
- [ ] E3. AppState wiring (YouTubeCaptions + pusher)
- [ ] E4. Canlı doğrulama maddesi (lang param)
- [ ] F1. Twitch extension e2e smoke + prod checklist maddesi

Faz P5
- [ ] BH-10 Provider timeout ve WebSocket hata protokolünü düzelt
- [ ] A2. SDK: VAD cümle sonu işareti
- [ ] A3. Backend: UtteranceAssembler

Faz P6
- [ ] BH-11 Liveness, readiness ve operasyonel sağlık kontrolleri ekle
- [ ] BH-12 Hesap silme, voice-data cleanup ve audit altyapısını tamamla
- [ ] BH-14 Senkron dosya I/O, log rotation ve production panic noktalarını temizle

Faz P7
- [ ] G1. Uçtan uca doğrulama + doküman/roadmap güncellemeleri
- [ ] BH-15 CI'ya fmt, DB/Redis integration, audit ve coverage kapıları ekle
```

Tarihsel roadmap'te mTLS gibi tamamlanmış görünen maddeler doğrudan silinmemelidir.
BH maddeleri mevcut implementasyonun production sertleştirme/doğrulama katmanıdır.

---

## 12. BH-01 — Public config sınırı

> **Durum (2026-07-13): TAMAMLANDI.** `config::PUBLIC_CONFIG_KEYS` allowlist'i
> (default-deny) + `ServiceConfig::public_snapshot()` eklendi; `GET /config` artık
> yalnız promo/legal/company anahtarlarını döner. Yetkili `GET /admin/config`
> (`AdminUser`) full snapshot verir. Frontend: `admin-live-status` ve
> `admin-promo-panel` admin uca taşındı (`stt.provider`/`translate.provider`/
> `providers.mock_fallback`/`founding.gate.enabled` internal); `admin-legal-panel`
> yalnız public key okuduğundan `/config`'te kaldı. Testler: `config.rs` unit
> (allowlist include/exclude, "yeni internal key public olmaz", "allowlist seeded")
> + `config_api.rs` (public allowlist, admin full read, 401/403). fmt/clippy temiz.

### Amaç

Public `/config` endpoint'inin yalnız gerçekten public olması gereken değerleri
döndürmesini sağlamak.

### İlgili dosyalar

- `backend/src/app.rs`
- `backend/src/config.rs`
- `backend/tests/config_api.rs`
- Public config tüketen frontend dosyaları

### Uygulama adımları

1. Frontend'in `/config` üzerinden kullandığı alanları `rg` ile çıkar.
2. `PublicConfigResponse` adında explicit DTO oluştur.
3. DTO alanlarını gruplandır:
   - public product/promo
   - public company/legal
   - desteklenen dil/locale
   - kullanıcıya gösterilecek feature availability
4. Internal anahtarları public yanıttan çıkar:
   - `providers.mock_fallback`
   - `auth.*`
   - `cors.*`
   - internal provider URL'leri
   - rate-limit ayarları
   - Stripe internal mapping
   - provisioning/SSH ayarları
5. Admin için yetkili full config read endpoint'i oluştur veya mevcut admin
   endpoint'ini genişlet.
6. Secret store değerleri full admin config'e dahi eklenmemeli.
7. API değişiyorsa frontend tüketimini aynı commit serisinde açık sözleşmeyle taşı.

> **Not (birleştirmede eklendi):** Bölge B3, admin panelinde aktif STT sağlayıcı
> rozetini `GET /config`'ten `stt.provider` okuyarak göstermeyi öneriyordu. `stt.provider`
> internal/altyapısal bir detay olduğu için bu adımda **public** allowlist'e girmemeli —
> B3'ün implementasyonu bu görevin ürettiği **admin** full-config endpoint'ini kullanacak
> şekilde güncellendi (bkz. Bölge B3).

### TDD testleri

- Public response gerekli public alanları içerir.
- `providers.mock_fallback` public response'da yoktur.
- `auth.email_2fa` public response'da yoktur.
- Provider test/internal URL public response'da yoktur.
- Admin full config okuyabilir (`stt.provider` dahil).
- Broadcaster admin config okuyamaz.
- Config'e yeni internal key eklemek onu otomatik public yapmaz.

### Kabul kriteri

Public alan eklemek için DTO'ya explicit kod değişikliği gerekmelidir. Internal map
snapshot'ı hiçbir public route'tan dönmemelidir.

### Önerilen commit

```text
fix: restrict public service config response
```

## 13. BH-02 — Typed config ve plan validation

### Amaç

Runtime config ve plan mutation'larının anlamsız/güvensiz değer kabul etmesini
engellemek.

### İlgili dosyalar

- `backend/src/config.rs`
- `backend/src/plans/mod.rs`
- `backend/src/app.rs`
- `backend/tests/config_api.rs`
- `backend/tests/plans_api.rs`

### Tasarım

Merkezi config spec kaydı:

```rust
struct ConfigSpec {
    key: &'static str,
    visibility: Visibility,
    value_type: ConfigValueType,
    validator: fn(&str, Environment) -> Result<(), ValidationError>,
}
```

Desteklenen tipler:

- bool
- pozitif/sınırlı integer
- enum
- URL/origin listesi
- plan listesi
- dil kodu
- kontrollü HTML/metin
- secret reference; secret değeri değil

### Uygulama adımları

1. Mevcut default key listesini spec registry'ye taşı.
2. Provider değerlerini registry'de kayıtlı enumlarla sınırla.
3. Timeout/rate-limit değerlerine alt/üst sınır ekle.
4. Production URL'lerinde HTTPS/WSS zorunlu kıl.
5. CORS wildcard ve bozuk origin değerlerini reddet.
6. Plan CSV alanlarındaki her planın registry/DB'de varlığını doğrula.
7. Production'da mock fallback ve zayıf 2FA değerlerini reddet.
8. Plan validation ekle:
   - normalize edilmiş, boş olmayan slug
   - `price_cents >= 0`
   - limitler yalnız `-1` veya pozitif
   - kullanımda olan planın silinmesini engelle
9. Mutation sırası DB-first olmalı; DB hatasında memory snapshot değişmemeli.
10. Validation hatalarını `422` ve alan bazlı güvenli hata gövdesiyle dön.

> **Not (birleştirmede eklendi):** Bu registry tamamlandıktan sonra eklenecek tüm yeni
> config key'leri (Bölge A'nın `stt.utterance.max_ms`/`stt.utterance.gap_ms`, Bölge B'nin
> `stt.selfhosted.autoactivate`/`stt.selfhosted.health_interval_secs`, Bölge E'nin
> `integrations.youtube.captions.enabled`/`integrations.youtube.captions.retry_secs`)
> doğrudan bu spec formatında (tip + validator + visibility=internal/admin) tanımlanır —
> önce ham string key olarak eklenip sonra registry'ye taşınmaz. Aynı kural
> `docs/auth-workflow-hardening-plan.md`'nin ekleyeceği key'ler için de geçerlidir
> (legal doküman versiyon anahtarları, geçici `auth.oauth_legacy_fragment` flag'i vb.).

### TDD testleri

- Bilinmeyen provider reddedilir.
- Negatif/sıfır timeout ve limit reddedilir.
- Bozuk URL/origin reddedilir.
- Production'da HTTP provider URL reddedilir.
- Production'da mock fallback açılamaz.
- Negatif plan fiyatı ve sıfır plan limiti reddedilir.
- Kullanılan plan silinemez.
- DB write başarısızsa memory değişmez.
- Geçerli runtime değişiklik restart olmadan görünür.

### Önerilen commitler

```text
feat: add typed service config validation
fix: validate plan mutations before persistence
```

## 14. BH-03 — PostgreSQL tek kaynak

### Amaç

Production'da kalıcı domain verisinin tek kaynağını PostgreSQL yapmak ve süresiz
memory/file/DB ayrışmasını kaldırmak.

### İlk iş: ADR

`docs/adr/00xx-production-state-ownership.md` oluştur. Her store için:

- veri sahibi
- durability
- TTL
- cache invalidation
- encryption
- migration/cutover
- failure policy

kararlarını yaz.

### BH-03A — DB zorunluluğu ve repository temeli

1. Production'da `DATABASE_URL` zorunlu olsun.
2. `AppState.db: Option<PgPool>` kullanımını production servislerinde kaldır.
3. Test `AppState::skeleton` in-memory adapter kullanabilir.
4. Domain-spesifik repository oluştur; her şey için tek generic repository yazma.
5. Tüm write metotları async, hata döndüren ve transaction-aware olsun.
6. DB hatalarını logla, fakat SQL/DSN/PII kullanıcıya dönme.

### BH-03B — Schema/migration

Yeni forward-only migration'larla:

- Plan katalog seed kayıtları
- Eksik user/security alanları
- Relay targets
- User stream/STT/TTS settings
- Founding applications
- Voice model metadata/sample reference
- STT server metadata gerekiyorsa ilgili tablo
- **`overlay_tokens(user_id PK/FK, token UNIQUE, created_at)` (birleştirmede eklendi —
  Bölge D1'in kalıcı overlay token'ı için; bkz. §7.1 veri sahipliği tablosu)**
- Unique, FK, cascade ve lookup index'leri

eklenmelidir.

Plan seed migration'ı `ON CONFLICT` davranışını açıkça tanımlamalıdır. Production
admin değişikliklerini her startup'ta ezmemelidir.

### BH-03C — Domain cutover sırası

1. Users
2. Plans
3. Service config
4. SDK sessions
5. Relay targets
6. Platform/OAuth accounts
7. Channel bindings
8. Stream/STT/TTS settings
9. Founding applications
10. Voice metadata ve sample referansları
11. **Overlay tokens (birleştirmede eklendi — Bölge D1 bu adımdan sonra başlar)**

Usage calculation algoritmasına dokunma. Stripe plan transition'a dokunma.

### BH-03D — Legacy importer

`backend/src/bin/import_legacy_state.rs` benzeri idempotent bir araç ekle.

Zorunlu davranışlar:

- `--dry-run`
- Kayıt/hata sayısı özeti
- Tekrar çalışmada duplicate üretmeme
- Geçersiz kayıtları sessiz atlamama
- Secret/PII loglamama
- Kaynak dosyayı otomatik silmeme
- Import sonrası verification sorguları

Cutover:

1. Legacy dosya backup
2. Dry-run
3. Import
4. Kayıt sayısı ve checksum/örnek doğrulama
5. DB mode startup
6. Smoke test
7. Legacy dosyayı salt okunur arşiv
8. Gözlem penceresi sonrası kaldırma kararı

### TDD/integration testleri

- User create → process restart → DB load → login.
- İki instance eşzamanlı duplicate email oluşturamaz.
- İki instance plan/config değişikliğini tutarlı görür.
- DB write hatasında HTTP başarı dönmez.
- Transaction ortası hata kısmi kayıt bırakmaz.
- Legacy importer iki kez çalışınca duplicate üretmez.
- FK ve delete cascade beklenen davranışı verir.
- DB migration boş ve mevcut verili DB'de çalışır.

### Kabul kriteri

İki backend instance aynı PostgreSQL'e bağlıyken kalıcı domain verisi için aynı
sonucu vermelidir. Production restart dosya state'e ihtiyaç duymamalıdır.

### Önerilen commit sınırları

```text
feat: seed plan catalog in postgres
feat: load and persist users through postgres
feat: persist phase one domain state in postgres
chore: add idempotent legacy state importer
chore: disable file state stores in production
```

## 15. BH-04 — Redis paylaşılan ephemeral state

### Amaç

TTL, tek-kullanım ve multi-instance koordinasyon isteyen state'i Redis'e taşımak.

### Redis'e taşınacaklar

- Rate-limit sayaçları
- OAuth state
- Pairing code
- 2FA ve reset code
- SDK command queue
- Active stream presence
- Overlay snapshot ve Pub/Sub
- Kısa TTL entitlement/revocation cache

### Uygulama adımları

1. Tokio uyumlu Redis connection manager ekle.
2. `REDIS_URL` production'da zorunlu olsun.
3. Environment namespaced key formatı tanımla:

   ```text
   streamdubber:{env}:oauth:{state}
   streamdubber:{env}:rate:{endpoint}:{identity}
   streamdubber:{env}:pairing:{code}
   streamdubber:{env}:active-stream:{user_id}
   streamdubber:{env}:overlay:{token}
   ```

4. Her ephemeral key TTL taşısın.
5. OAuth/pairing consume atomik `GETDEL` veya Lua ile yapılsın.
6. Rate-limit atomik increment + expire olsun.
7. Overlay:
   - canlı mesaj için Pub/Sub
   - bounded snapshot için capped list
   - stream kapanışında TTL/cleanup
8. Security-sensitive endpoint Redis arızasında production'da local limiter'a
   sessiz düşmesin; fail-closed veya açık degradation policy kullan.
9. Development/test için açıkça seçilen memory adapter kalabilir.

> **Not (birleştirmede eklendi):** Bölge D1/D2 (kalıcı overlay token) bu görevin
> Redis-backed overlay/active-stream tasarımını doğrudan kullanacak şekilde yeniden
> yazıldı — token Postgres'te (BH-03), canlılık/kanal/snapshot burada. Bu görevi
> uygulayan ajan, `register/subscribe/unregister` API yüzeyini BH-06 ile birlikte
> tasarlarken Bölge D1/D2'nin ihtiyaçlarını (kalıcı token ile tekrar tekrar register
> edilebilme, snapshot reset) dikkate almalı.

> **Not (2026-07-12 revizyonu, bkz. §0.1):** `docs/auth-workflow-hardening-plan.md`
> A-03/A-04'ün ürettiği OAuth-login state (intent/locale/return_to) ve opaque 2FA
> challenge store'u da "Redis'e taşınacaklar" listesine dahildir. O plan store'ları
> taşınabilir bir arayüz arkasında bırakır (kendi §8'i); Redis implementasyonu bu
> görevin işidir — taşırken challenge attempt sayacı ve tek-kullanım semantiği
> atomik kalmalıdır.

### Testler

- İki instance aynı rate-limit sayacını paylaşır.
- OAuth state yalnız bir kez tüketilir.
- Pairing code yalnız bir instance tarafından claim edilir.
- TTL sonunda key temizlenir.
- Redis kesilince login/pairing güvenli davranır.
- Bir instance'ın overlay publish'i diğerinden subscribe edilebilir.
- Redis key taramasında TTL'siz application key bulunmaz.

### Önerilen commitler

```text
feat: add redis-backed ephemeral state
feat: distribute oauth pairing and command state
feat: distribute overlay state across backend instances
```

## 16. BH-05 — Trusted proxy ve client identity

### Amaç

İstemcinin sahte `X-Forwarded-For` header'ıyla rate-limit aşmasını engellemek.

### Uygulama adımları

1. TCP peer `SocketAddr` bilgisini request extension/extractor'a taşı.
2. Ortak `ClientIdentity` extractor oluştur.
3. `TRUSTED_PROXY_CIDRS` boot-time security config olsun.
4. Peer trusted değilse `X-Forwarded-For`, `Forwarded` ve `X-Real-IP` yok sayılsın.
5. Peer trusted ise zincir sağdan/sola seçilen dokümante politikayla çözülsün.
6. Bozuk/çok uzun header bounded `unknown` bucket'a düşsün veya reddedilsin.
7. Pairing, waitlist, auth ve extension'daki kopya çözümleme kodunu kaldır.
8. Caddy incoming forwarding header'larını temizleyip kendisi yazsın.
9. `infra/deploy.md` ve config örneklerini güncelle.

### Testler

- Direct client sahte XFF gönderince peer IP kullanılır.
- Trusted proxy gerçek client IP'yi iletir.
- Çoklu proxy zinciri deterministik çözülür.
- Bozuk/uzun header sınırsız key üretmez.
- XFF rotasyonu login/pairing limitini aşamaz.

### Önerilen commit

```text
fix: trust forwarded addresses only from configured proxies
```

## 17. BH-06 — Bounded public/realtime kaynaklar

### 17.1 Overlay lifecycle

Mevcut `subscribe` bilinmeyen token için channel oluşturur. Explicit lifecycle ekle:

```rust
register(token, owner, expires_at)
subscribe(token)
unregister(token)
```

Kurallar:

- `stream_start` token register eder.
- stop/disconnect token unregister eder.
- bilinmeyen token allocation yapılmadan reddedilir.
- global channel limiti ve TTL vardır.
- token ve global subscriber limiti vardır.
- snapshot satır ve byte olarak bounded'dır.
- cleanup eski session'ın yeni token'ını silmez.

> **Not (birleştirmede eklendi):** Bölge D1 (kalıcı token) sonrasında, `register()`
> **aynı token'ın** farklı stream oturumları için tekrar tekrar çağrılabilmelidir
> (token artık `stream_start` başına değil kullanıcı başına). Her `register()` çağrısı
> o token'ın bounded snapshot'ını sıfırlar/yeni TTL ile açar — Bölge D2'nin "stale
> snapshot replay edilmesin" şartı ayrı bir `reset_snapshot` metodu **değil**, bu
> davranışın kendisidir.

### 17.2 OAuth state

- Issue öncesi expired cleanup
- User/IP başına outstanding state limiti
- Global limit
- Public OAuth start endpoint'ine rate-limit
- Redis TTL/GETDEL kullanımı

### 17.3 WebSocket

- Global connection semaphore
- User/device başına limit
- Auth öncesi minimum allocation
- Control message/audio frame boyut limiti
- Idle timeout
- Ping/pong policy
- Açık close/error code
- Graceful permit release

### Testler

- Binlerce sahte token map'i büyütmez.
- Stream kapanışında channel temizlenir.
- Geçerli aktif token subscribe edilebilir.
- **Kalıcı token birden fazla stream oturumunda tekrar register edilebilir, her
  seferinde snapshot sıfırlanır (birleştirmede eklendi).**
- OAuth outstanding/global limit uygulanır.
- Expired state temizlenir.
- WS connection limiti aşılınca kontrollü red gelir.
- Büyük frame kontrollü kapatılır.
- Permit disconnect/panic-safe şekilde serbest bırakılır.

### Önerilen commitler

```text
fix: bound overlay channel lifecycle
fix: expire and limit oauth authorization state
fix: enforce websocket connection and payload limits
```

## 18. BH-07 — Ayrı SDK mTLS listener

### Amaç

Public browser/webhook trafiğini client certificate zorunluluğundan ayırırken SDK
trafiğinde gerçek zorunlu mTLS sağlamak.

### Router/listener tasarımı

`app::router` şu parçalara ayrılmalı:

```rust
public_router(state)
sdk_router(state)
```

Public router:

- public/dashboard/admin API
- Stripe webhook
- OAuth callbacks
- overlay
- health

SDK router:

- `/ws/stream`
- yalnız gerçekten SDK'ya ait device endpoint'leri

SDK listener `SDK_BIND_ADDR` üzerinde TLS 1.3 ve mandatory client CA doğrulamasıyla
çalışır. Public listener ayrı sertifika/proxy politikasına sahiptir.

### Sertifika-device binding

Yalnız güvenilen CA imzası yeterli değildir:

1. Cert SAN/URI içinde `sdk_session_id` veya device identity bulunmalı.
2. JWT `sid`, cert identity ve aktif DB session eşleşmeli.
3. Removed/revoked device kabul edilmemeli.
4. Cert kısa ömürlü olmalı ve rotate edilebilmeli.
5. Private key SDK'da üretilmeli; CSR sunucu/Vault PKI tarafından imzalanmalı.
6. Sertifika enrollment tek kullanımlık ve audit'li olmalı.
7. SDK connector gerçek client cert sunmalı.

Backend listener yazılmış, fakat SDK cert sunmuyorsa BH-07 tamamlanmış sayılmaz.

> **Not (birleştirmede eklendi):** Bu görev bittikten sonra Bölge D ve E'nin
> `stream_start` değişiklikleri bu yeni `sdk_router`/handshake üzerine yazılır (bkz.
> §8 Faz P3→P4). Bölge C1/C2 (SDK mic/relay OBS'ten bağımsız) bu görevden bağımsızdır
> ve paralel ilerleyebilir — SDK'nın cloud session'a bağlanma **zamanlamasını**
> değiştirir, mTLS/handshake mekanizmasını değiştirmez.

### Testler

- Public health cert olmadan çalışır.
- SDK listener cert olmadan handshake'i reddeder.
- Unknown CA ve expired cert reddedilir.
- Geçerli cert + eşleşen SDK JWT kabul edilir.
- Cert/JWT `sid` mismatch reddedilir.
- Public listener `/ws/stream` sunmaz.
- SDK listener public webhook/OAuth route'u sunmaz.
- Partial TLS env production startup'ı durdurur.

### Önerilen commitler

```text
feat: split public and sdk network listeners
feat: require client certificates on sdk listener
feat: bind sdk certificates to active device sessions
```

## 19. BH-08 — Production fail-closed startup

### Amaç

Development kolaylığı sağlayan fallback'lerin production güvenliğini zayıflatmasını
engellemek.

### Environment modeli

```text
APP_ENV=development|test|production
```

### Production startup zorunlulukları

- PostgreSQL erişilebilir
- Redis erişilebilir
- Vault production policy'ye uygun yapılandırılmış
- JWT secret en az 32 byte ve bilinen placeholder değil
- SDK mTLS listener/CA aktif
- `providers.mock_fallback=false`
- `auth.email_2fa=required`
- SMTP mevcut
- CORS exact allowlist
- Public URL HTTPS
- SDK URL WSS
- Trusted proxy topolojisi geçerli
- Persistence legacy file mode kapalı
- TLS env ya tamamen geçerli ya tamamen yok/uygun proxy mode
- Production secret persistence açık ve şifreli

Security invariant admin runtime mutation ile gevşetilememeli.

### Testler

- Her zorunlu ayarın eksikliği table-driven startup testiyle hata verir.
- Development DB/Redis olmadan açıkça başlayabilir.
- Empty/placeholder JWT reddedilir.
- Partial TLS config reddedilir.
- Production insecure config update `422` verir.
- Tam production fixture başarıyla doğrulanır.
- Error mesajı secret/DSN içermez.

### Önerilen commit

```text
fix: fail closed on insecure production configuration
```

## 20. BH-09 — Canlı entitlement ve SDK token güvenliği

> **Revizyon (2026-07-12, bkz. §0.1):** Bu görevin eski "kullanıcı token modeli"
> bölümü (kısa ömürlü access + rotating refresh + reuse'da family revoke) browser
> tarafında `auth::session` ile zaten uygulanmış durumda
> (`backend/src/auth/session.rs`, `browser_sessions` migration 0005): 15 dk
> `sd_access`, SHA-256 hash'li 30 gün refresh, her refresh'te rotation, reuse ve
> logout'ta family revoke. O bölüm bu plandan çıkarıldı. Browser auth'un devamı
> (OAuth'un cookie session'a taşınması, 2FA challenge, consent, logout)
> `docs/auth-workflow-hardening-plan.md`'nin işidir — bu görev browser session
> mekaniğine DOKUNMAZ, güvenlik olaylarında onun `SessionStore`'una entegre olur.

### Amaç

Plan, rol, parola veya device değişikliklerinden sonra eski yetkilerin (stale JWT
claim'leri veya açık SDK session'ları üzerinden) geçerli kalmasını engellemek.

### Schema

Migration ile en az:

- `users.token_version` (güvenlik olayında kullanıcının tüm access JWT'lerini
  geçersiz kılmak için)
- SDK session kayıtlarına revoke alanları (`revoked_at` vb.)
- SDK JWT `sid`/`jti` eşlemesi için gerekli kolon/index'ler

`browser_sessions` tablosu (migration 0005) family/status/rotated_at/revoked_at
alanlarını zaten taşıyor — browser için yeni token tablosu YOKTUR.

### Authorization kaynağı

- JWT plan claim'i authorization kaynağı olmayacak.
- Plan ve rol PostgreSQL/çok kısa Redis cache'ten çözülecek.
- Admin plan/role değişikliği cache invalidation üretecek.
- Cache/DB arızasında privileged access fail-closed olacak.
- Bu kural browser session'dan üretilen access JWT'ler için de geçerlidir —
  `SessionPrincipal` içindeki rol/plan anlık görüntüdür, canlı entitlement DB'den
  çözülür.

### SDK token modeli

- JWT içinde `sdk_session_id`/`sid` ve `jti`.
- Her SDK auth'ta session active kontrolü.
- Device removal session ve ilgili tokenları revoke eder.
- Aktif WS gerekiyorsa Redis revoke event'iyle kapatılır.

### Güvenlik olayları

- Password reset: `token_version++`; kullanıcının browser session family'leri
  `SessionStore::revoke` üzerinden, SDK tokenları kendi mekanizmasıyla revoke edilir.
- Role downgrade: eski admin erişimi anında kapanır.
- Account deletion request: tüm token/device revoke.
- Device removal: yalnız ilgili cihaz tokenları revoke.

### Testler

Mevcut regresyon (yeniden yazılmaz — `auth::session` testleri kapsıyor): refresh
iki kez kullanılamaz; reuse bütün family'yi revoke eder; logout family'yi revoke
eder; raw token DB'de tutulmaz.

Bu görevin ekleyeceği testler:

- Plan değişikliği eski access tokenla yeni entitlement verir.
- Role downgrade eski admin tokenını engeller.
- Password reset eski access JWT'leri VE browser refresh family'lerini geçersiz kılar.
- Device removal ilgili SDK JWT'yi engeller.
- İki cihaz birbirinden izole kalır.
- DB/cache failure güvenli davranır.

### P0 sınırı

Stripe'ın planı değiştirmesi bu görevin işi değildir. Bu görev DB plan değiştiğinde
auth katmanının bunu anında görmesini sağlar.

### Önerilen commitler

```text
fix: resolve authorization from live user state
fix: revoke user and device tokens on security events
```

## 21. BH-10 — Provider resilience ve WebSocket hata sözleşmesi

### Provider HTTP client standardı

1. Ortak configured HTTP client factory oluştur.
2. Connect, total request ve response-body timeout ekle.
3. Timeout değerlerini typed config ile sınırla.
4. Retry yalnız idempotent/güvenli çağrılarda çalışsın.
5. Exponential backoff + jitter kullan.
6. Stripe checkout, voice clone ve yan etkili POST otomatik retry edilmesin.
7. Provider bazlı circuit-breaker/cooldown uygula.
8. Ham provider body/error kullanıcıya veya loga aynen yazılmasın.

### Axum middleware

- Request ID
- REST timeout
- Body size limit
- REST concurrency/load shed
- WebSocket upgrade timeout'tan hariç
- Structured tracing span
- Graceful shutdown

### WebSocket protokolü

- Malformed JSON → client protocol error
- Invalid frame → codec/frame error
- STT primary+fallback başarısız → `4005`
- Translation primary+fallback başarısız → `4006`
- Retry edilebilirlik açık alanla ifade edilir
- Internal provider detayı sızmaz
- Log: provider, operation, latency, retry count, user/session reference

Streaming STT ve usage algoritmasına dokunma.

> **Not (birleştirmede eklendi — bkz. §8 Faz P5):** Bu görev **Bölge A2/A3'ten önce**
> tamamlanır. `process_audio`/`gateway.transcribe` çağrı zincirini ilk refactor eden bu
> görevdir; 4005/4006 kodları ve timeout sözleşmesi burada netleşir. Bölge A3
> (UtteranceAssembler) bu sözleşmeyi **frame başına değil flush başına** uygulayacak
> şekilde üstüne biner — aynı kod yolunu iki ajan bağımsız refactor etmez.

### Testler

- Yavaş provider timeout olur.
- Primary timeout sonrası fallback çalışır.
- Bütün providerlar hata verince `4005/4006` gelir.
- Malformed JSON `5000` dönmez.
- Büyük REST body `413`.
- Request ID response ve logda aynıdır.
- Yan etkili POST retry edilmez.
- WS upgrade long-lived timeout'tan etkilenmez.

### Önerilen commitler

```text
feat: standardize provider request timeouts
fix: map gateway failures to websocket protocol errors
feat: add request ids and bounded http middleware
```

## 22. BH-11 — Liveness, readiness ve health

### Endpoint sözleşmesi

- `/health/live`: process/event-loop; dependency çağrısı yapmaz.
- `/health/ready`: trafik almaya hazır mı; failure'da `503`.
- `/health`: geçici compatibility alias olabilir.

### Readiness bileşenleri

- PostgreSQL `SELECT 1`
- Redis ping
- Migration version
- Zorunlu config/secret varlığı
- Public listener state
- SDK mTLS listener state
- Cached provider-check durumu; her health çağrısında ücretli API çağrısı yok

Her probe kısa timeout taşımalı. Response secret, DSN veya raw provider hata gövdesi
içermemeli.

### Health-check command

Mevcut binary health-check plaintext varsayımını kaldır:

- public local health listener'a bağlan veya
- TLS-aware client kullan
- SDK mTLS portunu probe etme

### Testler

- DB kapalıyken liveness 200.
- DB/Redis kapalıyken readiness 503.
- Dependency kontrolü timeout limitini aşmaz.
- Response güvenli component status döndürür.
- Proxy ve direct TLS modlarında container health command çalışır.

### Önerilen commit

```text
feat: separate liveness and dependency readiness checks
```

## 23. BH-12 — Account deletion, audit ve retention

### Amaç

Hesap silme talebinin auth, DB, Redis, voice provider ve lokal sample katmanlarını
idempotent ve gözlemlenebilir şekilde temizlemesini sağlamak.

### Deletion workflow

External cleanup tek DB transaction'a sığmadığı için outbox/workflow kullan:

```text
requested
external_cleanup_pending
local_cleanup_pending
database_cleanup_pending
completed
failed_retryable
manual_review
```

### Endpoint

```text
DELETE /api/account
```

Şartlar:

- yakın zamanda doğrulanmış parola/2FA
- explicit confirmation
- idempotency
- request anında yeni login/refresh engeli
- aktif WS/SDK session kapatma
- token/device revoke

### Cleanup kapsamı

- External voice model delete
- Encrypted local voice sample delete
- Voice metadata
- OAuth/platform tokenları
- Relay targets/settings
- SDK sessions
- Redis/cache keys
- User record
- Usage/stream kayıtları için belgelenmiş yasal retention/anonymization kararı
- Backup retention yaklaşımı

Voice model reddedildiğinde sample'ın tutulması açık policy'ye bağlı olmalı. Güvenli
varsayılan reddedilen sample'ı silmektir.

### Audit service

`audit_logs` tablosunu gerçek servis/repository ile kullan:

- secret/key kullanımı; değer olmadan
- admin config mutation
- plan/role mutation
- başarısız auth/mTLS
- token revoke
- voice consent/withdrawal
- deletion request/completion

Audit metadata allowlist tabanlı olmalı. Token, password, key, ham sample ve tam
provider response yasaktır.

### Consent

> **Revizyon (2026-07-12, bkz. §0.1):** Versiyonlu consent ledger'ın veri modeli ve
> implementasyonu `docs/auth-workflow-hardening-plan.md`'nin (A-06, Faz 4)
> sahipliğindedir — burada yeniden tanımlanmaz. BH-12 yalnız silme akışının
> consent'e etkisini kapsar:

- Hesap silinirken consent kayıtları için belgelenmiş retention/anonymization
  kararı uygulanır (yasal kanıt gerekiyorsa anonimize edilerek tutulabilir).
- Silme talebi ve tamamlanması audit'e yazılır; consent ledger'ın modeli
  değiştirilmez.

### Retention belgesi

Şunlar için süre/policy yaz:

- audit log
- operational log
- OAuth/2FA/reset state
- voice sample
- failed deletion retry
- backups

### Testler

- Delete iki kez çağrılınca idempotent.
- Talep sonrası login/refresh engellenir.
- External cleanup geçici hatada retry edilir.
- Voice sample fiziksel/kalıcı depodan silinir.
- Başka kullanıcının verisi etkilenmez.
- Audit secret içermez.
- Redis keys temizlenir.
- Consent version/timestamp korunur.

### Önerilen commitler

```text
feat: persist security audit events
feat: add idempotent account deletion workflow
fix: purge rejected and deleted voice samples
```

(`feat: persist versioned legal consent` bu listeden çıkarıldı — consent ledger
`docs/auth-workflow-hardening-plan.md` Faz 4'ün commit'idir.)

## 24. BH-13 — Faz dışı özellikler için gate

### Amaç

Faz 2/3 kodunu silmeden production saldırı/operasyon yüzeyini aktif ürün kapsamıyla
sınırlamak.

### Örnek flag'ler

```text
feature.voice_clone.enabled=false
feature.self_hosted_stt.enabled=false
feature.twitch_extension.enabled=false
feature.local_tts.enabled=false
feature.gpu_provisioning.enabled=false
```

> **Not (birleştirmede eklendi):** `feature.self_hosted_stt.enabled` ve
> `feature.gpu_provisioning.enabled`, statik altyazı planının **Bölge B1**'i tarafından
> tüketilir — B1'in "provision başarısında `stt.provider=selfhosted` otomatik aktivasyon"
> davranışı bu iki flag'in İÇİNDE tanımlıdır (bkz. §6 madde 1, §8 Faz P2). Bu görevi
> uygulayan ajan, testlerini yazarken Bölge B1'in beklediği iki-seviyeli davranışı
> (flag kapalı → provizyon SSH açmadan reddedilir; flag açık + `autoactivate=true` →
> otomatik aktivasyon çalışır) hesaba katmalıdır — B1 bu görevden **sonra** uygulanır.
> `feature.twitch_extension.enabled` için böyle bir çakışma yok: Bölge F1 bu flag'i
> açmaya çalışmaz, yalnız mevcut (kapalı) durumuna karşı smoke test yazar.

### Uygulama adımları

1. Flag'leri DB/config kaynaklı yap.
2. Production default kapalı.
3. Disabled handler external çağrı/process başlatmadan `404` veya feature-disabled
   yanıtı versin.
4. Public response yalnız coming-soon görünürlüğünü taşısın.
5. Admin gerçek gate durumunu görebilsin.
6. Disabled feature'ın background worker/provider/provisioner'ı kurulmasın.
7. Restart gerektiren ve runtime değişebilen gate'leri dokümante et.

### Testler

- Voice clone disabled iken provider çağrılmaz.
- Provisioning disabled iken SSH process başlatılmaz.
- Extension disabled iken state allocation yapılmaz.
- Gate açılınca plan/role authorization hâlâ uygulanır.
- Internal gate config public'e sızmaz.

### Önerilen commit

```text
fix: gate post-mvp backend features behind runtime flags
```

## 25. BH-14 — Async I/O, log ve panic temizliği

### Senkron dosya I/O

- DB/Redis'e geçen store'larda production file persistence kaldır.
- Dev/legacy adapter request worker'da `std::fs` çalıştırmamalı.
- Gerekli blocking iş `spawn_blocking` veya ayrı worker üzerinden yürümeli.
- Her mutation'da full snapshot rewrite yapılmamalı.

### Operational logging

Tercih:

1. Production structured stdout
2. Harici log collector
3. Admin için bounded ring/query backend
4. Dosya gerekiyorsa async writer + size/time rotation + retention

Log queue dolması ölçülebilir ve bounded olmalı. Request thread disk I/O beklememeli.

### Panic noktaları

Production kodunu test kodundan ayırarak:

- lock poisoning
- system clock
- cipher construction
- store state varsayımları

için `expect/unwrap` yerine typed error veya açık fail-closed davranış kullan.

### Testler

- Yavaş disk request latency'sini bloke etmez.
- Log configured limitte rotate olur.
- Queue dolması bounded davranır.
- Poisoned lock process panic'i üretmez.
- Clock hatası kontrollü error olur.
- Production source check yasak panic noktalarını yakalar.

### Önerilen commitler

```text
chore: remove blocking file persistence from request paths
fix: bound and rotate backend log persistence
chore: remove panic paths from backend runtime code
```

## 26. BH-15 — CI kalite ve güvenlik kapıları

### Format borcu

Önce fonksiyonel değişikliklerden ayrı mekanik commit:

```bash
cargo fmt --all
git diff --check
```

```text
chore: normalize rust formatting
```

Bu commit büyük olursa başka functional değişiklik içermemelidir.

### CI'a eklenecekler

1. `cargo fmt --all -- --check`
2. PostgreSQL service container
3. Redis service container
4. Migration run
5. Ignored DB testlerini gerçek DB ile çalıştırma
6. `cargo clippy --workspace --all-targets --all-features -- -D warnings`
7. Dependency advisory audit
8. License/source policy
9. Coverage report
10. Auth, crypto ve kritik backend yollarında minimum %80 coverage
11. Migration drift/schema smoke
12. P0 Docker düzeltildikten sonra backend image build smoke

### Toolchain

- CI stable, Docker 1.85 ve local toolchain farkını bilinçli kararla çöz.
- Önce ilan edilen MSRV ile workspace build'i dene.
- MSRV geçmiyorsa rastgele yükseltme yapma.
- `rust-toolchain.toml` ile reproducible sürüm düşün.
- Toolchain değişikliği ayrı commit ve doküman güncellemesi olmalı.

### DB/Redis test izolasyonu

- Test başına unique schema/database veya transaction rollback
- Redis key'lerinde test UUID namespace
- Paralel testler birbirinin verisini bozmamalı
- `#[ignore]` testler CI'da explicit job ile çalışmalı veya güvenli şekilde
  ignore kaldırılmalı

### Önerilen commitler

```text
test: run postgres and redis integration suites in ci
chore: enforce rust formatting in ci
chore: add dependency security policy checks
test: enforce coverage for critical backend paths
```

---

## Bölge A — Utterance Birleştirme (STT kalite/latency temeli)

### A1. Protokol: reserved byte → flags

> **Durum (2026-07-13): TAMAMLANDI.** SDK `FrameFlags` ile byte[5]'e
> `utterance_end` bitini yazıyor; backend parser aynı biti okurken `0x00` eski
> frame'leri ve bilinmeyen üst bitleri geriye/ileri uyumlu kabul ediyor. SDK,
> backend ve cloud E2E çağrıları yeni imzaya geçirildi; A2'nin gerçek VAD sonu
> üretimi ayrı ve açık kalır.

**Bugün:** Binary frame header 8 byte: magic `SDAU` + codec + `[5] reserved: 0x00` +
payload_len (`docs/websocket-protocol.md` "Audio Binary Frame"; SDK
`sdk/src/core/audio_frame.rs`; backend parser `backend/src/ws/frame.rs`).

**Yapılacak:** Byte[5] `flags` olur: `bit0 = utterance_end` (bu frame bir cümlenin son
frame'i). `0x00` = işaret yok → tam geriye uyumlu. `build_audio_frame`'e flags
parametresi; `frame::parse` çıktısına `utterance_end: bool`.
`docs/websocket-protocol.md` güncellenir (frame tablosu + akış diyagramı notu).

Doğrulama: İki tarafta unit test — flag'li/flag'siz frame round-trip; eski format
(`0x00`) parse edilmeye devam eder.

### A2. SDK: VAD cümle sonu işareti

**Bugün:** `VadGate` (`sdk/src/core/vad.rs`) RMS eşiği + 200ms hangover ile frame
başına voiced/silent kararı verir; `CapturePipeline::next_frame`
(`sdk/src/core/gate.rs:67`) sessiz frame'leri yutar, voiced frame'i tek tek döndürür.
Cümle sonu bilgisi dışarı çıkmıyor.

**Yapılacak:** `VadGate` voiced→silent geçişini (hangover penceresi kapandığı an)
bildirir; `CapturePipeline` utterance sonunu işaretler. İki geçerli tasarım
(implementasyoncu seçer, testle sabitler):
- (a) son voiced frame flag'li gönderilir (bir frame gecikmeli işaretleme gerektirir
  — buffer-1 yaklaşımı), veya
- (b) hangover kapanınca payload'sız (`payload_len=0`) `utterance_end` frame'i gönderilir
  — daha basit; backend'de "boş payload'lı flag frame = sadece flush sinyali" olarak
  işlenir.

`cloud_session.rs` değişen akışı olduğu gibi taşır (frame'ler zaten kanal üzerinden
geçiyor).

Doğrulama: `gate.rs` testleri: konuşma→sessizlik→konuşma dizisinde doğru noktada
utterance_end üretilir; `MockCapture` ile mevcut test deseni kullanılır.

### A3. Backend: UtteranceAssembler

> **Bağımlılık (birleştirmede eklendi, bkz. §8 Faz P5):** Bu görev **BH-10'dan sonra**
> yapılır. STT/çeviri hata kodları (4005/4006) ve provider timeout sözleşmesi BH-10'da
> netleşir; A3 bu sözleşmeyi bozmaz, "frame başına" yerine "flush başına" uygular —
> yani bir utterance flush'ının STT'si başarısız olursa BH-10'un tanımladığı aynı 4005
> close code'u tetiklenir (frame başına ayrı bir hata yolu **eklenmez**).

**Bugün:** `StreamSession::process_audio` (`backend/src/pipeline/mod.rs:142`) her
çağrıda `gateway.transcribe(audio)` — frame başına STT.

**Yapılacak:** `StreamSession`'a tampon: gelen frame payload'ları birikir; **flush
koşulları** (öncelik sırasıyla):

1. `utterance_end` flag'i geldi (A1/A2 yolu — ana yol, ek bekleme yok),
2. tampon `stt.utterance.max_ms` eşdeğerini aştı (seed: `5000`; 16kHz s16le → ms başına
   32 byte, 5sn = 160.000 byte) — uzun kesintisiz konuşmada ara flush,
3. eski/işaretsiz SDK fallback'i: son frame'in üzerinden `stt.utterance.gap_ms`
   (seed: `400`) geçti ve tampon boş değil — varış zamanı bazlı; WS döngüsü
   mesaj-güdümlü olduğundan bu kontrol ya `tokio::select!` + timeout ile ya da bir
   sonraki frame gelişinde "arada boşluk var mıydı" kontrolüyle yapılır (ikincisi
   daha basit; boşluk sonrası gelen ilk frame önce eski tamponu flush ettirir).

Flush → **tek** `gateway.transcribe` çağrısı → mevcut `subtitle_for` akışı (çeviri +
overlay publish değişmez). Kota tick'i (`tick_usage`) davranışı korunur (duvar saati
bazlı — frame başına değil flush başına çağrılması sonucu değiştirmez, testle
doğrula — bu, hardening planının P0 dışı bıraktığı "usage frame-başına yuvarlama"
hatasını düzeltmez, sadece bozmaz). Config anahtarları `backend/src/config.rs`
seed'ine, **BH-02'nin `ConfigSpec` registry formatında** eklenir (admin panelden
restart'sız değişir).

Doğrulama: Pipeline testleri: (1) flag'li dizi → tek transcribe çağrısı (MockStt çağrı
sayacı), (2) 5sn+ kesintisiz → max_ms'te ara flush, (3) işaretsiz frame'ler + boşluk →
gap flush, (4) boş tampon flush edilmez, (5) tüm provider'lar flush'ta hata verince
BH-10'un `4005` close code'u tetiklenir, (6) mevcut testler kırılmaz.

### A4. stt-server notu

**Yapılacak:** Kod değişikliği yok (batch `/transcribe` cümle istekleriyle birebir
uyumlu). `main.py` docstring'ine "girdi = VAD ile bölünmüş tek utterance (tipik
1-6sn), 16kHz mono s16le" notu; isteğe bağlı çok kısa girdi (<100ms) için erken boş
dönüş.

---

## Bölge B — GPU Worker Otomatik Aktivasyon + Sağlık İzleme

### B1. Provision başarısında otomatik aktivasyon

> **Bağımlılık (birleştirmede eklendi, bkz. §6 madde 1, §8 Faz P2):** Bu görev
> **BH-13'ten sonra** yapılır. `feature.gpu_provisioning.enabled` kapalıyken
> `provision_server` hiç çağrılamaz (SSH süreci başlamaz — BH-13 testi).
> `feature.self_hosted_stt.enabled` kapalıyken `stt.provider` asla `selfhosted`'a
> geçemez, `stt.selfhosted.autoactivate=true` olsa bile. Prod varsayılanı ikisi de
> `false`'tur; bu görevin "otomatik aktivasyon" davranışı yalnız bu iki flag admin
> tarafından açıldıktan sonra devreye girer.

**Bugün:** `provision_server` (`backend/src/stt_servers/mod.rs:634`) başarıda
`pin_fingerprint` + `mark_ready(endpoint, token, gpu_name, vram_mb)` yapar;
`stt.provider` değişmez (varsayılan `deepgram`, `backend/src/config.rs:69`). Gateway
seçim/fallback: `backend/src/gateway/mod.rs:77-103` (`stt.provider` → `stt.fallback`);
`SelfHostedStt` her istekte `active_ready()` ile sunucu çözer
(`backend/src/gateway/selfhosted_stt.rs:55`), yoksa `Unavailable` → fallback zaten
çalışır.

**Yapılacak:**
- `provision_server` girişinde `feature.gpu_provisioning.enabled` kontrolü (kapalıysa
  `FeatureDisabled` hatası döner, SSH/provisioner client kurulmadan).
- `mark_ready` başarısından sonra, `feature.self_hosted_stt.enabled` VE
  `stt.selfhosted.autoactivate` (yeni seed: `true`) ikisi de açıksa
  `service_config.set("stt.provider", "selfhosted")`. `stt.fallback` dokunulmaz
  (deepgram kalır). Logger'a Info kaydı ("stt provider auto-activated: selfhosted").
- Not: provision `tokio::spawn` içinde koşuyor (`mod.rs:658`) — `ServiceConfig` Arc'ı
  task'a taşınmalı.

Doğrulama: Test: `feature.gpu_provisioning.enabled=false` → provision denemesi SSH
açmadan reddedilir; flag açık + FakeProvisioner başarı + `feature.self_hosted_stt.enabled=true`
→ config `selfhosted` oldu; `feature.self_hosted_stt.enabled=false` → autoactivate=true
olsa da değişmedi; autoactivate=false → değişmedi; provision hatası → değişmedi.

### B2. Periyodik health-check döngüsü

**Bugün:** `last_health_check_at` sadece provision anında set edilir (`mark_ready`,
`mod.rs:316`); arka plan izleme yok. Worker `/health` endpoint'i hazır
(`stt-server/main.py:119` — `{status, model, gpu_name, vram_mb}`).

**Yapılacak:** `main.rs`'te uzun ömürlü `tokio::spawn` döngüsü (mevcut spawn
desenleri: `main.rs:622` shutdown watcher): her `stt.selfhosted.health_interval_secs`
(yeni seed: `60`) saniyede Ready sunucuların `/health`'ine bearer token'la GET; başarı
→ `last_health_check_at` güncelle, başarısızlık (N ardışık, örn. 3) → `status=Error` +
`status_detail`. Store'a `record_health(id, ok, detail)` benzeri metod eklenir. Aktif
provider selfhosted iken worker Error'a düşerse ek bir şey gerekmez — gateway
`Unavailable` → Deepgram fallback zaten devrede; görünürlük panelde sağlanır.

Doğrulama: Store metodu unit testleri; döngü mantığı (ardışık hata eşiği, Ready'e
dönüş) mock HTTP ile test.

### B3. Admin panel görünürlüğü

**Bugün:** `frontend/src/components/admin-stt-servers-panel.tsx` — ekle/provision/sil
var; `last_health_check_at` tipte var (satır 24) ama render edilmiyor; aktif
sağlayıcı gösterilmiyor; aktifleştirme kontrolü yok.

**Yapılacak:** (1) Panel başlığına aktif STT sağlayıcı rozeti (**BH-01 sonrası admin
full-config endpoint'inden** `stt.provider` — `GET /config` public endpoint'i değil,
çünkü BH-01 bu değeri internal sayıp public allowlist'ten çıkarabilir); (2) sunucu
satırına `last_health_check_at` ("son sağlık: X dk önce") + Error durumunda
`status_detail`; (3) "Aktifleştir / Devre dışı bırak" butonu (`PUT /admin/config` ile
`stt.provider` set — mevcut config panel endpoint'i yeniden kullanılır, yeni backend
yüzeyi gerekmez); flag'ler kapalıyken (`feature.gpu_provisioning.enabled=false`)
buton devre dışı + "feature kapalı" ipucu.

Doğrulama: Mevcut panel test deseni (`admin-stt-servers-panel.test.ts`) genişletilir;
`npm run test` + `npm run lint`.

---

## Bölge C — Mikrofonun OBS Publish'inden Bağımsızlaşması

### C1. Cloud session'ı hemen başlat

**Bugün:** `run_desktop_shell` (`sdk/src/lib.rs:93`): targets sync → heartbeat →
`bind_ingest_listener` → **`accept_and_relay` OBS bekler (bloklar)** → relay başlar →
ancak ondan sonra mic capture + `run_cloud_session` spawn edilir (`lib.rs:128-180`).
`dub_audio_sinks` relay engine'den alınıp cloud session'a veriliyor (`lib.rs:149`).

**Yapılacak:** Mic capture + cloud session, listener bind'ından hemen sonra
(OBS'ten bağımsız) spawn edilir. Relay accept ayrı task olur. Dub sinks bağı: relay
engine henüz yokken **boş map** ile başlanır — bugünkü fiili durumla aynı (TTS
`tts.provider=disabled` varsayılan, dub target'sız yayıncıda map zaten boş;
`route_dub_audio` sink yoksa sessizce düşürüyor, `cloud_session.rs:125`). Engine
sonradan başlayınca sink'lerin canlı bağlanması Faz 2 mux işine not düşülür (bu plan
kapsamı dışı). `ShellStatus.mic_capture_ok` semantiği güncellenir (artık OBS
beklemeden set edilir — `lib.rs:63` yorumu düzeltilir).

Doğrulama: Mevcut sdk testleri + yeni test: OBS hiç bağlanmadan cloud session başlar
(MockCapture ile; `scripts/smoke.sh`'teki mevcut e2e deseni referans).

### C2. OBS yeniden bağlanma

**Bugün:** `accept_and_relay` tek seferlik (`sdk/src/core/relay_service.rs:27` —
"OBS koparsa çağıran tekrar çağırmalı" ama `run_desktop_shell` çağırmıyor).

**Yapılacak:** Accept döngüye alınır: OBS kopunca engine düşer, listener yeni publish
bekler, yeni `RelayEngine` kurulur (targets yeniden sync edilebilir).
`relay_listening` status'u doğru yansıtılır.

Doğrulama: Test: iki ardışık publish bağlantısı — ikisi de relay'lenir.

---

## Bölge D — Kalıcı Overlay Token (Kick / OBS Browser Source)

> **Bu bölge birleştirmede önemli ölçüde güncellendi.** Orijinal tasarım (D1)
> `RelayTargetStore::with_file`'ı klonlayan yeni bir şifreli **dosya** store
> öneriyordu. Bu, hardening planının §7.1 hedef mimarisiyle (overlay/relay state için
> "Instance-local snapshot" ve "Sınırsız HashMap/VecDeque" production'da yasak) doğrudan
> çelişiyordu — bkz. §0 çakışma #2. Aşağıdaki D1/D2, **BH-03 (Postgres) ve BH-04/BH-06
> (Redis-backed overlay hub)** üzerine kurulacak şekilde yeniden yazıldı. Bu bölge
> §8 Faz P2'de, BH-03/04/06'dan **sonra** uygulanır.

### D1. Kullanıcı başına kalıcı token (BH-03/BH-04/BH-06 üzerine, güncellendi)

**Bugün:** Her `stream_start`'ta `uuid::Uuid::new_v4()` (`backend/src/ws/mod.rs:344`);
kalıcılık yok (grep: migrations/schema'da overlay token yok).

**Yapılacak:**
- Postgres migration (BH-03B'nin migration setine eklenir):
  `overlay_tokens(user_id PK/FK, token UNIQUE, created_at)`.
- Yeni repository (BH-03'ün domain-repository desenini izler):
  `token_for(user_id) -> token` — yoksa üret + insert (`ON CONFLICT DO NOTHING` +
  re-select, iki instance eşzamanlı çağırsa bile aynı token'a yakınsar).
- `stream_start` yeni uuid yerine bu repository'den okur.
- Kanal/abone yaşam döngüsü BH-06'nın `OverlayHub.register(token, owner, expires_at)`
  API'sinden geçer — token kalıcı olsa da her `stream_start` canlı kaydı yeniden
  `register` eder (expires_at yenilenir, snapshot sıfırlanır — bkz. BH-06 notu),
  `stream_stop`/disconnect `unregister` çağırır.
- AppState'e repository alanı + main.rs kurulumu + `AppState::skeleton` (in-memory
  adapter, mevcut test deseniyle).

Doğrulama: Repository unit testleri (üret/oku/persist round-trip, iki instance aynı
token'ı görür — BH-03'ün çoklu-instance testiyle aynı desen); ws testi: iki ardışık
`stream_start` aynı token'ı verir VE her ikisinde de BH-06 `register`/`unregister`
çağrılır.

### D2. Liveness'i token varlığından ayır (BH-04/BH-06 üzerine, güncellendi)

**Bugün:** "Canlı mı" = `ActiveStreams.get(user).is_some()` — tüketiciler:
`get_overlay_token.active` (`backend/src/overlay/mod.rs:205`), ext_twitch
`get_state.live` (`backend/src/ext_twitch/mod.rs:435`) ve `captions_ws` 409 kapısı
(`:504-506`). `clear_if` last-writer-wins koruması (`overlay/mod.rs:164`).
`OverlayHub` kanalları token-keyed, snapshot hiç temizlenmiyor (`overlay/mod.rs:38-46`)
— kalıcı token'da önceki yayının son satırları yeni yayının başında replay edilirdi.

**Yapılacak:** `ActiveStreams`in "aktif yayın var mı" verisi BH-04'ün Redis TTL
tasarımıyla aynı kaynaktan gelir (canlılık = Redis'te `active-stream:{user_id}` key'i
var mı). `get_overlay_token` her zaman D1'in kalıcı token'ını döner + `active` flag'i
bu Redis canlılık kaydından. ext_twitch aynı canlılık sinyalini okur (yapısı
değişmez — `channel→binding→user→live?` zinciri korunur, overlay token izleyiciye
yine sızmaz). `stream_start`'ta BH-06'nın `register(token, owner, expires_at)`
çağrısı ilgili token'ın snapshot'ını temizler/yeniden açar — **ayrı bir
`reset_snapshot` metoduna gerek yok**, bu `register`'ın kendi davranışıdır (bkz.
BH-06 §17.1 notu).

Doğrulama: Overlay + ext_twitch mevcut testleri BH-06'nın yeni API'siyle güncellenir;
yeni testler: yayın kapalıyken `active=false` ama token dolu; yeni yayın başında eski
snapshot replay edilmez (`register`'ın snapshot sıfırlama davranışı test edilir);
ext_twitch 409 davranışı korunur.

### D3. Panel: sabit URL + yenileme

**Bugün:** `caption-overlay-builder.tsx` canlı değilken placeholder gösterir
(`:102-108` — token yokken URL yok).

**Yapılacak:** URL yayın kapalıyken de gösterilir ("yayın başlayınca canlanır" notu —
OBS'e bir kez girilir); token sızıntısına karşı `POST /api/overlay/token/regenerate`
endpoint'i + panelde yenile butonu (regenerate sonrası eski token'ın D1 repository
kaydı güncellenir ve BH-06 hub'ında eski token `unregister` edilir).

Doğrulama: Frontend test + backend endpoint testi (regenerate → yeni token, eski
aboneler kopar).

---

## Bölge E — YouTube Native Caption Pusher

> **Sıra notu (birleştirmede eklendi, bkz. §8 Faz P3→P4):** Bu bölge `ws/mod.rs
> stream_start`'a dokunur — BH-07 (SDK mTLS listener ayrımı) ve BH-09 (canlı
> entitlement/SDK session revoke) tamamlandıktan, ve Bölge D bittikten sonra uygulanır.

### E1. Aktif broadcast keşfi

**Bugün:** `fetch_ingestion_url(access_token, broadcast_id)` broadcast id'yi
parametre alıyor (`backend/src/integrations/youtube.rs:72`); id'yi bulan kod yok.

**Yapılacak:** `youtube.rs`'e `find_active_broadcast(access_token) -> Result<ActiveBroadcast, YouTubeError>`:
`GET {api_base}/liveBroadcasts?broadcastStatus=active&mine=true&part=contentDetails`
→ ilk item'dan id + `closedCaptionsIngestionUrl` (tek çağrı, ayrıca
`fetch_ingestion_url` çağırmaya gerek kalmaz). Aktif yayın yoksa belirgin hata
(`NoActiveBroadcast`) — pusher bunu "henüz yayın açılmadı, retry" olarak yorumlar.

Doğrulama: Mevcut mock-server test deseniyle (`youtube.rs:161` `spawn_api_mock`)
unit testler.

### E2. CaptionPusher komponenti

**Bugün:** Subtitle fan-out tek noktada: `publish_and_send` →
`state.overlay.publish(&overlay_token, reply)` (`backend/src/ws/mod.rs:523`). Uzun
ömürlü abone deseni mevcut: `ext_twitch::forward_captions`
(`backend/src/ext_twitch/mod.rs:532`) — `OverlaySubscription` alır, snapshot replay +
`loop/select` ile subtitle filtreler.

**Yapılacak:** Yeni modül `backend/src/captions_push/` (veya `integrations/` altı):
`stream_start` başarısında (`ws/mod.rs:344-384`, `active_streams.set` yanında)
kullanıcının YouTube entegrasyonu bağlıysa (`Integrations` üzerinden kontrol) bir
pusher task spawn edilir:

- `OverlayHub.subscribe(overlay_token)` ile beslenir (BH-06'nın yeni `subscribe` API'si);
  **sadece `is_final: true`** subtitle'ları işler (interim push edilmez —
  kota/uyumluluk).
- Başlangıçta `find_active_broadcast` — yayın YouTube'da henüz açık değilse
  backoff'la yeniden dener (örn. 15sn aralık, config
  `integrations.youtube.captions.retry_secs`).
- Her final subtitle için: kaynak metin ana track'e (`push_caption(url, text, seq)`),
  her `translations[i]` kendi track'ine — ingestion URL'e `&lang={code}&name={code}`
  eklenerek. **`seq` sayacı dil başına ayrı ve monoton artar.**
- Access token her push turunda `get_fresh_access_token` ile alınır
  (`integrations/mod.rs:375` — 60sn marjlı yenileme zaten var).
- Hata → `Logger` Warn + backoff; pipeline asla bloklanmaz (pusher ayrı task,
  `broadcast::Receiver` lag'inde stale mesaj düşürmek kabul — `RecvError::Lagged`
  yut).
- Yaşam döngüsü: subscription kapanınca / stream_stop / plan limitinde doğal
  sonlanır (ext_twitch ile aynı).

Config (BH-02'nin `ConfigSpec` registry formatında): `integrations.youtube.captions.enabled`
(seed: `true`).

Doğrulama: Mock ingestion server ile (youtube.rs test altyapısı hazır): final
subtitle → dil başına doğru `lang` + artan `seq`; interim push edilmez; entegrasyon
yoksa pusher spawn edilmez; token refresh yolu test edilir.

### E3. AppState wiring

**Yapılacak:** `YouTubeCaptions` (`CaptionsSink` trait objesi — test için mock enjekte
edilebilir) + pusher fabrikası `AppState`'e (`state.rs:38` alan + `FromRef` gerekiyorsa;
`main.rs:569` `Ok(AppState {...})` kurulumu — `integrations`'tan sonra;
`AppState::skeleton` mock ile).

Doğrulama: `cargo test` — skeleton kullanan tüm mevcut testler derlenip geçer.

### E4. Canlı doğrulama maddesi

**Yapılacak:** `streamdubber_roadmap_final.md` "Canlı Doğrulama Listesi"nin "Overlay /
YouTube (1.9, 1.10)" bölümüne ek: "ingestion URL `lang`/`name` parametreli çok-track
push gerçek YouTube Live yayınında doğrulanacak" — bu parametre davranışı üçüncü
parti caption vendor pratiğinden biliniyor ama resmi dokümantasyonu zayıf; **canlı
testte tek-track fallback kararı verilebilsin diye pusher'da track-per-lang mantığı
tek fonksiyonda izole tutulmalı.**

---

## Bölge F — Twitch Extension Aktifleştirme (çoğu ops)

**Bugün:** EBS adapter'ı (`backend/src/ext_twitch/mod.rs`) ve extension frontend'i
(`extension/js/`) mock E2E seviyesinde tamamdır; ancak bu, gerçek STT→çeviri→viewer
caption zincirinin production-ready olduğu anlamına gelmez. Frame-başına STT,
process-local state, worker pool, viewer lifecycle ve dashboard broadcaster auth
boşlukları vardır.

**Yürütme kaynağı:** Extension'a özel code, test, operasyon, Hosted Test ve canary
kapılarının tek kaynağı artık
`docs/twitch-extension-production-readiness-plan.md`'dir. Bu bölüm yalnız birleşik
hardening sırasını korur: F1 smoke, o dokümandaki PR-13/PR-16 kanıtlarının bir
parçasıdır; `BH-13`, `BH-03/04/06`, `BH-10` ve `A2/A3` tamamlanmadan global activation
yapılamaz.

**Kapsam dışı (insan yetkisi):** Twitch console kaydı, DNS/TLS yayına alma, Hosted Test
yükleme ve review başvurusu; agent bunlar için artefact ve runbook hazırlar.

---

## Bölge G — Uçtan Uca Doğrulama + Dokümantasyon (G1)

- `scripts/smoke.sh` genişletmesi: flag'li utterance akışı → tek subtitle üretimi;
  kalıcı token ile iki ardışık session.
- `docs/commands/test-pipeline.md`'ye yeni senaryo: "GPU worker + utterance +
  overlay + (mock) YouTube push".
- `docs/websocket-protocol.md`: flags alanı, UtteranceAssembler backend akışı,
  `stream_ack.overlay_token`'ın artık kalıcı olduğu notu.
- `docs/audio-pipeline.md` + `docs/dashboard.md`: ilgili bölümler güncellenir.
- `streamdubber_roadmap_final.md`: tamamlanan işlerin tiki repo formatında
  (checkbox + inline HTML yorum).

---

## 27. Global test stratejisi

### 27.1 Her küçük commit öncesi

```bash
cargo test -p streamdubber-backend <ilgili_test_filtresi>
cargo fmt -p streamdubber-backend -- --check
```

### 27.2 Her iş paketi sonunda (BH veya Bölge)

```bash
cargo test -p streamdubber-backend
cargo clippy -p streamdubber-backend --all-targets --all-features -- -D warnings
cargo fmt -p streamdubber-backend -- --check
git diff --check
```

SDK/frontend'e dokunan Bölge görevleri için ayrıca:

```bash
cargo test -p streamdubber-sdk   # Bölge A2/C1/C2
npm run test && npm run lint     # Bölge B3/D3
```

### 27.3 Yüksek riskli paket sonunda

BH-03, BH-04, BH-07, BH-08, BH-09, BH-12, BH-15 **ve** Bölge D (BH-03/04/06'ya
bağımlı olduğu için) için:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

Ayrıca gerçek PostgreSQL/Redis integration testleri ve ilgili TLS smoke çalıştırılır.

### 27.4 Güvenlik negatif testleri

Her security endpoint için en az:

- missing auth
- wrong role
- expired/revoked token
- malformed input
- oversized input
- dependency failure
- duplicate/replay
- cross-user isolation
- secret redaction

testleri bulunmalıdır.

## 28. Migration ve rollout güvenliği

### 28.1 Genel kural

Schema migration, data migration ve runtime cutover aynı deploy içinde kontrolsüz
birleştirilmemelidir.

Önerilen expand/migrate/contract akışı:

1. Expand: yeni tablo/kolon, eski kod hâlâ çalışır.
2. Migrate: legacy veriyi idempotent taşı.
3. Verify: sayım, constraint ve örnek doğrulama.
4. Cutover: read source'u DB/Redis yap.
5. Observe: log/metrik hata oranını izle.
6. Contract: eski write/file yolunu kaldır.

Bu akış Bölge D1'in `overlay_tokens` migration'ı için de geçerlidir — yeni tablo
`stream_start` hâlâ eski uuid yolunu kullanırken eklenir (expand), sonra cutover
yapılır.

### 28.2 Rollback

- Forward-only migration geri alınmaz; yeni corrective migration yazılır.
- Cutover öncesi DB backup ve legacy file backup alınır.
- Legacy dosya verification tamamlanmadan silinmez.
- Dual-write yalnız sınırlı migration penceresi içindir; kalıcı mimari değildir.
- Security schema değişikliklerinde eski binary'nin yeni schema ile davranışı
  test edilir.

## 29. Commit ve PR sınırları

### Tek commit'e konmayacak kombinasyonlar

- PostgreSQL cutover + Redis migration
- mTLS listener + canlı entitlement/SDK revoke sistemi (BH-07 + BH-09)
- Config validation + bütün config değerlerinin değişmesi
- Account deletion + unrelated voice feature
- Format-all + functional değişiklik
- CI toolchain bump + dependency upgrade + feature refactor
- **GPU worker autoactivate (Bölge B1) + feature gate altyapısı (BH-13) — B1, BH-13
  bittikten sonra ayrı bir commit'tir (birleştirmede eklendi).**
- **Kalıcı overlay token (Bölge D1) + Redis/Postgres cutover (BH-03/04/06) — D1, o
  cutover'lar bittikten sonra ayrı bir commit'tir (birleştirmede eklendi).**

### Prefix

Repo kuralına göre yalnız uygun prefix kullan:

- `feat:`
- `fix:`
- `chore:`
- `test:`

Doküman-only commit için:

```text
chore: document backend hardening and static captions plan merge
```

## 30. Her iş paketi için Definition of Done

Bir BH veya Bölge maddesi ancak aşağıdaki şartların tamamında tamamlanır:

- Başlangıç regresyonu kaydedildi.
- Önce başarısız olan test eklendi.
- Yeni testler geçti.
- Backend (ve varsa SDK/frontend) regresyonu geçti.
- Clippy warnings-as-errors temiz.
- Format kontrolü temiz.
- Migration varsa gerçek PostgreSQL'de çalıştı.
- Redis davranışı varsa gerçek Redis'te çalıştı.
- Security değişikliği fail-open yol bırakmıyor.
- Secret/PII loglanmıyor.
- Cross-user ve multi-instance testleri var (ilgiliyse).
- §8'deki bağımlılık sırası ihlal edilmedi (örn. Bölge B1, BH-13'ten önce; Bölge D,
  BH-03/04/06'dan önce; Bölge A3, BH-10'dan önce merge edilmedi).
- `.env.example`, deploy ve protokol belgeleri güncel.
- Bu belgedeki §11 checklist'te yalnız ilgili madde tiklendi; roadmap'te ilgili
  madde varsa o da tiklendi.
- `git status`, `git diff` ve `git diff --cached` incelendi.
- İlgisiz dosya commit'e alınmadı.
- Tek sorumluluklu commit oluşturuldu.
- Kullanıcı istemediyse push yapılmadı.

## 31. Yeni ajan ilk çalışma turu

Yeni ajan ilk turda implementasyona başlamadan şu çıktıyı üretmelidir:

1. Okuduğu zorunlu belgeler
2. Güncel `git status`
3. Başlangıç test/Clippy/fmt sonucu
4. Seçtiği tek BH veya Bölge iş paketi — **§8'deki faz sırasına uygun olmalı**
5. Bu paketin P0 işlerle ve §8'deki bağımlılıklarla çakışma kontrolü
6. Eklenecek ilk failing test
7. Beklenen dosyalar
8. Migration/rollout riski
9. Tahmini commit sınırları

İlk önerilen functional iş **BH-01**'dir (§8 Faz P0). BH-03 gibi büyük veri
refactoruyla veya Bölge B/D gibi P1'e bağımlı işlerle doğrudan başlanmamalıdır.
Format borcu functional diff'i okunamaz yapıyorsa önce yalnız format için mekanik
commit hazırlanabilir.

## 32. Handoff şablonu

Bir ajan işi yarıda bırakır veya başka ajana devrederse final mesajında:

```markdown
## Backend hardening + statik altyazı handoff

- Aktif görev: BH-XX veya Bölge-X — görev adı
- Faz (§8): P0-P7'den hangisi
- Son tamamlanan commit: <hash> <mesaj>
- Çalışma ağacı: temiz/kirli ve ilgili dosyalar
- Eklenen migration: <dosya veya yok>
- Geçen testler: <komutlar>
- Başarısız/çalıştırılamayan testler: <neden>
- Alınan mimari karar: <kısa özet>
- Sonraki somut adım: <tek adım>
- Bloker: <varsa — özellikle §8'deki bir önkoşulun eksik olması>
- Dokunulmaması gereken kullanıcı/başka ajan değişiklikleri: <dosyalar>
```

## 33. Nihai başarı ölçütü

Bu plan tamamlandığında backend:

- İki veya daha fazla instance ile aynı kalıcı/ephemeral state'i paylaşabilmeli
  (kalıcı overlay token dahil).
- Public ve SDK trafiğini farklı güvenlik politikalarıyla sunabilmeli.
- Production'da eksik güvenlik ayarıyla başlamamalı.
- Sahte forwarding header, sınırsız token/state ve uzun ömürlü stale JWT ile
  kolayca aşılamamalı.
- Provider ve dependency arızalarını bounded timeout ve açık protokol hatalarıyla
  yönetebilmeli.
- Hesap/voice verisini izlenebilir ve idempotent şekilde silebilmeli.
- Liveness/readiness ile doğru trafik kabul kararı verebilmeli.
- Format, lint, test, gerçek DB/Redis, dependency security ve coverage kapıları
  CI tarafından otomatik zorlanmalı.

Ve admin panelden bir GPU worker eklenip provision edildiğinde (flag'ler açıkken),
yayıncının **tüm statik altyazı kanalları kutudan çıktığı gibi çalışmalı**:

1. **OBS Overlay** (`/o/{token}` Browser Source — Kick dahil her platforma videoya
   gömülü altyazı, kalıcı token ile)
2. **YouTube Native CC** (closedCaptionsIngestionUrl'e çok dilli caption track push)
3. **Twitch Extension** (izleyici tarafı canlı altyazı)

Bu ölçütler sağlanmadan roadmap'teki "Backend Hardening + Statik Altyazı" bölümü
tamamlandı sayılmamalıdır.

## 34. Roadmap Eşlemesi

| Bölge/BH | Roadmap |
|---|---|
| Bölge A | §1.7 STT notu (streaming yerine utterance-batch), §2.6 faster-whisper uyumu |
| Bölge B | §2.6 (GPU'lu sunucu maddesi; "Admin toggle" → otomatik aktivasyon, BH-13 flag'i içinde) |
| Bölge C | §1.8/1.9 canlı doğrulama ön koşulu |
| Bölge D | §1.9 OBS Browser Source; Canlı Doğrulama "Overlay / YouTube (1.9, 1.10)" |
| Bölge E | §1.10 YouTube Native CC; Canlı Doğrulama "Overlay / YouTube (1.9, 1.10)" |
| Bölge F | §2.4 Twitch Extension |
| BH-01..BH-15 | Roadmap'te ayrı "Backend Hardening" başlığı (§11'deki checklist) — hiçbir mevcut tikli maddeyi geçersiz kılmaz, production sertleştirme/doğrulama katmanıdır |
