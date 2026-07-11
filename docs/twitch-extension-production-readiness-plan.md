# Twitch Extension — Production Readiness Plan

> **Durum:** Planlandı — implementasyon başlamadı
> **Kapsam:** StreamDubber Live Captions Twitch Extension'ın gerçek SDK sesi → STT →
> çeviri → izleyici altyazısı akışını güvenli, ölçeklenebilir ve review'a hazır hâle
> getirmek.
> **Bağlı planlar:** `docs/backend-hardening-captions-plan.md`,
> `docs/twitch-extension-plan.md`, `docs/twitch-extension-policy.md`
> **İlk release sınırı:** subtitles-only. Dublaj sesi, Bits, UGC, identity link ve
> viewer satın alma akışı bu planın kapsamı dışındadır.

## 1. Karar ve başarı tanımı

Extension worker'a doğrudan bağlanmaz. Yayıncı SDK'sı sesi backend'e yollar; backend
STT ve çeviriyi üretir; Extension Backend Service (EBS) yalnız oluşmuş `subtitle`
mesajlarını kanal bazında viewer'lara taşır.

```text
SDK audio → SDK/WebSocket listener → utterance assembler → STT worker/provider
          → translation provider → Subtitle → distributed caption hub → Twitch EBS
          → viewer overlay
```

Bu plan tamamlandığında aşağıdaki koşulların **tamamı** sağlanmadan Extension
"production ready" sayılmaz:

1. Gerçek VAD-sona-ermiş bir utterance, STT ve çeviri sonrasında viewer overlay'de
   anlamlı altyazı olarak görünür; 20 ms frame'ler ayrı STT isteği olmaz.
2. Yayıncı kendi kullanıcı oturumuyla kanal bağlayabilir; admin hesabı yayıncı
   bağlamasında araya girmez.
3. İzleyici, Extension'ı SDK başlamadan açsa da yayın başlayınca yenilemeden canlıya
   geçer; yayın durup yeniden başladığında eski caption kanalında takılmaz.
4. İki veya daha fazla backend replica'sı aynı active-stream, snapshot ve caption
   fan-out durumunu paylaşır.
5. Worker arızası, provider timeout'u, backend restart'ı ve EBS WebSocket kopması
   kontrollü hata/fallback üretir; sessizce caption kaybı yaşanmaz.
6. Gerçek GPU worker + gerçek çeviri sağlayıcısı ile staging kanıtı, Twitch Hosted
   Test ve Twitch review için canlı review senaryosu vardır.
7. Global ve kanal bazlı kill switch, ölçümler, alarm eşiği ve rollback prosedürü
   hazırdır.

Bu doküman, mevcut hardening planının faz sırasını değiştirmez. Özellikle
`BH-10 → A2/A3` sırası ve Redis/state işleri için `BH-03/04/06` önkoşulları
zorunludur.

## 2. Bugünkü boşluklar

| Alan | Bugünkü durum | Production gate |
|---|---|---|
| Utterance | SDK 20 ms voiced frame gönderiyor; backend frame başına STT çağırıyor | `A1/A2/A3` tamamlanır |
| Yayıncı bağlama | Dashboard kartı admin token kullanıyor | broadcaster `userFetch` akışına geçer |
| Viewer yaşam döngüsü | Offline → live ve yeni session geçişi yenileme olmadan çalışmıyor | controller testleriyle düzelir |
| Worker yönlendirme | En yeni `Ready` worker tek başına seçiliyor | health-aware pool + failover hazır olur |
| Çoklu backend | `OverlayHub` ve `ActiveStreams` process-local | Redis/Postgres ortak state'e taşınır |
| Provider hatası | Timeout/error sözleşmesi eksik; hata sessizce atlanabiliyor | `BH-10` error contract uygulanır |
| Doğrulama | E2E testler mock STT/çeviri kullanıyor | gerçek GPU + gerçek çeviri staging kanıtı alınır |
| Yayınlama | EBS kapalı; DNS, secret, Hosted Test ve review eksik | rollout kapıları geçilir |

## 3. Değişmez kurallar

- Video buluta gitmez; SDK'dan yalnız ses gider. Transit için TLS 1.3 kullanılır,
  ayrıca AES katmanı eklenmez.
- Extension viewer'a overlay capability token, provider key veya StreamDubber JWT
  verilmez.
- `TWITCH_EXT_SECRET`, provider key'leri, worker token'ları ve mTLS materyali
  Vault/env/ManagedSecrets'tan gelir; kaynak koda veya Twitch Configuration
  Service'e yazılmaz.
