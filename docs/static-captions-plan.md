# Statik Altyazı Uçtan Uca Hazırlık Planı (GPU Worker → YouTube CC / Twitch Extension / Kick Overlay)

**Durum:** Planlandı — implementasyon başlamadı (2026-07-10)
**Kapsam:** Backend (Rust/Axum) + SDK (Tauri/Rust) + Frontend (Next.js) + stt-server (Python)
**Faz uyumu:** Faz 1 tamamlama + Faz 2.6 (faster-whisper) — Faz 1 sınırlarını ihlal etmez (dublaj/mux işi YOK)
**Bağlı dokümanlar:** `docs/websocket-protocol.md`, `docs/audio-pipeline.md`, `docs/twitch-extension-plan.md`, `docs/local-stt-plan.md`, `docs/dashboard.md`, `streamdubber_roadmap_final.md`

## İlerleme

- [ ] A1. Protokol: reserved byte → flags (`utterance_end`)
- [ ] A2. SDK: VAD cümle sonu işareti
- [ ] A3. Backend: UtteranceAssembler
- [ ] A4. stt-server dokümantasyon notu
- [ ] B1. Provision başarısında otomatik STT aktivasyonu
- [ ] B2. Periyodik GPU worker health-check döngüsü
- [ ] B3. Admin panel: aktif sağlayıcı + sağlık görünürlüğü
- [ ] C1. Mikrofon/cloud session'ı OBS'ten bağımsız başlat
- [ ] C2. OBS yeniden bağlanma (relay accept döngüsü)
- [ ] D1. Kullanıcı başına kalıcı overlay token
- [ ] D2. Liveness'i token varlığından ayır + snapshot reset
- [ ] D3. Panel: sabit overlay URL + token yenileme
- [ ] E1. YouTube aktif broadcast keşfi
- [ ] E2. CaptionPusher (OverlayHub → YouTube CC, dil başına track)
- [ ] E3. AppState wiring (YouTubeCaptions + pusher)
- [ ] E4. Canlı doğrulama maddesi (lang param)
- [ ] F1. Twitch extension e2e smoke + prod checklist maddesi
- [ ] G1. Uçtan uca doğrulama + doküman/roadmap güncellemeleri

## Hedef

Admin panelden bir GPU worker (self-hosted faster-whisper sunucusu) eklenip provision edildiğinde, yayıncının **tüm statik altyazı kanalları kutudan çıktığı gibi çalışmalı**:

