# StreamDubber Backend Production Hardening — Yeni Ajan Uygulama Planı

> Durum: Uygulama bekliyor
>
> Hazırlanma tarihi: 2026-07-10
>
> Hedef okuyucu: Projeyi daha önce görmemiş uygulama ajanı
>
> Kapsam: Backend analizinde bulunan yüksek ve orta seviye sorunlar
>
> Repo kökü: `/home/adem/Projects/GitHub/Stream-translator-live`

## 1. Belgenin amacı

Bu belge, StreamDubber backend'ini tek-instance geliştirme/MVP yapısından güvenli,
gözlemlenebilir ve yatay ölçeklenebilir production mimarisine taşımak için eksiksiz
bir uygulama planıdır. Yeni ajan bu belge dışında eski oturum bağlamına ihtiyaç
duymadan çalışabilmelidir.

Plan özellikle şunları kapsar:

- PostgreSQL ve Redis için net veri sahipliği
- Bellek ve dosya tabanlı state'in production yolundan çıkarılması
- Public API ile SDK mTLS trafiğinin ayrılması
- Güvenilir client IP ve dağıtık rate-limit
- OAuth, overlay ve WebSocket kaynak tüketiminin sınırlandırılması
- Production yapılandırmasının fail-closed olması
- JWT refresh, iptal ve canlı entitlement kontrolü
- Provider timeout, WebSocket hata sözleşmesi ve request middleware
- Liveness/readiness ve operasyonel sağlık
- Hesap silme, voice-data temizliği, consent ve audit log
- Faz dışı yüzeylerin feature gate arkasına alınması
- Senkron dosya I/O, log büyümesi ve production panic noktalarının temizlenmesi
- CI format, DB/Redis integration, dependency audit ve coverage kapıları

Bu çalışma yeni ürün özelliği geliştirme yetkisi vermez. Roadmap aktif fazı Faz 2
olarak gösterse bile repo kökündeki `AGENTS.md` kuralları ve kullanıcı tarafından
verilen görev sınırı uygulanır.

## 2. Kesin kapsam sınırı

### 2.1 Bu planın içinde

- Önceki backend analizindeki tüm yüksek seviye bulgular
- Önceki backend analizindeki tüm orta seviye bulgular
- Bu bulguların gerektirdiği migration, test, dokümantasyon ve CI işleri
- Geriye uyumlu ve kontrollü veri cutover araçları
- Güvenlik değişiklikleri için frontend/SDK'da zorunlu olan minimal sözleşme
  güncellemeleri

### 2.2 Bu planın dışında kalan P0 işler

Aşağıdaki dört kritik konu ayrı görev/PR olarak ele alınmalıdır:

1. Backend Docker image'ın workspace üyesi eksikliği nedeniyle derlenememesi
2. Deepgram için gerçek streaming STT WebSocket pipeline'ı
3. Usage süresinin frame başına yuvarlanması ve yanlış kota hesabı
4. Stripe webhook'un gerçek kullanıcı planını yükseltmemesi/düşürmemesi

Bu plan bu alanlarla temas edebilir, fakat P0 davranışı aynı commit içinde
düzeltmeye çalışmamalıdır. Örnekler:

- Canlı entitlement katmanı DB'deki plan değişikliğini anında görmelidir; Stripe'ın
  o planı değiştirmesi ayrı P0 işidir.
- Provider timeout ve hata kodları düzeltilebilir; prerecorded STT'nin streaming'e
  dönüştürülmesi ayrı P0 işidir.
- Usage repository hazırlanabilir; süre hesaplama algoritması bu planın işi değildir.
- CI'a Docker build adımı ancak P0 Dockerfile düzeltildikten sonra zorunlu kapı yapılır.

### 2.3 Değiştirilemez güvenlik kuralları

- Ses akışına TLS 1.3 üstüne AES ekleme. Transport için TLS/mTLS kullanılacak.
- AES-256-GCM yalnız at-rest veriler için kullanılacak.
- Secret, JWT, API key, stream key veya voice sample loglanmayacak.
- Production'da eksik güvenlik ayarı için sessiz fallback yapılmayacak.
- Migration sırasında legacy dosyalar doğrulama tamamlanmadan silinmeyecek.
- Kullanıcı açıkça istemedikçe `git push` çalıştırılmayacak.
- İlgisiz çalışma ağacı değişiklikleri commit'e alınmayacak.

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

