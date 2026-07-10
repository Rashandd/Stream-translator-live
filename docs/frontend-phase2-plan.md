# StreamDubber Frontend — Faz 2 Uygulama Planı

Durum: Uygulamaya hazır plan; bu doküman kod değişikliği yapmaz

Tarih: 10 Temmuz 2026

Hedef kitle: Yeni Codex/Claude ajanları ve entegrasyon sorumlusu

Ana kaynaklar: `AGENTS.md`, `DESIGN.md`, `docs/frontend-config.md`,
`docs/multistream.md`, `docs/audio-pipeline.md`, `docs/security.md`,
`docs/websocket-protocol.md`, `streamdubber_roadmap_final.md`

## 1. Amaç

Bu plan, mevcut Next.js frontend denetiminde bulunan güvenlik, veri bütünlüğü,
runtime config, yayın kontrolü, Faz 2 kapsamı, erişilebilirlik ve test eksiklerini
tek sorumluluklu iş paketlerine böler.

Planın ilk ürün dili görevi, mutlak “video buluta çıkmaz” vaadini kaldırmaktır.
Bu değişiklik gerçek bir cloud video özelliği tamamlanmış gibi gösterilmemelidir.
Kullanıcıya gösterilecek doğru anlam seçilen moda bağlıdır:

- Yerel relay veya cihazda mux seçildiğinde video yayıncı bağlantısından platformlara gider.
- Faz 2 cloud mux seçildiğinde yalnız opt-in dublaj hedefinin video çıkışı buluta gider.
- Tüm yayını buluttan dağıtan MediaMTX/full cloud multistream Faz 3 kapsamındadır.
- Cloud mux gerçek media ingest, consent, retention, plan kontrolü ve kill switch
  tamamlanmadan aktif UI olarak gösterilmez.

Bu plan, “Faz 2’ye geçildi” kararını tam MediaMTX multistream’i Faz 2’ye çekme
yetkisi olarak yorumlamaz. Bu kapsam değişikliği ayrıca kullanıcı onayı gerektirir.

## 2. Mevcut Durum ve Kanıtlanmış Bulgular

Başlangıç frontend regresyonu:

- 40 test dosyası, 228 test başarılı.
- ESLint başarılı.
- Production build başarılı.
- `DESIGN.md` lint iki WCAG AA kontrast uyarısı veriyor.

Kritik bulgular:

1. `frontend/src/components/sdk-stream-key-panel.tsx`, `PUT /api/relay/targets`
   çağrısında yalnız `obs` kaydı gönderiyor. Backend `RelayTargetStore::set` listeyi
   tamamen değiştiriyor; mevcut Twitch/Kick ve `dub_language` hedefleri silinebilir.
2. `obs` yerel ingest credential’ı outbound relay target listesinde tutuluyor.
   SDK bu listeyi çıkış hedefi olarak çektiği için veri modeli kavramsal olarak yanlış.
3. Admin legal HTML girdisi sanitize edilmeden admin preview ve public legal sayfada
   `dangerouslySetInnerHTML` ile render ediliyor.
4. JWT’ler `localStorage` içinde. `AuthGuard` token doğrulamak yerine herhangi bir
   string varlığını yeterli kabul ediyor. Refresh-token akışı yok.
5. Admin login `role` ve `two_factor_required` sözleşmesini doğrulamıyor; admin verify
   sayfası gerçek API doğrulaması yapmıyor.
6. `frontend/src/lib/config.ts::getPublicConfig`, runtime config yerine yalnız statik
   `en.json` döndürüyor. Dashboard status/metric/nav ve public copy redeploy gerektiriyor.
7. Distribution seçimi yalnız component state’inde. Sayfa yenilenince kayboluyor ve
   Go Live sayfası statik `activeMode` okuyor.
8. `frontend/src/components/go-live-panel.tsx` içindeki Start düğmesinde handler yok.
9. Admin overview metrikleri, infrastructure kontrolleri ve bazı routing alanları
   çalışır kontrol görünümünde statik mock davranış sergiliyor.
10. 40 test dosyasının 27’si kaynak metinde string arıyor; DOM render veya E2E testi yok.