1. **OBS Overlay** (`/o/{token}` Browser Source — Kick dahil her platforma videoya gömülü altyazı)
2. **YouTube Native CC** (closedCaptionsIngestionUrl'e çok dilli caption track push)
3. **Twitch Extension** (izleyici tarafı canlı altyazı)

## Neden — Mevcut Zincirin Koptuğu Yerler (2026-07-10 analizi)

Uçtan uca akış (pairing → SDK → mikrofon → backend pipeline → çıkışlar) kod üzerinden izlendi. Zincir mimari olarak bağlı ve testli, ancak beş noktada kopuk:

1. **STT frame problemi (en kritik):** SDK her 20ms'lik voiced frame'i ayrı binary frame olarak gönderiyor (`sdk/src/core/gate.rs:67` `CapturePipeline::next_frame`, `sdk/src/core/cloud_session.rs:200`), backend her frame'de ayrı STT çağrısı yapıyor (`backend/src/ws/mod.rs:426` `send_for_audio` → `backend/src/pipeline/mod.rs:142` `process_audio` → `gateway.transcribe`). Konuşma sırasında saniyede ~50 STT + hedef dil başına ~50 çeviri isteği; 20ms'lik parçadan anlamlı transkript çıkmaz. Hiçbir yerde utterance birleştirme yok (grep ile doğrulandı). GPU worker (`stt-server/main.py:75` `/transcribe`) batch faster-whisper — cümle bazlı istekle birebir uyumlu, 20ms parçayla kullanılamaz.
2. **Mikrofon OBS'e koşullu:** `run_desktop_shell` (`sdk/src/lib.rs:128-149`) OBS lokal relay'e publish edene kadar bloklar; mic capture + cloud session ancak ondan sonra başlar. Backend YouTube'u relay hedefi olarak **reddediyor** (`backend/src/relay/mod.rs:105` — YouTube OBS'ten doğrudan yayın alır). Yani YouTube yayıncısı relay kullanmaz → mikrofon hiç başlamaz → hiçbir altyazı üretilmez.
3. **GPU worker aktivasyonu manuel:** Provision akışı var (`backend/src/stt_servers/mod.rs:634`, `provision.rs:164`) ama başarı sonrası `stt.provider` config'i elle değiştirilmeli (roadmap §2.6 "Admin toggle" maddesi). Health check sadece provision anında bir kez (`mark_ready`, `stt_servers/mod.rs:316`); periyodik izleme yok. Panel (`admin-stt-servers-panel.tsx`) aktiflik/sağlık göstermiyor.
4. **YouTube caption push bağlı değil:** OAuth + refresh hazır (`backend/src/integrations/mod.rs:375` `get_fresh_access_token`, scope `youtube.force-ssl`, offline params `mod.rs:168-171`), caption client hazır ve testli (`backend/src/integrations/youtube.rs` — `fetch_ingestion_url:72`, `push_caption:102`), ama: aktif broadcast id keşfi yok, `push_caption`'ın production çağıranı yok, `YouTubeCaptions` AppState'te kurulmuyor.
5. **Overlay token her yayında değişiyor:** `stream_start`'ta yeni uuid (`backend/src/ws/mod.rs:344`) → OBS Browser Source URL'i her yayında farklı; yayıncı her yayın öncesi OBS'i güncellemek zorunda. Liveness sinyali token varlığına gömülü (`ActiveStreams.get`, `backend/src/overlay/mod.rs:173`).

Twitch Extension backend'i + extension frontend'i kod olarak tamam (`backend/src/ext_twitch/mod.rs`, `extension/js/`); kalan config/secret + insan-işi Hosted Test.

## Kesinleşmiş Tasarım Kararları (ürün sahibiyle 2026-07-10'da netleşti)

1. **GPU worker otomatik aktifleşir:** Provision "ready" olur olmaz `stt.provider=selfhosted` otomatik set edilir; `stt.fallback=deepgram` korunur. `stt.selfhosted.autoactivate` config'i (seed `true`) ile kapatılabilir.
2. **Utterance birleştirme: SDK işaretler + backend tamponlar.** Latency gerekçesi: sadece-backend çözümü sessizliği "frame gelmemesinden" anlamak zorunda (+300-400ms yapay bekleme/cümle); SDK'daki VAD ise cümle sonunu kesin bilir (hangover kapanışı) → işaretle ek bekleme ~0ms. Binary header'ın reserved byte'ı flag olur — protokol geriye uyumlu. Backend'de işaretsiz akışlar için gap/süre fallback'i kalır. Proje canlıya çıkmadığı için sahada eski SDK kısıtı yok.
3. **YouTube caption dili: kaynak + TÜM hedef diller ayrı track.** Ingestion URL `lang` (+`name`) query parametreleriyle dil başına ayrı caption track kabul eder; izleyici dili YouTube player'ın kendi menüsünden seçer. Bu, Faz 2 multi-audio/dublaj modeliyle aynı mantık (her dil kendi track'i). `lang` param davranışı canlı yayında smoke testle doğrulanacak (Görev E4).

## Mimari Özet

