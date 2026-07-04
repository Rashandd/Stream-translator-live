# Twitch Extension Uygulama Planı

Durum: Kabul edildi — Faz 2 implementasyonu başladı (kullanıcı onayı 2026-07-04)
Kapsam: StreamDubber Twitch Extension (viewer overlay + broadcaster config + EBS)
Faz uyumu: Faz 1'de yalnız hazırlık (extension kodu YOK), Faz 2'de implementasyon
Bağlı dokümanlar: `docs/twitch-extension-policy.md` (kabul edilmiş policy), `streamdubber_roadmap_final.md` §2.4, `DESIGN.md`

İlerleme (2026-07-04):

- [x] A1 overlay sertleştirme · [x] A2 channel_bindings · [x] A3 EBS JWT tasarımı
  (policy dokümanına eklendi) · [x] A4 domain kararı (`infra/deploy.md` §7)
- [x] B1 paket iskeleti · [x] B2 EBS (`backend/src/ext_twitch`) · [x] B3 viewer UI
  · [x] B4 broadcaster config + dashboard kartı
- [ ] A5 review operasyon hazırlığı (insan: test hesapları, canlı senaryo provası)
- [ ] B5 Hosted Test (insan: Twitch console kaydı, DNS, CSP/wss doğrulaması)
- [ ] B6 review başvurusu (B5 sonrası)
- Not: `ext.twitch.enabled` varsayılan **kapalı** — Faz 1 davranışı değişmedi;
  aktivasyon admin panelden (bkz. `infra/deploy.md` §7 kurulum sırası).

## Neden

- Roadmap 2.4 Twitch Extension'ı Faz 2'nin izleyici-dil-seçimi ayağı olarak tanımlıyor;
  policy dokümanı (2026-07-03, kabul edildi) review kurallarını ve teknik sınırları zaten
  sabitledi. Eksik olan: policy'yi bu repo'nun **gerçek koduna** bağlayan, Faz 1/Faz 2
  sınırını net çizen bir uygulama planı.
- Backend'de extension'ın ihtiyaç duyduğu çekirdek zaten çalışıyor:
  - `backend/src/overlay/mod.rs` — `OverlayHub` (token → `broadcast::channel`) her
    `subtitle`'ı abonelere push ediyor; `GET /overlay/{token}` auth'suz WS (e2e testli,
    `tests/overlay_api.rs`).
  - `ServerMessage::Subtitle` (`backend/src/ws/protocol.rs`) `source` + `translations:
    Vec<Translation>` taşıyor — **tüm hedef diller tek mesajda**. İzleyici dil seçimi için
    backend'de yeni pipeline gerekmez; seçim viewer tarafında filtre olarak uygulanır.
  - `backend/src/pairing.rs` — kısa kod ile cihaz eşleştirme deseni; broadcaster'ın Twitch
    kanalını StreamDubber hesabına bağlamak için aynı desen yeniden kullanılır.
  - DB-driven config (`config::get`), merkezi loglama + redaction, plan limitleri hazır.

## Faz Sınırı (KRİTİK)

CLAUDE.md ve policy dokümanı açık: **Faz 1'de Twitch Extension implementasyonu kesin
yasak.** Bu plan iki bölgeye ayrılır:

- **Bölge A — Faz 1 uyumlu hazırlık:** Extension'a özgü olmayan, Faz 1 ürününe de fayda
  sağlayan backend/ops işleri. Extension bundle'ı, EBS endpoint'i, Twitch console kaydı
  içermez. Bunlar policy dokümanındaki "Faz 2'ye geçilmeden önce tamamlanmış olmalı"
  koşullarını karşılar.
- **Bölge B — Faz 2 implementasyon:** Extension package, EBS, viewer/config UI, Hosted
  Test, review. Faz 2 kilidi açılmadan bu bölgeden tek satır kod yazılmaz.

## Mimari Özet

```
Twitch viewer (Video Overlay Extension, Twitch CDN'de static bundle)
        │  Twitch Helper JWT (onAuthorized)
        ▼
EBS = backend'de yeni /ext/twitch/* yüzeyi (ayrı modül: backend/src/ext_twitch/)
        │  channel_id → binding → aktif session overlay_token (SUNUCU İÇİ çözülür)
        ▼
OverlayHub (mevcut) ──► kanal-bazlı fan-out + cache + rate limit ──► viewer'lara caption
```

Kilit karar — **overlay token viewer'a asla verilmez.** `/overlay/{token}` token'ı bir
capability'dir ve dashboard/OBS URL'lerinde yaşar; binlerce izleyiciye dağıtılmaz. EBS,
channel binding üzerinden token'ı **sunucu içinde** çözer, `OverlayHub`'a kendisi abone
olur ve viewer'lara kanal-anahtarlı ayrı bir fan-out katmanından (cache + rate limit ile)
servis eder. Böylece policy'nin zorunlu kıldığı per-channel cache/rate-limit katmanı da
doğal olarak bu sınırda kurulur.