## 3. Çalışma Kuralları

Her ajan başlamadan önce:

```bash
git status --short
npm --prefix frontend run test
npm --prefix frontend run lint
npm --prefix frontend run build
```

Backend/SDK dosyası değişecekse ayrıca:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Kurallar:

- TDD zorunlu: red → green → refactor.
- Her iş paketi ayrı branch/commit sorumluluğudur.
- Başka ajanın değiştirdiği dosyaya aynı anda yazma.
- `backend/src/app.rs`, `frontend/src/lib/api.ts`, `en.json` ve roadmap ortak
  çakışma noktalarıdır; entegrasyon sorumlusu bu dosyaları sırayla birleştirir.
- Feature flag olmadan yarım özellik görünür hale getirilmez.
- Roadmap tiki yalnız gerçek davranış ve test tamamlandıktan sonra atılır.
- Session dosyası kullanılmaz; handoff git durumu, commit, plan paketi ve test
  çıktısından kurulur.
- Kullanıcı istemedikçe `git push` yapılmaz.

## 4. Uygulama Dalgaları ve Bağımlılık Grafiği

```text
P0 ürün sözleşmesi/copy ─────────────────────────────┐
P1 legal XSS ────────────────────────────────────────┤ bağımsız başlayabilir
P2 cookie session/auth ──────────────────────────────┤
P3 ingest credential + relay target ayrımı ─────────┘

P2 auth ───────────────► P4 runtime UI config
P3 relay ayrımı ───────► P5 kalıcı distribution settings
P4 config + P5 settings ► P6 Go Live control plane
P4 config ─────────────► P7 admin/capability yüzeyleri
P1-P7 ─────────────────► P8 DESIGN/accessibility
Her paket ─────────────► P9 davranışsal test/E2E ve rollout
```

Önerilen ajan dağılımı:

| Ajan | İlk paket | Dosya sahipliği |
|---|---|---|
| Agent A | P1 Legal XSS | legal backend modülü, legal frontend yüzeyleri |
| Agent B | P2 Auth | auth backend, BFF/session frontend |
| Agent C | P3 Relay ayrımı | relay backend, SDK ingest doğrulama, SDK paneli |
| Entegrasyon ajanı | P0, ardından P4-P9 koordinasyonu | ortak config, app route kayıtları, roadmap |

P1/P2/P3 paralel yürütülebilir; ancak `backend/src/app.rs` route kayıtları
entegrasyon ajanı tarafından tek tek alınmalıdır.

## 5. P0 — Faz 2 Ürün Sözleşmesi ve Copy Temizliği

### Hedef

Mutlak video-local iddiasını mode-dependent routing açıklamasıyla değiştirmek;
henüz çalışmayan cloud yeteneğini aktif ürün gibi göstermemek.

### Test-first

Önce aşağıdaki frontend sözleşme testlerini kırmızıya çevir:

- Public copy içinde koşulsuz `video never leaves`, `video hiç buluta çıkmaz`,
  `only audio goes to cloud`, `Strovak never receives it` kalmamalı.
- Security/How It Works metni hem local relay hem opt-in cloud yolunu açıklamalı.
- Cloud mux copy’si `feature_flag`, `consent` ve `selected output` kavramlarını taşımalı.
- Local relay alt ağacındaki mod-bazlı açıklamalar whitelist edilmelidir.

### Dosya kapsamı

- `AGENTS.md`, `CLAUDE.md`
- `DESIGN.md`
- `docs/audio-pipeline.md`, `docs/multistream.md`, `docs/security.md`
- `docs/twitch-extension-policy.md`, `docs/twitch-extension-plan.md`
- `infra/deploy.md`
- `sdk/ui/index.html`
- `sdk/src/core/cloud.rs`, `sdk/src/core/rtmp/mod.rs` dokümantasyon yorumları
- `frontend/src/i18n/dictionaries/en.json`
- ilgili dictionary testleri
- `streamdubber_roadmap_final.md`

### Copy matrisi