### 3.2 Başlamadan önce okunacak belgeler

Zorunlu:

1. `AGENTS.md`
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

Bu bilgiler başlangıç varsayımıdır. Yeni ajan ilk turda tekrar doğrulamalıdır.

## 5. Bilinen risk özeti

### 5.1 Yüksek seviye riskler

1. Kalıcı state parçalıdır: PostgreSQL, memory ve şifreli/şifresiz dosya store'ları
   aynı anda kaynak gibi davranır.
2. Redis kullanılmadığı için rate-limit, OAuth state, komut kuyruğu ve active
   stream verisi instance-local'dır.
3. Mevcut mTLS doğrulaması aynı listener'daki bütün route'lara uygulanır; public
   API ile SDK trafiği birlikte güvenli biçimde sunulamaz.
4. Overlay ve OAuth state bilinmeyen/public girdilerle sınırsız büyüyebilir.
5. Client IP çözümlemesi istemcinin gönderdiği `X-Forwarded-For` değerine güvenir.
6. Mock provider, email 2FA ve TLS yapılandırmasında production fail-open yolları var.
7. JWT plan/rol claim'leri uzun süre stale kalabilir; refresh ve merkezi revoke
   sistemi yoktur.
8. WebSocket provider hataları protokolde tanımlı `4005/4006` kodlarıyla
   iletilmemekte veya sessizce yutulmaktadır.
9. Hesap silme, voice-data cleanup, key-use audit ve retention akışı eksiktir.

### 5.2 Orta seviye riskler

1. Public `/config` bütün internal config snapshot'ını döndürür.
2. Config ve plan güncellemelerinde semantik validation yoktur.
3. `/health` dependency durumundan bağımsız `ok` döndürür.
4. Global request ID, timeout, concurrency, load-shed ve body limit politikası yoktur.
5. Faz 1/2/3 yüzeyleri aynı runtime'da yeterince güçlü feature gate olmadan bulunur.
6. Production kodunda lock/system-time varsayımlarına dayalı `expect()` çağrıları vardır.
7. JSONL log dosyaları sınırsız büyür ve request path'te senkron yazılabilir.
8. Format, dependency audit, coverage ve gerçek DB/Redis testleri CI tarafından
   zorlanmaz.

## 6. Hedef mimari

### 6.1 Veri sahipliği

| Veri | Tek kaynak | Cache/dağıtım | Yasaklanan production davranışı |
|---|---|---|---|
| Users, role, plan | PostgreSQL | Kısa TTL Redis cache | File store'u source of truth yapmak |
| Plans ve service config | PostgreSQL | Redis invalidation/cache | Yalnız code default ile yaşamak |
| Subscriptions | PostgreSQL | Gerekiyorsa Redis | Webhook sonucu yalnız memory'de tutmak |
| SDK sessions/devices | PostgreSQL | Redis presence | Yalnız encrypted JSON kullanmak |
| OAuth/platform accounts | PostgreSQL, at-rest encrypted | Kısa TTL cache | Raw token dosyası |
| Relay/settings/bindings | PostgreSQL | Gerekiyorsa Redis | Instance-local snapshot |
| Usage ledger | PostgreSQL | Redis aggregation opsiyonel | Dosya JSONL'ı tek kaynak yapmak |
| OAuth CSRF state | Redis TTL | Yok | TTL'siz memory map |
| 2FA/reset/pairing code | Redis TTL | Yok | Multi-instance memory map |
| Rate-limit counters | Redis TTL | Yok | Process-local güvenlik limiti |
| Active stream | Redis TTL | Pub/Sub | Process-local map |
| Overlay snapshot | Redis bounded list | Pub/Sub | Sınırsız HashMap/VecDeque |
| Audit log | PostgreSQL/harici audit sink | Yok | Secret içeren JSONL |
| Operational log | Structured stdout/log backend | Bounded local ring | Sınırsız senkron dosya |

### 6.2 Ağ topolojisi

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