- Yeni ayar, limit, rollout ve capacity değeri `ServiceConfig`/plan verisiyle
  yönetilir; `BH-02` typed config doğrulamasından geçmeden yeni anahtar eklenmez.
- Production'da `providers.mock_fallback=false` olur. Eksik key sahte caption
  üretmek yerine görünür, ölçülebilir provider hatasıdır.
- İlk release subtitles-only kalır. Policy'deki viewer download, commerce, audio,
  Bits ve UGC yasakları release gate'tir.

## 4. İş paketleri ve kanonik merge sırası

Her maddede test önce yazılır, sonra implementasyon yapılır. Aşağıdaki Workstream
başlıkları sahiplik ve kabul kriteri içindir; **merge sırası tablodur**. Böylece bu
doküman, `backend-hardening-captions-plan.md` içindeki P0–P7 sırasını değiştirmez.

| Merge sırası | Hardening fazı | Açılabilecek işler |
|---|---|---|
| 1 | P0 | PR-01 typed config temelini, PR-04'ün A1 wire-formatını ve SDK capture bağımsızlığını hazırla |
| 2 | P1 | PR-01 feature gate'leri, PR-07 Redis/Postgres state temelini kur |
| 3 | P2 | PR-07 lifecycle/persistent token'ı ve PR-08 worker autoactivation/health'i tamamla |
| 4 | P3 | PR-03 SDK mTLS, fail-closed startup ve canlı entitlement'ı tamamla |
| 5 | P4 | PR-10/11/12 broadcaster-viewer doğruluğu; F1 EBS smoke; PR-13 review artefact taslağı |
| 6 | P5 | **Önce PR-02**, sonra PR-04/05 utterance işleri, ardından PR-06 gerçek provider smoke |
| 7 | P6 | PR-09 metrics/alert/load harness ve PR-13 log-redaction/CI kapıları |
| 8 | P7 | PR-14/15/16 staging, game day, Hosted Test, review ve kontrollü canary |

Aynı hot path'e dokunan iki büyük iş paralel merge edilmez. Özellikle `BH-10` bitmeden
SDK VAD-end ve backend assembler değişiklikleri merge edilmez.

### Workstream A — Sözleşme ve güvenli temel

**Bağımlılık:** Yok. `backend-hardening-captions-plan.md` Faz P0 ile birlikte yürür.

1. **PR-01 — Production config ve feature gate sözleşmesi**
   - `BH-01`, `BH-02` ve `BH-13` tamamlanır.
   - Extension için global gate, kanal bazlı etkin/pasif durum, caption TTL, viewer
     capacity, retry/backoff, rollout allowlist ve provider timeout ayarları typed
     config olarak tanımlanır.
   - Global gate kapalıyken EBS, worker background işi veya dağıtık state allocation
     yapılmaz. Kanal pause durumunda sadece o kanal güvenli `unavailable` yanıtı alır.
   - **Kanıt:** config validation, global/channel kill switch ve restart'sız değişim
     testleri.

2. **PR-02 — Provider error contract**
   - `BH-10` uygulanır: STT/çeviri HTTP çağrılarında bounded timeout, cancellation ve
     açık hata sınıfları olur.
   - STT primary/fallback tamamen başarısızsa SDK ve EBS'nin gözlemleyebileceği
     `4005`; çeviri politikasına göre `4006` veya kaynak caption'ın devam ettiği
     açık degraded durum üretilir. Sessiz frame drop yasaktır.
   - **Kanıt:** timeout, primary failure, fallback success/failure ve connection
     cleanup testleri; loglarda secret/transcript sızıntısı olmadığı doğrulanır.

3. **PR-03 — SDK listener güvenliği**
   - `BH-07`, `BH-08` ve `BH-09` tamamlanır: public EBS ile SDK listener ayrılır,
     SDK mTLS kullanır, production eksik security ayarında fail-closed başlar ve
     session entitlement/revocation canlı kontrol edilir.
   - **Kanıt:** sertifikasız SDK reddi, geçersiz/revoke token reddi, EBS'nin public
     TLS rotasından çalışması ve startup negative testleri.

### Workstream B — Anlamlı caption üretimi

**Bağımlılık:** PR-02. Bu faz tamamlanmadan gerçek STT worker veya Deepgram ile
caption kalitesi doğrulanmaz.

1. **PR-04 — Utterance wire formatı ve SDK VAD sinyali**
   - İki parçaya ayrılır: Hardening P0'da `A1` audio header byte 5'i geriye uyumlu
     `utterance_end` flag'i yapar; Hardening P5'te ve PR-02 sonrasında `A2`, SDK VAD
     hangover kapanışında kesin flush sinyali üretir.
   - Eski işaretsiz frame'ler desteklenir; yeni SDK'da cümle bitişi ek bekleme
     yaratmaz.
   - **Kanıt:** flag round-trip, konuşma→sessizlik→konuşma dizisi, boş flush frame ve
     eski SDK fallback testleri.