| Mod | Kullanıcıya söylenecek gerçek |
|---|---|
| Local relay | Video doğrudan yayıncı setup’ından platformlara gider; Strovak AI hattı speech/transcript işler. |
| Device mux | Dublaj sesi bu cihazda video ile birleştirilir. |
| Cloud dub mux | Açıkça seçilen dublaj hedefi cloud ingest’e gönderilir ve bulutta mux edilir. |
| Full cloud multistream | Faz 3; `disabled/coming_soon`, aktif Faz 2 seçeneği değil. |

### Kabul kriterleri

- Public pricing, security, solutions, SDK, FAQ ve legal metinlerinde mutlak iddia yok.
- Privacy metni cloud mux’ın amaç, veri yolu ve retention sınırını açıklar.
- Yerel yol teknik olarak doğru kalır.
- Cloud özelliği gerçek backend readiness olmadan enabled görünmez.
- `npm run test`, `npm run lint`, `npm run design:lint`, `npm run build` geçer.

## 6. P1 — Legal HTML Stored XSS Düzeltmesi

### Hedef

Admin kaynaklı legal HTML’in public sayfada script çalıştırmasını engellemek.

### Backend tasarımı

- Allowlist tabanlı HTML sanitizer ekle; regex ile HTML temizleme yapma.
- İzinli minimum etiketler: `h1-h3`, `p`, `ul`, `ol`, `li`, `strong`, `em`,
  güvenli `a`, gerekirse `blockquote`.
- `script`, `style`, `iframe`, `object`, `embed`, SVG, event handler ve
  `javascript:`/`data:` URL’lerini reddet veya temizle.
- Yazma sırasında sanitize et; eski kayıtları okurken de defense-in-depth uygula.
- Audit log old/new değer yerine güvenli hash/özet ve actor metadata tutsun;
  raw payload loglanmasın.

### Frontend tasarımı

- Admin preview sanitize edilmiş server sonucunu render etsin veya sandbox iframe kullansın.
- Public legal sayfa yalnız sanitize edilmiş alanı kabul etsin.
- CSP ekle. Inline theme script nonce/hash ile çalışsın.

### Testler

- `<script>`, `onerror`, `javascript:`, malformed HTML ve SVG payload çalışmaz.
- İzinli başlık/liste/güvenli link korunur.
- Stored payload public sayfada executable DOM oluşturmaz.
- CSP header beklenen `script-src`, `connect-src`, `frame-ancestors` sınırlarını taşır.

### Kabul kriteri

- Sanitizasyon hem write hem read sınırında testlidir.
- Public legal sayfada ham admin HTML’i doğrudan render edilmez.
- LocalStorage token varken dahi XSS ile okunabilecek script yolu kalmaz; P2
  tamamlandığında token zaten JS erişiminden çıkarılır.

## 7. P2 — Cookie Session, Refresh ve Gerçek 2FA

### Hedef

`localStorage` bearer token modelini HttpOnly cookie/BFF session modeline taşımak.

### Önerilen sözleşme

```text
POST /api/auth/login
POST /api/auth/2fa
GET  /api/auth/session
POST /api/auth/refresh
POST /api/auth/logout
POST /api/auth/oauth/exchange
```

Kurallar:

- Access token kısa ömürlü.
- Refresh token hashlenmiş, rotasyonlu, revoke edilebilir.
- Cookie: `HttpOnly`, `Secure`, uygun `SameSite`, dar `Path`.
- Mutating BFF isteklerinde Origin ve CSRF kontrolü.
- OAuth callback access tokenı URL fragment’ına koymaz; tek kullanımlık code exchange kullanır.
- `AuthGuard` canlı session ve role doğrular.
- Admin login `role` ve `two_factor_required` alanlarını doğrular.
- Admin verify gerçek ortak 2FA bileşenini kullanır.
- Geçiş sırasında eski localStorage tokenları bir kez temizlenir.

### Testler

- Token `document.cookie` veya localStorage’dan okunamaz.
- Yanlış role admin paneline giremez.
- Expired access token bir kez refresh edilir; tekrar başarısızsa logout olur.
- Refresh replay reddedilir.
- Logout refresh sessionı revoke eder.
- 2FA bekleyen yanıt admin tokenı gibi saklanmaz.
- OAuth URL/fragment içinde access token bulunmaz.