## 7. Genel uygulama yöntemi

Her iş paketinde şu sıra zorunludur:

1. Kapsamla ilgili referans belge ve kodu oku.
2. `git status` ve `git diff` ile başkalarının değişikliklerini belirle.
3. İlgili regresyon testlerini çalıştır.
4. Başarısız test ekle.
5. Minimal implementasyon yap.
6. Testi geçir.
7. Refactor et.
8. Migration/cutover etkisini doğrula.
9. İlgili dokümanı güncelle.
10. Backend test, Clippy ve fmt çalıştır.
11. Risk yüksekse workspace testlerini çalıştır.
12. Roadmap'te yalnız tamamlanan BH maddesini tikle.
13. Diff'i incele ve tek sorumluluklu commit oluştur.
14. Push yapma.

## 8. Roadmap'e eklenecek görevler

`streamdubber_roadmap_final.md` içine uygun bir “Backend Production Hardening”
başlığı altında şu checklist eklenmelidir:

```markdown
### Backend Production Hardening

- [ ] BH-01 Public config yüzeyini güvenli allowlist ile sınırla
- [ ] BH-02 Config ve plan güncellemelerine semantik validation ekle
- [ ] BH-03 PostgreSQL'i production state için tek kaynak yap
- [ ] BH-04 Redis tabanlı paylaşılan ephemeral state kur
- [ ] BH-05 Trusted proxy ve güvenilir client IP çözümlemesi ekle
- [ ] BH-06 Rate-limit, OAuth state, overlay ve WS kaynaklarını sınırla
- [ ] BH-07 Public API ile SDK mTLS listener'larını ayır
- [ ] BH-08 Production startup kontrollerini fail-closed yap
- [ ] BH-09 JWT yenileme, iptal ve canlı entitlement kontrolü ekle
- [ ] BH-10 Provider timeout ve WebSocket hata protokolünü düzelt
- [ ] BH-11 Liveness, readiness ve operasyonel sağlık kontrolleri ekle
- [ ] BH-12 Hesap silme, voice-data cleanup ve audit altyapısını tamamla
- [ ] BH-13 Faz dışı backend özelliklerini feature gate arkasına al
- [ ] BH-14 Senkron dosya I/O, log rotation ve production panic noktalarını temizle
- [ ] BH-15 CI'ya fmt, DB/Redis integration, audit ve coverage kapıları ekle
```

Tarihsel roadmap'te mTLS gibi tamamlanmış görünen maddeler doğrudan silinmemelidir.
BH maddeleri mevcut implementasyonun production sertleştirme/doğrulama katmanıdır.

## 9. Bağımlılık ve önerilen sıra

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

## 10. BH-01 — Public config sınırı

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

### TDD testleri

- Public response gerekli public alanları içerir.
- `providers.mock_fallback` public response'da yoktur.
- `auth.email_2fa` public response'da yoktur.
- Provider test/internal URL public response'da yoktur.
- Admin full config okuyabilir.
- Broadcaster admin config okuyamaz.
- Config'e yeni internal key eklemek onu otomatik public yapmaz.

### Kabul kriteri

Public alan eklemek için DTO'ya explicit kod değişikliği gerekmelidir. Internal map
snapshot'ı hiçbir public route'tan dönmemelidir.

### Önerilen commit

```text
fix: restrict public service config response
```

## 11. BH-02 — Typed config ve plan validation

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

## 12. BH-03 — PostgreSQL tek kaynak

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

## 13. BH-04 — Redis paylaşılan ephemeral state

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

## 14. BH-05 — Trusted proxy ve client identity

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

## 15. BH-06 — Bounded public/realtime kaynaklar

### 15.1 Overlay lifecycle

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

### 15.2 OAuth state

- Issue öncesi expired cleanup
- User/IP başına outstanding state limiti
- Global limit
- Public OAuth start endpoint'ine rate-limit
- Redis TTL/GETDEL kullanımı

### 15.3 WebSocket

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

## 16. BH-07 — Ayrı SDK mTLS listener

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

## 17. BH-08 — Production fail-closed startup

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

## 18. BH-09 — Refresh, revoke ve canlı entitlement

### Amaç