```
SDK (yayıncı makinesi)                     Backend (VPS)                          Çıkışlar
┌──────────────────────┐                  ┌────────────────────────────┐
│ mic → preprocess     │   binary frame   │ ws/stream                  │
│ → VAD (hangover)     │  (+utterance_end │ → UtteranceAssembler (YENİ)│
│ → 20ms frame'ler     │      flag'i)     │ → tek STT çağrısı          │
│   [OBS'ten BAĞIMSIZ  │ ───────────────► │   (selfhosted GPU worker,  │
│    başlar — YENİ]    │                  │    fallback: deepgram)     │
└──────────────────────┘                  │ → DeepL (hedef dil paralel)│
┌──────────────────────┐                  │ → Subtitle                 │
│ OBS → localhost:19350│                  │   ├→ SDK'ya geri (WS)      │
│ → RTMP relay fan-out │                  │   ├→ OverlayHub.publish    │
│   (Twitch/Kick vs.)  │                  │   │   ├→ /overlay/{token} ─┼─► OBS Browser Source (/o/{token}) → Kick vd.
└──────────────────────┘                  │   │   ├→ ext_twitch WS/poll┼─► Twitch Extension izleyicisi
                                          │   │   └→ CaptionPusher(YENİ┼─► YouTube CC ingestion (dil başına track)
                                          │   └ overlay_token: KALICI  │
                                          │     (kullanıcı başına—YENİ)│
                                          └────────────────────────────┘
Admin panel: GPU worker ekle → provision → OTOMATİK stt.provider=selfhosted (YENİ) + periyodik health check (YENİ)
```

## Çalışma Kuralları (uygulayıcı agent için)