### Kabul kriteri

- `frontend/src/lib/api.ts` merkezi 401/refresh davranışı sağlar.
- Her panelin ayrı unauthorized state üretmesine gerek kalmaz.
- Browser reload sonrası session korunur, expiry sonrası güvenli şekilde kapanır.

## 8. P3 — OBS Ingest Credential ve Relay Target Ayrımı

### Hedef

Yerel OBS publish credential’ını outbound relay hedeflerinden ayırmak ve target
listesi veri kaybını kapatmak.

### Yeni API sözleşmesi

```text
GET  /api/sdk/ingest-credential
POST /api/sdk/ingest-credential/rotate

GET    /api/relay/targets
POST   /api/relay/targets
PATCH  /api/relay/targets/:target_id
DELETE /api/relay/targets/:target_id
```

### Veri modeli

- `IngestCredential`: user id, encrypted key, version, created/rotated timestamp.
- `RelayTarget`: kararlı `target_id`, platform, RTMP(S) URL, encrypted stream key,
  `dub_language`, enabled, revision.
- Yerel ingest key outbound hedef değildir.
- Target PATCH tek hedefi değiştirir ve optimistic concurrency/revision kullanır.

### Migration

1. Şifreli relay store yedeği al.
2. Eski `platform=obs` kaydından ingest credential seed et.
3. Seed doğrulanmadan eski kaydı silme.
4. Targetlara ID/revision ekle.
5. Dual-read süresinden sonra eski full-list PUT’u kapat.

### SDK

- Beklenen ingest credential’ı güvenli şekilde çek.
- RTMP `publish` key’ini doğrula; yanlış key’i reddet.
- Rotate sonrası güvenli reload/yeniden bağlanma uygula.
- Stream key veya credential loglama.

### TDD

- İki target ve bir `dub_language` varken ingest rotate targetları byte-for-byte korur.
- Tek target PATCH diğer targetları değiştirmez.
- Yanlış OBS publish key reddedilir.
- Kullanıcılar birbirinin credential/target kayıtlarına erişemez.
- Eşzamanlı revision çakışması kayıp güncelleme yerine 409 döner.

### Kabul kriteri

- SDK sayfasını açmak veri mutasyonu yapmaz.
- Regenerate yalnız ingest credential’ı döndürür.
- Full-list replacement endpoint’i frontend tarafından kullanılmaz.

## 9. P4 — Gerçek Runtime UI Config

### Hedef

`docs/frontend-config.md` sözleşmesini uygulamak; `en.json`ı business source of
truth olmaktan çıkarmak.

### Veri ve API

```text
ui_config(key, locale, value_json, schema_version, is_public,
          revision, updated_at, updated_by)

GET   /api/public-config?locale=en
GET   /api/me/config
GET   /api/admin/config
PATCH /api/admin/config/:key
```

Kurallar:

- Public endpoint yalnız `is_public=true` alanları döndürür.
- `/api/me/config`; plan, feature flags, SDK capability, integration ve canlı
  kullanıcı durumunu birleştirir.
- Secret, stream key veya admin operational değer public/me payload’a girmez.
- PATCH key’e özel JSON schema ile doğrulanır.
- Her yazı audit kaydı üretir.
- Revision/ETag ve kısa TTL uygulanır.
- Eski `/config` endpoint’i public allowlist ile daraltılarak kaldırılır.

### Migration sırası

1. Brand/nav/feature flag/security claims.
2. Public home/solutions/pricing copy.
3. SDK downloads/platform catalog.
4. Dashboard card definitions ve navigation.
5. Admin editable metadata.

`en.json` yalnız locale/schema fallback’i olur; fiyat, limit, provider, platform,
feature availability veya marketing claim source of truth olmaz.

### Testler

- Admin copy değiştirir; redeploy olmadan public render değişir.
- Private key public yanıta sızmaz.
- Restart sonrası config korunur.
- ETag unchanged durumda 304, publish sonrası yeni revision döner.
- API yoksa kullanıcı açık stale/fallback state görür; sessizce eski fiyat gösterilmez.