Uzun ömürlü stateless tokenların plan, rol, parola veya device değişikliklerinden
sonra geçerli kalmasını engellemek.

### Schema

Migration ile en az:

- `users.token_version`
- `auth_tokens.jti`
- `auth_tokens.family_id`
- `auth_tokens.revoked_at`
- `auth_tokens.last_used_at`
- `auth_tokens.sdk_session_id`
- gerekli unique/index'ler

### Kullanıcı token modeli

- Access token kısa ömürlü; hedef yaklaşık 15 dakika.
- Refresh token yüksek entropili.
- DB'de yalnız SHA-256 hash tutulur.
- Her refresh rotate edilir.
- Eski refresh reuse edilirse bütün token family revoke edilir.
- Logout current family'yi revoke eder.

### Authorization kaynağı

- JWT plan claim'i authorization kaynağı olmayacak.
- Plan ve rol PostgreSQL/çok kısa Redis cache'ten çözülecek.
- Admin plan/role değişikliği cache invalidation üretecek.
- Cache/DB arızasında privileged access fail-closed olacak.

### SDK token modeli

- JWT içinde `sdk_session_id`/`sid` ve `jti`.
- Her SDK auth'ta session active kontrolü.
- Device removal session ve ilgili tokenları revoke eder.
- Aktif WS gerekiyorsa Redis revoke event'iyle kapatılır.

### Güvenlik olayları

- Password reset: `token_version++`, tüm family revoke.
- Role downgrade: eski admin erişimi anında kapanır.
- Account deletion request: tüm token/device revoke.
- Device removal: yalnız ilgili cihaz tokenları revoke.

### Testler

- Plan değişikliği eski access tokenla yeni entitlement verir.
- Role downgrade eski admin tokenını engeller.
- Password reset eski tokenları geçersiz kılar.
- Refresh token iki kez kullanılamaz.
- Reuse detection bütün family'yi revoke eder.
- Device removal ilgili SDK JWT'yi engeller.
- İki cihaz birbirinden izole kalır.
- Raw token DB/logda yoktur.
- DB/cache failure güvenli davranır.

### P0 sınırı

Stripe'ın planı değiştirmesi bu görevin işi değildir. Bu görev DB plan değiştiğinde
auth katmanının bunu anında görmesini sağlar.

### Önerilen commitler

```text
feat: add rotating refresh token sessions
fix: resolve authorization from live user state
fix: revoke user and device tokens on security events
```

## 19. BH-10 — Provider resilience ve WebSocket hata sözleşmesi

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

## 20. BH-11 — Liveness, readiness ve health

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

## 21. BH-12 — Account deletion, audit ve retention

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