2. **PR-05 — Backend UtteranceAssembler**
   - Hardening planındaki `A3` uygulanır. Session başına bounded PCM buffer tutulur;
     `utterance_end`, azami utterance süresi veya eski istemci gap timeout'unda tek
     STT çağrısı yapılır.
   - Codec/format doğrulaması STT sağlayıcısına aktarılmadan önce tamamlanır. Self-hosted
     worker'ın beklediği 16 kHz mono PCM ile uyumsuz Opus frame ya decode edilir ya da
     `stream_start` aşamasında reddedilir; "kabul edip yanlış yorumlama" yasaktır.
   - `stream_stop`, disconnect ve plan-limit durumlarında buffer davranışı açıkça
     tanımlanır: final flush yalnız semantik olarak güvenliyse yapılır, aksi durumda
     metric/log ile atılır.
   - **Kanıt:** STT çağrı sayacıyla bir utterance→bir çağrı, max duration flush,
     gap fallback, bounded memory, provider error/close ve sequence testleri.

3. **PR-06 — Gerçek STT/translation staging smoke**
   - Docker/worker test ortamında gerçek Faster-Whisper engine ve gerçek DeepL anahtarı
     kullanılarak kısa TR corpus çalıştırılır. Test verisi ve sonuçları secret içermez.
   - Release latency SLO'su, launch kapasite ölçümünden önce operasyon sahibi tarafından
     onaylanır; varsayılan ürün hedefi utterance-end'den viewer render'a toplam p95
     `< 2 s` olarak doğrulanır.
   - **Kanıt:** timestamp'li E2E raporu, provider latency/error dağılımı, caption
     örnekleri ve staging config snapshot'ı.

### Workstream C — Dağıtık state ve worker dayanıklılığı

**Bağımlılık:** `BH-03`, `BH-04`, `BH-06`, PR-05.

1. **PR-07 — Dağıtık caption hub ve stream lifecycle**
   - `OverlayHub` snapshot/fan-out ile `ActiveStreams` bilgisi Redis-backed olur;
     kalıcı overlay token ve channel binding PostgreSQL'de source of truth olur
     (`D1/D2`).
   - Her session için register/subscribe/unregister yaşam döngüsü uygulanır. Stream
     stop, disconnect, plan limit ve backend restart eski snapshot'ı temizler; viewer
     bağlantılarına terminal/liveness sinyali verir.
   - **Kanıt:** iki backend replica ile SDK bir instance'a, EBS diğerine bağlı E2E;
     restart, stale snapshot ve token/session yarış testi.

2. **PR-08 — Worker pool routing ve health**
   - `B1/B2/B3` tamamlanır: feature-gate içinde autoactivation, periyodik health check,
     admin görünürlüğü ve manuel override vardır.
   - `active_ready()` yerine health, capacity ve circuit-breaker durumunu kullanan worker
     pool seçimi eklenir. Bir worker timeout/error verirse aynı havuzdaki sağlıklı
     worker denenir; havuz tükenirse provider-level fallback çalışır.
   - Kapasite ölçümü launch concurrency'sinden türetilir; plan limitleri ve admission
     limitleri ölçülmüş güvenli kapasitenin üstüne çıkarılmaz.
   - **Kanıt:** iki worker failover, unhealthy worker quarantine/recovery, autoactivate
     flag kapalıyken no-op ve admin override testleri.

3. **PR-09 — EBS capacity, abuse ve gözlemlenebilirlik**
   - Per-viewer state rate limit ile per-channel WS limiti aynı efektif kapasite modeline
     bağlanır; OverlayHub ve Extension limitleri çelişmez.
   - Metrics: active streams, utterance duration/bytes, STT/translation latency ve error,
     worker saturation/health, caption age, EBS WS connections/reconnects, 429/5xx,
     Redis fan-out lag ve channel pause sayıları.
   - Alertler launch kapasitesi, caption-age, provider failure ve worker health için
     operasyon runbook'una bağlanır.
   - **Kanıt:** load testte admission kontrolü, metric/alert smoke ve log redaction
     incelemesi.

### Workstream D — Extension ürün doğruluğu

**Bağımlılık:** PR-01, PR-07.