## 10. P5 — Kalıcı Distribution ve Mux Ayarları

### Hedef

Component-local seçimi kullanıcıya ait kalıcı, backend-enforced ayara dönüştürmek.

### Faz 2 model

```json
{
  "distribution_method": "sdk_relay | streamlabs",
  "dub_mux_mode": "device | cloud",
  "version": 1,
  "cloud_video_consent_version": null,
  "cloud_video_consent_at": null
}
```

Faz 2’de `full_cloud_multistream` bu enum’a eklenmez.

### Endpoint

```text
GET /api/distribution/settings
PUT /api/distribution/settings
```

Yanıt ayrıca şunları verir:

- plan izinleri,
- provider/integration bağlantı durumu,
- SDK capability,
- cloud mux readiness,
- upload uyarı eşiği,
- feature flags,
- consent metni ve güncel consent version.

Config anahtarları:

```text
dist.relay.plans
dist.streamlabs.plans
dist.dub_cloud.enabled
dist.dub_cloud.plans
dist.upload_warning_mbps
dist.cloud_consent.version
```

### Güvenlik

- Plan/readiness yalnız UI’da değil backend’de enforce edilir.
- Cloud mux seçimi consent version olmadan reddedilir.
- UI seçimi video ingest’i kendiliğinden başlatmaz.
- Kill switch cloud seçimlerini güvenli biçimde device mux’a düşürür.
- Mod değişiklikleri audit log’a yazılır.

### Kabul kriteri

- Refresh sonrası seçim korunur.
- Save hatasında optimistic UI eski değere döner.
- Upload uyarı değeri config’ten gelir.
- Cloud seçimi hedefi ve maliyeti isimlendiren confirmation olmadan kaydedilemez.

## 11. P6 — Gerçek Go Live Control Plane

### Hedef

Dekoratif Start düğmesini idempotent stream lifecycle ile değiştirmek.

### Önerilen endpointler

```text
GET    /api/streams/preflight
GET    /api/streams/current
POST   /api/streams
DELETE /api/streams/:id
POST   /api/sdk/commands/:id/ack
POST   /api/sdk/stream-status
```

State modeli:

```text
idle → preflight → starting → live → degraded → stopping → stopped
                              └───────────────► failed
```

Kurallar:

- `POST /api/streams` idempotency key kullanır ve `202 starting` döner.
- SDK komutu ACK gelene kadar tekrar teslim edilebilir; poll edildiği anda silinmez.
- Genel command loop, local STT/TTS motoru yokken de start/stop işler.
- SDK observed state raporlar; dashboard yalnız desired state göstermemeli.
- Local moddaki Start yalnız translation/dubbing sessionı yönetir; OBS videosunu
  başlattığını iddia etmez.
- Cloud mux worker hazır değilse cloud Start disabled kalır.

Preflight reason code’ları:

- `sdk_offline`
- `no_target_language`
- `no_relay_target`
- `provider_disabled`
- `voice_model_missing`
- `plan_locked`
- `cloud_mux_unavailable`
- `cloud_consent_required`

### Testler

- Double click tek stream session oluşturur.
- Offline SDK anlamlı reason code verir.
- Start → SDK ACK → live geçişi UI’da görünür.
- Timeout/degraded/stop doğru render edilir.
- Statik overlay URL, stream key veya target toggle kalmaz.
- Stop başka kullanıcının streamini etkileyemez.

## 12. P7 — Faz 2 Admin ve Capability Yüzeyleri

### Hedef

Statik/handlersız admin kontrollerini gerçek operasyon yüzeylerine dönüştürmek.

İşler:

- Provider listesi canlı registry’den gelir.
- Hazır olmayan provider/motor seçilemez.
- SDK heartbeat capabilities taşır: local STT, local TTS, ffmpeg, device mux,
  cloud mux uplink ve minimum SDK version.
- Admin overview gerçek active stream, latency ve error rate endpointini kullanır.
- `/admin/multistream` canlı mode/worker/egress özetini gösterir.
- Infrastructure Start/Stop ve notification sender ya gerçek endpointle bağlanır
  ya açıkça read-only/disabled görünür.