- Terms version
- Privacy version
- Server timestamp
- Marketing consent ayrı ve opsiyonel
- Marketing ertelendiyse alanı sessiz yutma; API'den kaldır veya consent ledger'a yaz

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
feat: persist versioned legal consent
```

## 22. BH-13 — Faz dışı özellikler için gate

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

## 23. BH-14 — Async I/O, log ve panic temizliği

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

## 24. BH-15 — CI kalite ve güvenlik kapıları

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

## 25. Global test stratejisi

### 25.1 Her küçük commit öncesi

```bash
cargo test -p streamdubber-backend <ilgili_test_filtresi>
cargo fmt -p streamdubber-backend -- --check
```

### 25.2 Her BH iş paketi sonunda

```bash
cargo test -p streamdubber-backend
cargo clippy -p streamdubber-backend --all-targets --all-features -- -D warnings
cargo fmt -p streamdubber-backend -- --check
git diff --check
```

### 25.3 Yüksek riskli paket sonunda

BH-03, BH-04, BH-07, BH-08, BH-09, BH-12 ve BH-15 için:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

Ayrıca gerçek PostgreSQL/Redis integration testleri ve ilgili TLS smoke çalıştırılır.

### 25.4 Güvenlik negatif testleri

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

## 26. Migration ve rollout güvenliği

### 26.1 Genel kural

Schema migration, data migration ve runtime cutover aynı deploy içinde kontrolsüz
birleştirilmemelidir.

Önerilen expand/migrate/contract akışı:

1. Expand: yeni tablo/kolon, eski kod hâlâ çalışır.
2. Migrate: legacy veriyi idempotent taşı.
3. Verify: sayım, constraint ve örnek doğrulama.
4. Cutover: read source'u DB/Redis yap.
5. Observe: log/metrik hata oranını izle.
6. Contract: eski write/file yolunu kaldır.

### 26.2 Rollback

- Forward-only migration geri alınmaz; yeni corrective migration yazılır.
- Cutover öncesi DB backup ve legacy file backup alınır.
- Legacy dosya verification tamamlanmadan silinmez.
- Dual-write yalnız sınırlı migration penceresi içindir; kalıcı mimari değildir.
- Security schema değişikliklerinde eski binary'nin yeni schema ile davranışı test edilir.

## 27. Commit ve PR sınırları

### Tek commit'e konmayacak kombinasyonlar

- PostgreSQL cutover + Redis migration
- mTLS listener + JWT refresh sistemi
- Config validation + bütün config değerlerinin değişmesi
- Account deletion + unrelated voice feature
- Format-all + functional değişiklik
- CI toolchain bump + dependency upgrade + feature refactor

### Prefix

Repo kuralına göre yalnız uygun prefix kullan:

- `feat:`
- `fix:`
- `chore:`
- `test:`

Doküman-only commit için:

```text
chore: document backend production hardening plan
```

## 28. Her iş paketi için Definition of Done

Bir BH maddesi ancak aşağıdaki şartların tamamında tamamlanır:

- Başlangıç regresyonu kaydedildi.
- Önce başarısız olan test eklendi.
- Yeni testler geçti.
- Backend regresyonu geçti.
- Clippy warnings-as-errors temiz.
- Format kontrolü temiz.
- Migration varsa gerçek PostgreSQL'de çalıştı.
- Redis davranışı varsa gerçek Redis'te çalıştı.
- Security değişikliği fail-open yol bırakmıyor.
- Secret/PII loglanmıyor.
- Cross-user ve multi-instance testleri var.
- `.env.example`, deploy ve protokol belgeleri güncel.
- Roadmap'te yalnız ilgili madde tiklendi.
- `git status`, `git diff` ve `git diff --cached` incelendi.
- İlgisiz dosya commit'e alınmadı.
- Tek sorumluluklu commit oluşturuldu.
- Kullanıcı istemediyse push yapılmadı.

## 29. Yeni ajan ilk çalışma turu

Yeni ajan ilk turda implementasyona başlamadan şu çıktıyı üretmelidir:

1. Okuduğu zorunlu belgeler
2. Güncel `git status`
3. Başlangıç test/Clippy/fmt sonucu
4. Seçtiği tek BH iş paketi
5. Bu paketin P0 işlerle çakışma kontrolü
6. Eklenecek ilk failing test
7. Beklenen dosyalar
8. Migration/rollout riski
9. Tahmini commit sınırları

İlk önerilen functional iş BH-01'dir. BH-03 gibi büyük veri refactoruyla doğrudan
başlanmamalıdır. Format borcu functional diff'i okunamaz yapıyorsa önce yalnız
format için mekanik commit hazırlanabilir.

## 30. Handoff şablonu

Bir ajan işi yarıda bırakır veya başka ajana devrederse final mesajında:

```markdown
## Backend hardening handoff

- Aktif görev: BH-XX — görev adı
- Son tamamlanan commit: <hash> <mesaj>
- Çalışma ağacı: temiz/kirli ve ilgili dosyalar
- Eklenen migration: <dosya veya yok>
- Geçen testler: <komutlar>
- Başarısız/çalıştırılamayan testler: <neden>
- Alınan mimari karar: <kısa özet>
- Sonraki somut adım: <tek adım>
- Bloker: <varsa>
- Dokunulmaması gereken kullanıcı/başka ajan değişiklikleri: <dosyalar>
```

## 31. Nihai başarı ölçütü

Bu plan tamamlandığında backend:

- İki veya daha fazla instance ile aynı kalıcı/ephemeral state'i paylaşabilmeli.
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

Bu ölçütler sağlanmadan roadmap'teki “Backend Production Hardening” bölümü tamamlandı
sayılmamalıdır.