İkinci kilit karar — **transport gate (policy §Realtime Caption Transport):** hedef
`wss://` (EBS WS), ama Hosted Test'te CSP/allowlist doğrulanana kadar WebSocket zorunlu
bağımlılık yapılmaz; kısa-poll edilen cache'li caption-state endpoint'i baştan tasarlanır
(fallback sonradan eklenen bir yama değil, ilk günden ikinci yol).

---

## Bölge A — Faz 1 Uyumlu Hazırlık

Sıra bağımsız; her biri ayrı task + ayrı commit. Hiçbiri Twitch'e özgü API/console kaydı
veya extension kodu içermez.

### A1. Overlay fan-out'una kanal-bazlı cache + rate limit katmanı

- `OverlayHub` bugün sınırsız abone kabul ediyor (`CHANNEL_CAPACITY: 32` sadece broadcast
  buffer'ı). Extension viewer fan-out'u öncesi genel bir koruma katmanı gerekiyor — ama bu
  Faz 1'de OBS Browser Source + dashboard önizleme için de doğru bir sertleştirme:
  - Token başına eşzamanlı abone üst sınırı (config-driven: `overlay.max_subscribers`).
  - Son N altyazıyı tutan kısa "caption state" snapshot'ı (yeni bağlanan istemci boş ekran
    yerine son satırları görür — Faz 1 overlay'inde de bilinen bir UX eksiği).
  - Aşımda log + sade kapatma; agresif retry'a karşı istemci tarafında backoff zaten
    `/o/[token]` sayfasında var, davranış korunur.
- TDD: mevcut `overlay/mod.rs` unit test stiliyle — abone limiti, snapshot replay,
  token izolasyonunun bozulmadığı.

### A2. Kanal binding veri modeli (platformdan bağımsız)

- Yeni tablo: `channel_bindings` (`id`, `user_id`, `platform` — `"twitch"` ilk değer,
  `platform_channel_id`, `status`, `created_at`; `platform`+`platform_channel_id` unique).
- Migration: `backend/migrations/000X_channel_bindings.sql`.
- Faz 1'de bu tabloyu dolduran akış YOK (Twitch OAuth/extension yok) — sadece şema ve
  model katmanı hazırlanır ki Faz 2'de EBS `channel_id → user_id → aktif session`
  çözümlemesi hazır bir modele otursun. Platform kolonu sayesinde Kick/Chrome extension
  (roadmap 2.5) aynı modeli kullanır.
- TDD: model CRUD + unique constraint testleri.

### A3. EBS JWT doğrulama tasarım notu (kod değil, doküman)

- `docs/twitch-extension-policy.md`'nin EBS Authentication bölümünü somutlaştıran kısa
  tasarım: hangi crate (ör. `jsonwebtoken`), secret'ın Vault/env yolu
  (`TWITCH_EXT_SECRET`, base64), claim doğrulama sırası (`exp` → imza → `channel_id` →
  `role`), secret rotasyonu (iki aktif secret ile doğrulama denemesi).
- Çıktı: bu plana ek bir bölüm veya `docs/twitch-extension-policy.md`'ye "EBS JWT
  Tasarımı" eki. Kod Faz 2'de yazılır.

### A4. Domain ve allowlist kararları

- Policy'deki örnek domainler (`*.streamdubber.example`) gerçek karara bağlanır:
  öneri `https://ext.strovak.com` (EBS) — `api.strovak.com`'dan ayrı host, çünkü
  extension viewer trafiği ayrı ölçeklenmeli/ölçülmeli (CLAUDE.md'deki bandwidth
  izolasyon ilkesinin aynısı) ve Twitch allowlist'ine dar, extension'a özel bir origin
  yazılmalı.
- Production / staging / hosted-test için üç ayrı hostname şimdi belirlenir, DNS Faz 2'de
  açılır. Karar `infra/deploy.md`'ye not düşülür.

### A5. Review operasyon hazırlığı (insan işi, kod yok)

- Test broadcaster Twitch hesabı + 2 test viewer hesabı açılır.
- Review channel'da kullanılacak senaryo Faz 1 ürünüyle prova edilir: TR kaynak ses →
  EN altyazı, SDK + backend canlı (policy: "Twitch review için canlı çalışmayan
  backend/SDK ortamı sunmak" kesin yasak).
- Privacy Policy / Terms / support email URL'lerinin yayında olduğu doğrulanır.

### Faz 2 kapısı (gate)

Bölge B'ye geçiş koşulu — policy dokümanındaki liste + bu plan:

- [ ] Faz 1 caption pipeline canlı yayında stabil (roadmap 1.x çıkış kriterleri).
- [ ] A1–A5 tamam.
- [ ] Policy Gate Checklist (`docs/twitch-extension-policy.md`) ilk geçişi yapıldı.
- [ ] Kullanıcı Faz 2 başlangıcını onayladı.

---

## Bölge B — Faz 2 Implementasyon

Sıra bağımlı; her adım ayrı task + ayrı commit, TDD ile.

### B1. Extension package iskeleti — `/extension` (yeni üst-düzey klasör)

- Next.js'ten tamamen bağımsız, küçük static bundle: vanilla TypeScript + Vite (veya
  esbuild), çıktı `viewer.html`, `config.html`, tek CSS, tek JS.
- Kurallar (policy'den, build-time zorlanır):
  - Twitch Extension Helper her HTML'de ilk `<script>`.
  - iframe yok, `eval()` yok, obfuscation yok (minify açık, sourcemap review notunda).
  - Bundle bütçesi CI'da assert edilir: toplam < 1 MB (hedef < 200 KB — framework yok).
  - Web-safe font stack (Inter yüklenmez; `system-ui` öncelikli — mobile 3 sn hedefi).
- Görsel dil `DESIGN.md`'den: navy metin, pill kontroller, status renkleri (success/
  warning/danger), emoji YASAK, lucide ikonlar inline SVG olarak gömülür (CDN yok — CSP).
- Lokalizasyon: viewer UI metinleri bundle içi locale sözlüğünden (EN başlangıç), Twitch
  dil parametresi (`language` query) ile seçilir.

### B2. EBS — `backend/src/ext_twitch/` modülü

- Yeni route yüzeyi (`app.rs`'e eklenir):
  - `GET /ext/twitch/captions/{channel_id}` — WS upgrade; JWT header doğrulanır, binding
    çözülür, A1'deki kanal fan-out'una bağlanır. İlk mesaj: caption state snapshot.
  - `GET /ext/twitch/state/{channel_id}` — cache'li JSON snapshot (poll fallback;
    `Cache-Control` kısa TTL). WS gate'i geçemezse ana transport budur.
  - `POST /ext/twitch/bind` — broadcaster config sayfasından: `pairing.rs` deseniyle
    dashboard'da üretilen kısa kodu claim eder, `channel_bindings`'e yazar (JWT
    `role=broadcaster` şart).
- JWT middleware: A3 tasarımına göre; secret Vault/env (`TWITCH_EXT_SECRET`), koda
  gömülmez. Her istekte doğrulama; `role`/`channel_id` claim'i route parametresiyle
  eşleşmek zorunda.
- Rate limit: per-channel + per-viewer (opaque user id); 429 döner, log'a redaction'lı
  yazar. Plan limiti aşımı viewer'a ticari mesaj olarak DEĞİL "captions unavailable"
  durumu olarak yansır (policy §Commerce).
- Config anahtarları (DB-driven, restart'sız): `ext.twitch.enabled`,
  `ext.twitch.default_lang`, `ext.twitch.snapshot_lines`, `ext.twitch.rate.*` —
  hard-coded değer yok.
- TDD: JWT (geçerli / süresi dolmuş / yanlış imza / yanlış channel / yanlış role),
  binding claim (geçerli kod / kullanılmış kod / süresi dolmuş kod), fan-out e2e
  (`tests/overlay_api.rs` stili: sahte SDK → subtitle → extension WS'inde görünür),
  rate-limit 429, `ext.twitch.enabled=false` iken tüm yüzeyin kapalı olması.

### B3. Viewer UI (subtitles-only)

- İlk release kapsamı — policy'deki zorunlu kontroller, fazlası değil:
  - Dil seçimi: `Subtitle.translations` içindeki dillerden + kaynak dil; allowlist
    backend snapshot'ından gelir, serbest metin yok.
  - Altyazı aç/kapat.
  - Boyut ayarı (S/M/L) — okunabilirlik için zorunlu en az bir ayar.
  - Bağlantı/durum göstergesi (online / no captions yet / delayed / unavailable /
    not configured / offline).
- Render güvenliği: caption metni yalnız `textContent` ile basılır; HTML/markdown yok;
  dil kodları allowlist'ten doğrulanır; hata mesajlarında backend response body
  gösterilmez.
- Tercihler: anonim, `localStorage` (dil, boyut, açık/kapalı). Identity link İSTENMEZ.
- Player kontrolleriyle çakışmayan konum; collapsible; caption render throttle (frame
  drop yaratmaz); 429/5xx'te exponential backoff.
- Test: dil filtre mantığı + sanitizasyon için birim test (DOM tabanlı, ör. vitest +
  jsdom); durum makinesi geçişleri.

### B4. Broadcaster config UI + dashboard bağlama

- `config.html`: durum ("not configured" / "linked: kanal adı"), kısa kodla bağlama
  formu (B2'deki `bind` endpoint'i), varsayılan dil seçimi, "Open StreamDubber
  dashboard" off-site linki (açık off-site göstergesiyle — izinli tek yönetim linki).
- Twitch Configuration Service'e yalnız public değerler yazılır: `caption_enabled`,
  `default_lang`, `binding_ref` (opaque id), `required_config_version`. Secret/token
  ASLA (policy kesin yasağı).
- Required Configuration kullanılır: binding tamamlanmadan extension viewer'da aktif
  olmaz — viewer'a karışık hata yerine hiç görünmez ya da "not configured".
- Dashboard tarafı (Next.js, mevcut desenler): `/dashboard/platforms` veya captions
  sayfasına "Twitch Extension" kartı — bağlama kodu üretme (pairing UI deseni), bağlı
  kanal durumu. Fetch'ler `"use client"` + `NEXT_PUBLIC_API_URL` (proxy YASAK kuralı).

### B5. Hosted Test doğrulaması

- Bundle Twitch CDN'e yüklenir; gerçek CSP altında test edilir:
  - `wss://ext.strovak.com` çalışıyor mu? Çalışmıyorsa B2'deki poll fallback ana
    transport ilan edilir (kod hazır, karar değişikliği yeterli).
  - Mobile iOS/Android, portrait/landscape, slow network (500 Kb/s'de < 3 sn), theater/
    fullscreen, dark/light, caption burst, uzun metin, SDK kopması, backend kapalı.
- Bu adımın çıktısı: policy'deki Testing checklist'inin işaretlenmiş hali + allowlist'e
  girecek kesin URL listesi.

### B6. Review başvurusu (erken — süreç 2–4 hafta, B5 biter bitmez)

- Version Details: ad "StreamDubber Live Captions" (Twitch markası yok), doğru
  description, gerçek UI screenshot'ları, Privacy/Terms/support.
- Capabilities: identity link kapalı, chat kapalı, Bits kapalı, URL allowlist eksiksiz
  (yalnız `ext.strovak.com` + varsa dashboard panel URL'i).
- Walkthrough Guide + Change Log + review channel (A5 hesapları; canlı TR→EN senaryo).
- Review sürerken kod dondurulur; değişiklik gerekirse Local Test'e dönüş kuyruğu
  sıfırlayabilir — bu yüzden B5 tamamen bitmeden başvurulmaz.

### B7. Release sonrası küçük iterasyonlar (ayrı version'lar)

- Konum/kontrast viewer ayarı, son N satır geçmişi, gecikme göstergesi.
- Her yeni özellik yeni version + Policy Gate Checklist yeniden.

## Kapsam Dışı (bu planda YOK — ileride tekrar sorulmasın diye)

- **Dublaj/audio extension'da ses çalma** — ayrı policy gate, ayrı experimental branch
  (policy §Audio ve Dublaj). Subtitles-only ilk release kesin karar.
- **Bits, UGC, identity link, reklam/sponsor, viewer CTA** — policy kesin yasakları.
- **Chrome Extension (Kick/evrensel, roadmap 2.5)** — A2'deki platform-bağımsız binding
  modeli onu hazırlar ama implementasyonu ayrı plan.
- **Twitch PubSub ile caption taşıma** — 1 msg/sn + 5 KB limiti altyazı temposuna yetmez
  (policy kararı), hiçbir aşamada kullanılmaz.
- **Extension'dan Twitch API çağrıları / EventSub** — ilk release'in ihtiyacı yok.

## Riskler

| Risk | Etki | Önlem |
|---|---|---|
| Hosted Test'te `wss://` CSP sorunu | Transport değişir | B2 poll fallback ilk günden kodda (gate tasarımı) |
| Review reddi | 2–4 hafta gecikme | Policy checklist B5 sonunda tam geçiş; subtitles-only dar kapsam |
| Viewer fan-out maliyeti | Backend yükü/bandwidth | A1 cache+limit; ayrı `ext.strovak.com` host'unda izole ölçüm |
| Faz 1'e sızma | CLAUDE.md ihlali | Bölge A yalnız genel backend sertleştirmesi; extension kodu gate arkasında |

## Roadmap Eşlemesi

- Roadmap 2.4 "Component overlay extension" → B1–B5
- Roadmap 2.4 "İzleyici dil seçim butonu" → B3
- Roadmap 2.4 "Twitch review başvurusu (erken başla)" → B6
- A1–A5 roadmap'e "Faz 2 hazırlık (Faz 1 uyumlu)" alt maddeleri olarak eklenebilir —
  kullanıcı onayıyla.