- Statik history yerine audit kayıtları gösterilir.
- Plan fiyatı, limit ve feature yönetimi gerçek admin sayfasına alınır.

Önerilen endpointler:

```text
GET /admin/providers
GET /admin/dashboard/summary
GET /admin/multistream/summary
GET /admin/audit?resource=config
GET/PATCH /admin/plans/:id
```

Kabul:

- Handler olmayan kontrol kalmaz.
- Mock metric ancak açık dev fixture etiketiyle görünür.
- Provider toggle restart olmadan pipeline ve UI’a yansır.
- Plan değişikliği public pricing ve user entitlement’a aynı revision ile yansır.

## 13. P8 — DESIGN.md, Responsive ve Accessibility

İşler:

- Inter ve Geist Mono gerçekten `next/font` veya self-hosted asset ile yüklenir.
- Tüm touch targetlar en az 44×44 px olur.
- Public, dashboard ve admin header 360 px’de taşmaz.
- Dashboard nav mobil drawer/topbar modeline geçer.
- Active language chip ve danger status WCAG AA kontrastına ulaşır.
- Legal dark mode tokenları hard-coded light renk kullanmaz.
- Landing hero dekoratif orb yerine gerçek pipeline/product state gösterir.
- Panel radius, shadow ve nested-card kullanımı DESIGN tokenlarına döner.
- 360, 768 ve 1280 px’de yatay page overflow olmaz.
- Focus-visible, keyboard navigation ve reduced-motion kontrol edilir.

Kabul:

- Axe kritik/serious bulgu yok.
- Klavye ile tüm navigasyon ve dialog akışları tamamlanır.
- Cloud consent modal focus trap, Escape ve geri dönüş odağını doğru yönetir.
- Visual regression üç zorunlu viewportta geçer.

## 14. P9 — Davranışsal Test, E2E ve Rollout

### Test altyapısı

- Vitest `jsdom`.
- React Testing Library + user-event.
- MSW veya kontrollü fetch mockları.
- Playwright E2E.
- Mevcut source-string testleri aşamalı olarak davranış testleriyle değiştir.

Zorunlu E2E senaryoları:

1. Login → 2FA → dashboard → refresh → logout.
2. Admin config değiştirir; public sayfa redeploy olmadan değişir.
3. Distribution seçimi kaydedilir; Go Live aynı seçimi okur.
4. Ingest credential rotate relay targetları korur.
5. Legal XSS payload çalışmaz ve CSP violation kaydedilir.
6. Local relay stream preflight/start/stop.
7. Cloud mux flag kapalı, consent eksik ve worker unavailable durumları.
8. 360/768/1280 responsive smoke.

### Feature flags

```text
ui.runtime_config_v2
auth.cookie_sessions
relay.ingest_credentials_v2
dist.settings_v2
streams.control_plane
dist.dub_cloud
```

Rollout sırası:

1. Internal admin.
2. Founding/beta kullanıcıları.
3. Yüzde bazlı rollout.
4. Genel release.

Cloud video flag’i şu kapılar geçmeden açılamaz:

- RTMPS ingest ve kısa yaşamlı credential,
- target-bazlı consent,
- retention default off,
- plan/credit enforcement,
- worker health/readiness,
- kill switch,
- privacy/legal copy,
- audit ve kapasite metrikleri.

İzlenecek metrikler:

- 401/refresh failure rate,
- config fallback/stale oranı,
- relay target count mutation,
- credential rotation failure,
- SDK command ACK latency,
- stream start failure reason,
- cloud ingest bitrate/egress,
- CSP violation sayısı,
- frontend error boundary oranı.

## 15. Commit ve Handoff Sözleşmesi

Önerilen commit sırası:

```text
test: lock phase 2 routing copy contract
chore: align phase 2 routing documentation
fix: sanitize legal html before rendering
feat: add cookie-backed browser sessions
fix: separate sdk ingest credentials from relay targets
feat: load ui from runtime config
feat: persist distribution and mux settings
feat: add stream control plane
feat: connect phase 2 admin capabilities
fix: align frontend with responsive accessibility rules
test: add phase 2 browser journeys
```