1. **PR-10 — Yayıncı bağlama auth düzeltmesi**
   - `frontend/src/components/twitch-extension-link.tsx`, dashboard'ın yayıncı
     oturumunu (`userFetch`/`getUserToken`) kullanır. Admin UI bu işlemi yayıncı yerine
     yapmaz.
   - Kod oluşturma, binding gösterme ve unlink yalnız authenticated broadcaster'ın kendi
     `user_id`si üzerinde çalışır.
   - **Kanıt:** component contract testleri ve iki kullanıcıyla authorization E2E;
     admin token'ın başka yayıncı adına binding yapamadığı test edilir.

2. **PR-11 — Viewer transport/lifecycle controller**
   - `viewer.js` içindeki network state machine saf/test edilebilir bir controller'a
     ayrılır. Offline veya not-configured ilk state'te düşük frekanslı state polling
     sürer; live olduğunda WS açılır; terminal liveness veya WS kapanışında tekrar state
     çözülür.
   - Yeni session, stream stop, backend restart, token refresh, 429, 5xx ve slow network
     için bounded exponential backoff uygulanır. Poll fallback, WS başarılı olana kadar
     caption akışını sürdürür.
   - **Kanıt:** fake clock/fetch/WebSocket ile offline→live, live→offline→new live,
     token refresh, 429 ve reconnect testleri.

3. **PR-12 — Viewer caption ve config doğruluğu**
   - Dil önceliği sabitlenir: viewer local preference → Twitch broadcaster config →
     EBS global default → ilk available language. EBS global default broadcaster
     seçimini ezmez.
   - Caption expiry, clear ve status davranışı EBS'nin typed TTL ayarından gelir;
     son caption sonsuza kadar ekranda kalmaz.
   - Twitch Configuration Service değiştiğinde dil seçenekleri ve render hemen güncellenir.
   - **Kanıt:** DOM/controller testleri; XSS/textContent, long/RTL caption, preference
     persistence, config update ve TTL clear testleri.

4. **PR-13 — Tekrarlanabilir review artefact'ı**
   - Extension zip'i deterministik komutla üretilir. CI pakette yalnız izinli dosyalar,
     Twitch helper'ın ilk script oluşu, bundle size, `innerHTML`/iframe/eval yasağı ve
     production EBS URL allowlist uyumunu denetler.
   - WS query ticket/JWT için proxy/access-log redaction doğrulanır; token veya query
     değeri kalıcı loglara giremez.
   - **Kanıt:** CI artefact report, package hash, static policy testleri ve reverse-proxy
     log redaction smoke.

### Workstream E — Production ortamı ve canlı doğrulama

**Bağımlılık:** Kanonik merge sırasının ilk yedi adımı tamam, security ve policy
gates yeşil.

1. **PR-14 — Staging ve production altyapısı**
   - `ext-staging.strovak.com`, `ext-test.strovak.com` ve `ext.strovak.com` DNS/TLS
     olarak ayrılır; yalnız gerekli `https`/`wss` originleri Twitch allowlist'e girer.
   - Reverse proxy WebSocket upgrade, request timeout/body limit, query redaction ve
     health/readiness routing'i doğrulanır. Public EBS ile mTLS SDK listener ayrılır.
   - Vault/ManagedSecrets'a `TWITCH_EXT_SECRET` ve provider secret'ları girilir;
     `TWITCH_EXT_SECRET_PREVIOUS` ile rotasyon tatbikatı yapılır.
   - Production config: mock fallback kapalı, global Extension gate kapalı, yalnız
     allowlisted test kanalları aktif.
   - **Kanıt:** DNS/TLS, `/health`/readiness, secret rotation, external WS handshake ve
     negative config test raporu.

2. **PR-15 — Multi-replica load ve failure game day**
   - En az iki backend replica + Redis/PostgreSQL + iki worker ile launch profile'ın
     üzerinde yük testi çalıştırılır. Launch profile, kod sabiti değil operasyon
     konfigürasyonunda saklanan beklenen eşzamanlı yayıncı/viewer sayısıdır.
   - Game day senaryoları: worker kaybı, STT/translation timeout, Redis reconnect,
     backend rolling restart, channel pause, global kill switch ve Twitch viewer WS
     reconnect.
   - **Kanıt:** SLO raporu, alarm ekran görüntüsü/log referansı, kayıp/duplicate caption
     analizi ve rollback süresi.