- Başlamadan oku: `CLAUDE.md`, bu doküman, ilgili bölge için "Bağlı dokümanlar".
- TDD zorunlu: önce test (red) → implement (green) → refactor. Kritik path %80+ coverage.
- Her görev = tek sorumluluk = ayrı commit (`feat:`/`fix:`/`test:`/`chore:` prefix). Push yok.
- `unwrap()` yasak; config değerleri ServiceConfig'ten (`config::get` deseni), hard-code yok.
- Görev bitince: bu dokümandaki İlerleme listesinde tik + ilgili göreve `> Durum (tarih): ...` blockquote'u ekle, roadmap'te ilgili satır varsa tikle, `cargo test && cargo clippy && npm run lint && npm run test` temiz olmalı.
- Bölge sırası önerisi: **A → B → C → D → E → F** (A ve B paralel yapılabilir; C SDK tarafı bağımsız; E, D'den bağımsız ama ikisi de `ws/mod.rs stream_start`'a dokunur — çakışmayı önlemek için D'yi önce bitir).

---

## Bölge A — Utterance Birleştirme (STT kalite/latency temeli)

### A1. Protokol: reserved byte → flags

**Bugün:** Binary frame header 8 byte: magic `SDAU` + codec + `[5] reserved: 0x00` + payload_len (`docs/websocket-protocol.md` "Audio Binary Frame"; SDK `sdk/src/core/audio_frame.rs`; backend parser `backend/src/ws/frame.rs`).

**Yapılacak:** Byte[5] `flags` olur: `bit0 = utterance_end` (bu frame bir cümlenin son frame'i). `0x00` = işaret yok → tam geriye uyumlu. `build_audio_frame`'e flags parametresi; `frame::parse` çıktısına `utterance_end: bool`. `docs/websocket-protocol.md` güncellenir (frame tablosu + akış diyagramı notu).

Doğrulama: İki tarafta unit test — flag'li/flag'siz frame round-trip; eski format (`0x00`) parse edilmeye devam eder.

### A2. SDK: VAD cümle sonu işareti

**Bugün:** `VadGate` (`sdk/src/core/vad.rs`) RMS eşiği + 200ms hangover ile frame başına voiced/silent kararı verir; `CapturePipeline::next_frame` (`sdk/src/core/gate.rs:67`) sessiz frame'leri yutar, voiced frame'i tek tek döndürür. Cümle sonu bilgisi dışarı çıkmıyor.

**Yapılacak:** `VadGate` voiced→silent geçişini (hangover penceresi kapandığı an) bildirir; `CapturePipeline` utterance sonunu işaretler. İki geçerli tasarım (implementasyoncu seçer, testle sabitler):
- (a) son voiced frame flag'li gönderilir (bir frame gecikmeli işaretleme gerektirir — buffer-1 yaklaşımı), veya
- (b) hangover kapanınca payload'sız (`payload_len=0`) `utterance_end` frame'i gönderilir — daha basit; backend'de "boş payload'lı flag frame = sadece flush sinyali" olarak işlenir.

`cloud_session.rs` değişen akışı olduğu gibi taşır (frame'ler zaten kanal üzerinden geçiyor).

Doğrulama: `gate.rs` testleri: konuşma→sessizlik→konuşma dizisinde doğru noktada utterance_end üretilir; `MockCapture` ile mevcut test deseni kullanılır.

### A3. Backend: UtteranceAssembler

**Bugün:** `StreamSession::process_audio` (`backend/src/pipeline/mod.rs:142`) her çağrıda `gateway.transcribe(audio)` — frame başına STT.

**Yapılacak:** `StreamSession`'a tampon: gelen frame payload'ları birikir; **flush koşulları** (öncelik sırasıyla):

1. `utterance_end` flag'i geldi (A1/A2 yolu — ana yol, ek bekleme yok),
2. tampon `stt.utterance.max_ms` eşdeğerini aştı (seed: `5000`; 16kHz s16le → ms başına 32 byte, 5sn = 160.000 byte) — uzun kesintisiz konuşmada ara flush,
3. eski/işaretsiz SDK fallback'i: son frame'in üzerinden `stt.utterance.gap_ms` (seed: `400`) geçti ve tampon boş değil — varış zamanı bazlı; WS döngüsü mesaj-güdümlü olduğundan bu kontrol ya `tokio::select!` + timeout ile ya da bir sonraki frame gelişinde "arada boşluk var mıydı" kontrolüyle yapılır (ikincisi daha basit; boşluk sonrası gelen ilk frame önce eski tamponu flush ettirir).

Flush → **tek** `gateway.transcribe` çağrısı → mevcut `subtitle_for` akışı (çeviri + overlay publish değişmez). Kota tick'i (`tick_usage`) davranışı korunur (duvar saati bazlı — frame başına değil flush başına çağrılması sonucu değiştirmez, testle doğrula). Config anahtarları `backend/src/config.rs` seed'ine eklenir (admin panelden restart'sız değişir).

Doğrulama: Pipeline testleri: (1) flag'li dizi → tek transcribe çağrısı (MockStt çağrı sayacı), (2) 5sn+ kesintisiz → max_ms'te ara flush, (3) işaretsiz frame'ler + boşluk → gap flush, (4) boş tampon flush edilmez, (5) mevcut testler kırılmaz.

### A4. stt-server notu

**Yapılacak:** Kod değişikliği yok (batch `/transcribe` cümle istekleriyle birebir uyumlu). `main.py` docstring'ine "girdi = VAD ile bölünmüş tek utterance (tipik 1-6sn), 16kHz mono s16le" notu; isteğe bağlı çok kısa girdi (<100ms) için erken boş dönüş.

---

## Bölge B — GPU Worker Otomatik Aktivasyon + Sağlık İzleme

### B1. Provision başarısında otomatik aktivasyon

**Bugün:** `provision_server` (`backend/src/stt_servers/mod.rs:634`) başarıda `pin_fingerprint` + `mark_ready(endpoint, token, gpu_name, vram_mb)` yapar; `stt.provider` değişmez (varsayılan `deepgram`, `backend/src/config.rs:69`). Gateway seçim/fallback: `backend/src/gateway/mod.rs:77-103` (`stt.provider` → `stt.fallback`); `SelfHostedStt` her istekte `active_ready()` ile sunucu çözer (`backend/src/gateway/selfhosted_stt.rs:55`), yoksa `Unavailable` → fallback zaten çalışır.

**Yapılacak:** `mark_ready` başarısından sonra, `stt.selfhosted.autoactivate` (yeni seed: `true`) açıksa `service_config.set("stt.provider", "selfhosted")`. `stt.fallback` dokunulmaz (deepgram kalır). Logger'a Info kaydı ("stt provider auto-activated: selfhosted"). Not: provision `tokio::spawn` içinde koşuyor (`mod.rs:658`) — `ServiceConfig` Arc'ı task'a taşınmalı.

Doğrulama: Test: FakeProvisioner başarı → config `selfhosted` oldu; autoactivate=false → değişmedi; provision hatası → değişmedi.

### B2. Periyodik health-check döngüsü

**Bugün:** `last_health_check_at` sadece provision anında set edilir (`mark_ready`, `mod.rs:316`); arka plan izleme yok. Worker `/health` endpoint'i hazır (`stt-server/main.py:119` — `{status, model, gpu_name, vram_mb}`).

**Yapılacak:** `main.rs`'te uzun ömürlü `tokio::spawn` döngüsü (mevcut spawn desenleri: `main.rs:622` shutdown watcher): her `stt.selfhosted.health_interval_secs` (yeni seed: `60`) saniyede Ready sunucuların `/health`'ine bearer token'la GET; başarı → `last_health_check_at` güncelle, başarısızlık (N ardışık, örn. 3) → `status=Error` + `status_detail`. Store'a `record_health(id, ok, detail)` benzeri metod eklenir. Aktif provider selfhosted iken worker Error'a düşerse ek bir şey gerekmez — gateway `Unavailable` → Deepgram fallback zaten devrede; görünürlük panelde sağlanır.

Doğrulama: Store metodu unit testleri; döngü mantığı (ardışık hata eşiği, Ready'e dönüş) mock HTTP ile test.

### B3. Admin panel görünürlüğü

**Bugün:** `frontend/src/components/admin-stt-servers-panel.tsx` — ekle/provision/sil var; `last_health_check_at` tipte var (satır 24) ama render edilmiyor; aktif sağlayıcı gösterilmiyor; aktifleştirme kontrolü yok.

**Yapılacak:** (1) Panel başlığına aktif STT sağlayıcı rozeti (`GET /config`'ten `stt.provider`); (2) sunucu satırına `last_health_check_at` ("son sağlık: X dk önce") + Error durumunda `status_detail`; (3) "Aktifleştir / Devre dışı bırak" butonu (`PUT /admin/config` ile `stt.provider` set — mevcut config panel endpoint'i yeniden kullanılır, yeni backend yüzeyi gerekmez).

Doğrulama: Mevcut panel test deseni (`admin-stt-servers-panel.test.ts`) genişletilir; `npm run test` + `npm run lint`.

---

## Bölge C — Mikrofonun OBS Publish'inden Bağımsızlaşması

### C1. Cloud session'ı hemen başlat

**Bugün:** `run_desktop_shell` (`sdk/src/lib.rs:93`): targets sync → heartbeat → `bind_ingest_listener` → **`accept_and_relay` OBS bekler (bloklar)** → relay başlar → ancak ondan sonra mic capture + `run_cloud_session` spawn edilir (`lib.rs:128-180`). `dub_audio_sinks` relay engine'den alınıp cloud session'a veriliyor (`lib.rs:149`).

**Yapılacak:** Mic capture + cloud session, listener bind'ından hemen sonra (OBS'ten bağımsız) spawn edilir. Relay accept ayrı task olur. Dub sinks bağı: relay engine henüz yokken **boş map** ile başlanır — bugünkü fiili durumla aynı (TTS `tts.provider=disabled` varsayılan, dub target'sız yayıncıda map zaten boş; `route_dub_audio` sink yoksa sessizce düşürüyor, `cloud_session.rs:125`). Engine sonradan başlayınca sink'lerin canlı bağlanması Faz 2 mux işine not düşülür (bu plan kapsamı dışı). `ShellStatus.mic_capture_ok` semantiği güncellenir (artık OBS beklemeden set edilir — `lib.rs:63` yorumu düzeltilir).

Doğrulama: Mevcut sdk testleri + yeni test: OBS hiç bağlanmadan cloud session başlar (MockCapture ile; `scripts/smoke.sh`'teki mevcut e2e deseni referans).

### C2. OBS yeniden bağlanma

**Bugün:** `accept_and_relay` tek seferlik (`sdk/src/core/relay_service.rs:27` — "OBS koparsa çağıran tekrar çağırmalı" ama `run_desktop_shell` çağırmıyor).

**Yapılacak:** Accept döngüye alınır: OBS kopunca engine düşer, listener yeni publish bekler, yeni `RelayEngine` kurulur (targets yeniden sync edilebilir). `relay_listening` status'u doğru yansıtılır.

Doğrulama: Test: iki ardışık publish bağlantısı — ikisi de relay'lenir.

---

## Bölge D — Kalıcı Overlay Token (Kick / OBS Browser Source)

### D1. Kullanıcı başına kalıcı token

**Bugün:** Her `stream_start`'ta `uuid::Uuid::new_v4()` (`backend/src/ws/mod.rs:344`); kalıcılık yok (grep: migrations/schema'da overlay token yok).

**Yapılacak:** Kullanıcı başına kalıcı overlay token deposu — şifreli dosya deseni mevcutta hazır: `RelayTargetStore::with_file` (`backend/src/relay/mod.rs:72`) birebir örnek (AES-256-GCM blob, env ile dosya yolu, yoksa in-memory). `stream_start` yeni uuid yerine `store.token_for(user_id)` kullanır (yoksa üret + persist). AppState'e alan + `FromRef` (`backend/src/state.rs:38` deseni), `main.rs` kurulumu + `AppState::skeleton`.

Doğrulama: Store unit testleri (üret/oku/persist round-trip); ws testi: iki ardışık stream_start aynı token'ı verir.

### D2. Liveness'i token varlığından ayır

**Bugün:** "Canlı mı" = `ActiveStreams.get(user).is_some()` — tüketiciler: `get_overlay_token.active` (`backend/src/overlay/mod.rs:205`), ext_twitch `get_state.live` (`backend/src/ext_twitch/mod.rs:435`) ve `captions_ws` 409 kapısı (`:504-506`). `clear_if` last-writer-wins koruması (`overlay/mod.rs:164`). `OverlayHub` kanalları token-keyed, snapshot hiç temizlenmiyor (`overlay/mod.rs:38-46`) — kalıcı token'da önceki yayının son satırları yeni yayının başında replay edilirdi.

**Yapılacak:** `ActiveStreams` "aktif yayın var mı + token ne" ayrımını taşır (örn. `set/clear` canlılığı, token ayrı kaynaktan). `get_overlay_token` her zaman kalıcı token'ı döner + `active` flag'i gerçek canlılıktan; ext_twitch aynı canlılık sinyalini okur (yapısı değişmez — `channel→binding→user→live?` zinciri korunur, overlay token izleyiciye yine sızmaz). `stream_start`'ta `OverlayHub` ilgili token'ın snapshot'ını temizler (yeni metod, örn. `reset_snapshot(token)`).

Doğrulama: Overlay + ext_twitch mevcut testleri güncellenir; yeni testler: yayın kapalıyken `active=false` ama token dolu; yeni yayın başında eski snapshot replay edilmez; ext_twitch 409 davranışı korunur.

### D3. Panel: sabit URL + yenileme

**Bugün:** `caption-overlay-builder.tsx` canlı değilken placeholder gösterir (`:102-108` — token yokken URL yok).

**Yapılacak:** URL yayın kapalıyken de gösterilir ("yayın başlayınca canlanır" notu — OBS'e bir kez girilir); token sızıntısına karşı `POST /api/overlay/token/regenerate` endpoint'i + panelde yenile butonu (regenerate sonrası eski token'ın hub kanalı düşürülür).

Doğrulama: Frontend test + backend endpoint testi (regenerate → yeni token, eski aboneler kopar).

---

## Bölge E — YouTube Native Caption Pusher

### E1. Aktif broadcast keşfi

**Bugün:** `fetch_ingestion_url(access_token, broadcast_id)` broadcast id'yi parametre alıyor (`backend/src/integrations/youtube.rs:72`); id'yi bulan kod yok.

**Yapılacak:** `youtube.rs`'e `find_active_broadcast(access_token) -> Result<ActiveBroadcast, YouTubeError>`: `GET {api_base}/liveBroadcasts?broadcastStatus=active&mine=true&part=contentDetails` → ilk item'dan id + `closedCaptionsIngestionUrl` (tek çağrı, ayrıca `fetch_ingestion_url` çağırmaya gerek kalmaz). Aktif yayın yoksa belirgin hata (`NoActiveBroadcast`) — pusher bunu "henüz yayın açılmadı, retry" olarak yorumlar.

Doğrulama: Mevcut mock-server test deseniyle (`youtube.rs:161` `spawn_api_mock`) unit testler.

### E2. CaptionPusher komponenti

**Bugün:** Subtitle fan-out tek noktada: `publish_and_send` → `state.overlay.publish(&overlay_token, reply)` (`backend/src/ws/mod.rs:523`). Uzun ömürlü abone deseni mevcut: `ext_twitch::forward_captions` (`backend/src/ext_twitch/mod.rs:532`) — `OverlaySubscription` alır, snapshot replay + `loop/select` ile subtitle filtreler.

**Yapılacak:** Yeni modül `backend/src/captions_push/` (veya `integrations/` altı): `stream_start` başarısında (`ws/mod.rs:344-384`, `active_streams.set` yanında) kullanıcının YouTube entegrasyonu bağlıysa (`Integrations` üzerinden kontrol) bir pusher task spawn edilir:

- `OverlayHub.subscribe(overlay_token)` ile beslenir; **sadece `is_final: true`** subtitle'ları işler (interim push edilmez — kota/uyumluluk).
- Başlangıçta `find_active_broadcast` — yayın YouTube'da henüz açık değilse backoff'la yeniden dener (örn. 15sn aralık, config `integrations.youtube.captions.retry_secs`).
- Her final subtitle için: kaynak metin ana track'e (`push_caption(url, text, seq)`), her `translations[i]` kendi track'ine — ingestion URL'e `&lang={code}&name={code}` eklenerek. **`seq` sayacı dil başına ayrı ve monoton artar.**
- Access token her push turunda `get_fresh_access_token` ile alınır (`integrations/mod.rs:375` — 60sn marjlı yenileme zaten var).
- Hata → `Logger` Warn + backoff; pipeline asla bloklanmaz (pusher ayrı task, `broadcast::Receiver` lag'inde stale mesaj düşürmek kabul — `RecvError::Lagged` yut).
- Yaşam döngüsü: subscription kapanınca / stream_stop / plan limitinde doğal sonlanır (ext_twitch ile aynı).

Config: `integrations.youtube.captions.enabled` (seed: `true`).

Doğrulama: Mock ingestion server ile (youtube.rs test altyapısı hazır): final subtitle → dil başına doğru `lang` + artan `seq`; interim push edilmez; entegrasyon yoksa pusher spawn edilmez; token refresh yolu test edilir.

### E3. AppState wiring

**Yapılacak:** `YouTubeCaptions` (`CaptionsSink` trait objesi — test için mock enjekte edilebilir) + pusher fabrikası `AppState`'e (`state.rs:38` alan + `FromRef` gerekiyorsa; `main.rs:569` `Ok(AppState {...})` kurulumu — `integrations`'tan sonra; `AppState::skeleton` mock ile).

Doğrulama: `cargo test` — skeleton kullanan tüm mevcut testler derlenip geçer.

### E4. Canlı doğrulama maddesi

**Yapılacak:** `streamdubber_roadmap_final.md` "Canlı Doğrulama Listesi"nin "Overlay / YouTube (1.9, 1.10)" bölümüne ek: "ingestion URL `lang`/`name` parametreli çok-track push gerçek YouTube Live yayınında doğrulanacak" — bu parametre davranışı üçüncü parti caption vendor pratiğinden biliniyor ama resmi dokümantasyonu zayıf; **canlı testte tek-track fallback kararı verilebilsin diye pusher'da track-per-lang mantığı tek fonksiyonda izole tutulmalı.**

---

## Bölge F — Twitch Extension Aktifleştirme (çoğu ops)

**Bugün:** EBS tamam (`backend/src/ext_twitch/mod.rs` — bind/state/WS, viewer limitleri, token izolasyonu), extension frontend tamam (`extension/js/viewer.js`, `config.js`), config anahtarları seed'li ama `ext.twitch.enabled=false` (`backend/src/config.rs:189-192`). `docs/twitch-extension-plan.md` kalanları: A5 (test hesapları), B5 (Hosted Test), B6 (review) — insan işi.

**Yapılacak (agent kapsamında — F1):** (1) `GET /ext/twitch/state/{channel_id}` ve captions WS için uçtan uca smoke testi (config açık + binding + aktif yayın senaryosu — mevcut test altyapısıyla); (2) `docs/commands/security-check.md` / prod checklist'ine "prod'da `ext.twitch.enabled=true` + `TWITCH_EXT_SECRET` secret'ı girilecek" maddesi.

**Kapsam dışı (insan):** Twitch console kaydı, Hosted Test yükleme, review başvurusu.

---

## Bölge G — Uçtan Uca Doğrulama + Dokümantasyon (G1)

- `scripts/smoke.sh` genişletmesi: flag'li utterance akışı → tek subtitle üretimi; kalıcı token ile iki ardışık session.
- `docs/commands/test-pipeline.md`'ye yeni senaryo: "GPU worker + utterance + overlay + (mock) YouTube push".
- `docs/websocket-protocol.md`: flags alanı, UtteranceAssembler backend akışı, `stream_ack.overlay_token`'ın artık kalıcı olduğu notu.
- `docs/audio-pipeline.md` + `docs/dashboard.md`: ilgili bölümler güncellenir.
- `streamdubber_roadmap_final.md`: tamamlanan işlerin tiki repo formatında (checkbox + inline HTML yorum).

## Kapsam Dışı

- Gerçek zamanlı streaming STT (Deepgram WS / Voxtral — roadmap'te Faz 3 araştırma notu)
- SDK'da Opus encode (PCM s16le kalır)
- Dublaj/mux değişiklikleri (dub sink'lerin geç bağlanması dahil — Faz 2)
- mTLS client-cert connector (roadmap canlı doğrulama listesinde ayrı)
- `usage_update` periyodik mesajı (bilinen eksik, ayrı küçük iş — `docs/websocket-protocol.md:199`)
- Twitch Hosted Test / review (insan adımları)
- Gerçek GPU kutusunda canlı smoke + TR kalite testi (roadmap §2.6 ilk iki madde — insan/donanım gerektirir; bu plan kod tarafını hazır eder)

## Riskler

1. **YouTube `lang` param:** Çok-track push davranışı canlıda doğrulanmadı — E4 izolasyon şartına uy (tek fonksiyon, kolay tek-track fallback).
2. **Gap fallback ayarı:** `stt.utterance.gap_ms=400` konuşma temposuna göre ayar isteyebilir — config-driven olduğu için admin panelden düzeltilebilir.
3. **Kalıcı token + stale snapshot:** D2'deki `reset_snapshot` atlanırsa yeni yayının başında eski yayının altyazıları görünür — D1 ile D2 aynı PR'da gitmeli.
4. **Provision task'inde config set (B1):** `ServiceConfig` Arc'ının spawn'a taşınması gerekir; DB-persist eden config yolunda (varsa) async dikkat.
5. **C1 sıralaması:** Cloud session erken başlayınca kota sayacı OBS'siz de işler — bu bilinçli (mikrofon açık = hizmet veriliyor); istenmezse `stream_start`'ı VAD ilk konuşmayı görünce göndermek ayrı iyileştirme (nota düşüldü, kapsam dışı).

## Roadmap Eşlemesi

| Bölge | Roadmap |
|---|---|
| A | §1.7 STT notu (streaming yerine utterance-batch), §2.6 faster-whisper uyumu |
| B | §2.6 (GPU'lu sunucu maddesi; "Admin toggle" → otomatik aktivasyon) |
| C | §1.8/1.9 canlı doğrulama ön koşulu |
| D | §1.9 OBS Browser Source; Canlı Doğrulama "Overlay / YouTube (1.9, 1.10)" |
| E | §1.10 YouTube Native CC; Canlı Doğrulama "Overlay / YouTube (1.9, 1.10)" |
| F | §2.4 Twitch Extension |