Her ajan final handoff’ta şunları verir:

- Tamamlanan plan paketi.
- Değişen dosyalar ve migrationlar.
- Verilen önemli kararlar.
- Çalıştırılan testler ve sonuçları.
- Canlı hesap/donanım/sertifika gerektiren doğrulanmamış adımlar.
- Sonraki bağımlı paket.
- Bloker ve rollback bilgisi.

## 16. Definition of Done

Faz 2 frontend sağlamlaştırması ancak aşağıdakilerin tamamı gerçek olduğunda biter:

- Relay/ingest ayrımı veri kaybı olmadan migration edilmiştir.
- Stored XSS kapalı, CSP aktiftir.
- Browser session tokenı JavaScript tarafından okunamaz; refresh ve admin 2FA çalışır.
- Public/dashboard/admin business içeriği runtime config ve canlı veriden gelir.
- Distribution seçimi kalıcı ve backend-enforced’dır.
- Go Live gerçek desired/observed lifecycle yönetir.
- Cloud mux yalnız seçili hedef, explicit consent ve feature flag ile çalışır.
- Full cloud multistream Faz 3 olarak ayrı kalır.
- Admin mock/handlersız kontrolleri kaldırılmış veya açıkça disabled’dır.
- DESIGN.md responsive/accessibility kriterleri üç viewportta geçer.
- Kritik yol coverage en az %80; zorunlu Playwright senaryoları yeşildir.
- Roadmap yalnız tamamlanan maddeler için güncellenmiş ve testler temizken commitlenmiştir.

## 17. Yeni Ajan Başlangıç Promptları

Bu promptlar ayrı ajanlara doğrudan verilebilir. Her ajan önce bu planın tamamını
okumalı, yalnız kendisine atanan pakette yazma yapmalı ve ortak dosya değişikliğini
entegrasyon ajanına bildirmelidir.

### Agent A — Legal XSS

```text
docs/frontend-phase2-plan.md dosyasını tamamen oku ve yalnız P1 paketini uygula.
Önce backend/frontend regresyonlarını çalıştır. TDD ile legal HTML sanitizer,
güvenli preview ve CSP ekle. Auth, runtime config, relay veya roadmap işlerine girme.
backend/src/app.rs route değişikliğini ayrı hunk/commit olarak belirt. Final handoff'ta
payload testleri, CSP sonucu, migration ve kalan riskleri yaz.
```

### Agent B — Cookie Session ve 2FA

```text
docs/frontend-phase2-plan.md dosyasını tamamen oku ve yalnız P2 paketini uygula.
LocalStorage JWT modelini HttpOnly cookie + refresh rotation + gerçek role/2FA
doğrulamasına TDD ile taşı. Legal, relay, runtime config ve Go Live dosyalarına girme.
Eski token migration/cleanup ve OAuth one-time exchange testlerini unutma. Finalde
API sözleşmesi, CSRF/Origin kararı, rollback ve test sonuçlarını ver.
```

### Agent C — Ingest/Relay Veri Bütünlüğü

```text
docs/frontend-phase2-plan.md dosyasını tamamen oku ve yalnız P3 paketini uygula.
OBS ingest credential'ını outbound relay targetlardan ayır; ID-bazlı target CRUD ve
safe migration ekle. Mevcut target/dub_language değerlerinin rotate sırasında aynen
korunduğunu önce başarısız testle kanıtla. Cloud mux veya Go Live implement etme.
Finalde store backup/migration, concurrency, log redaction ve test sonuçlarını ver.
```

### Entegrasyon Ajanı

```text
docs/frontend-phase2-plan.md dosyasını tamamen oku. P1/P2/P3 ajanlarının handoff ve
commitlerini incele; ortak backend/src/app.rs değişikliklerini çakışmasız birleştir.
Sonra bağımlılık grafiğine göre P0 ve P4'ü planla. Hiçbir roadmap tikini gerçek davranış,
regression, lint ve build temiz olmadan işaretleme. Faz 3 full cloud multistream'i Faz 2
cloud dub mux ile birleştirme.
```