3. **PR-16 — Hosted Test ve review paketi** *(insan + agent destekli)*
   - Test broadcaster + en az iki test viewer hesabı ile gerçek TR→EN canlı senaryo
     hazırlanır. SDK gereksinimi review walkthrough'ta açıkça yazılır.
   - Hosted Test'te web/theater/fullscreen, iOS/Android, portrait/landscape, slow
     network, backend/SDK disconnect, caption burst, long/RTL caption ve dark/light
     doğrulanır.
   - Policy checklist, privacy/terms/support URL'leri, screenshot, changelog,
     walkthrough ve review channel erişimi tamamlanır. Bütün dış URL'ler allowlist ile
     birebir karşılaştırılır.
   - **Kanıt:** imzalı test checklist'i, gerçek UI screenshot/video, reviewer test
     credential/runbook ve Hosted Test issue listesi (açık yüksek hata = review yok).

## 5. Release ve rollback

### Canary rollout

1. Production deploy edilir ancak global gate kapalı kalır.
2. Internal/test channel binding'leri aktif edilir; diğer kanallar paused kalır.
3. Gerçek yayınlarda latency, caption age, worker saturation ve reconnect oranı
   gözlemlenir.
4. Onaylanmış launch kapasitesi içinde kanal grupları aşamalı açılır. Her aşamanın
   büyüklüğü config/operasyon kararından gelir; bundle veya backend koduna gömülmez.
5. Her aşamada SLO, güvenlik logları ve support feedback incelenmeden sonraki aşamaya
   geçilmez.

### Geri alma

| Olay | İlk aksiyon | Kullanıcı etkisi |
|---|---|---|
| Tek kanal caption hatası | Kanal status `paused` | O kanalda güvenli unavailable durumu |
| Worker/provider bozulması | Sağlıklı worker → provider fallback | Gecikme artabilir, caption korunur |
| Geniş EBS/pipeline hatası | Global Extension gate kapatılır | Viewer ticari mesaj görmeden unavailable olur |
| Deploy regresyonu | Önceki container/image'a dönülür | Mevcut Extension bundle korunur |
| Secret şüphesi | Secret rotate, önceki secret kısa geçiş penceresinde tutulur | Viewer tokenları doğal yenilenir |

Rollback, database migration'ını geri almak zorunda bırakmamalıdır: migrationlar
forward-compatible, kanal status ve feature gate'ler ise anlık kill switch olarak
kullanılır.

## 6. Zorunlu test matrisi

| Katman | Otomatik kanıt | Canlı/operasyon kanıtı |
|---|---|---|
| SDK + protocol | flags, assembler, bounded buffer, mTLS negative tests | Gerçek mikrofon/VAD utterance capture |
| STT + çeviri | provider timeout/fallback, fake worker, real HTTP contract | GPU + DeepL corpus/latency raporu |
| Dağıtık backend | 2 replica + Redis fan-out, restart/stale-state tests | rolling restart game day |
| EBS | JWT/channel/role, rate/capacity, token redaction | external `wss` handshake + rotation drill |
| Viewer | fake clock/fetch/WS lifecycle, DOM security/TTL | Twitch Hosted Test device/network matrisi |
| Policy/review | static bundle and URL allowlist checks | checklist, walkthrough, review channel |
| Load | reproducible load scenario + SLO assertion | staging launch-profile run ve alert smoke |

Mevcut `extension/npm test`, `cargo test -p streamdubber-backend --test
ext_twitch_api --test overlay_api --test ws_integration`, SDK cloud E2E ve worker
testleri bu matrisin regression tabanıdır; tek başına release kanıtı değildir.

## 7. İnsan kararı gereken noktalar

- Launch concurrency ve viewer kapasitesi: GPU benchmark sonuçlarından sonra operasyon
  sahibi onaylar.
- Gerçek provider maliyeti/fallback sırası ve kabul edilebilir degraded caption
  davranışı: ürün + operasyon kararı.
- DNS, Twitch Developer Console kaydı, Hosted Test yükleme, review credential ve review
  başvurusu: insan yetkisi gerektirir.
- SLO ihlalinde canary genişletme veya rollback kararı: release owner verir.

## 8. Definition of Done

- [ ] Kanonik merge sırasındaki tüm PR maddeleri tamamlandı ve bağımlı hardening
  maddeleri roadmap'te işaretli.
- [ ] Otomatik test matrisi CI'da yeşil; gerçek provider/GPU staging raporu ekli.
- [ ] Production iki backend replica ve worker pool ile test edildi; single-instance
  varsayımına dayanmıyor.
- [ ] Security check, dependency audit, mTLS, secret rotation ve log redaction geçti.
- [ ] Global ve kanal bazlı rollback tatbikatı yapıldı.
- [ ] Twitch Hosted Test ve policy checklist geçti; review paketi gerçek canlı senaryoyla
  doğrulandı.
- [ ] Review onayı ve kontrollü canary sonrasında SLO'lar kabul edildi.
