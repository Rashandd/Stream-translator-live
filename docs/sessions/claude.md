# Claude Session

## Agent
Claude (Sonnet 5, claude-sonnet-5)

## Neredeyim? (GÜNCEL — 2026-07-08, background job, bu oturumun sonu)
Kullanıcı geri bildirimi: yayıncı/admin panelindeki izleyici istatistikleri
"alakasız ve uğraştırıcı", MVP için gereksiz karmaşıklık; ayrıca YouTube'un
"otomatik desteği" (native CC/multi-audio) olduğunu ve OBS'ten YouTube'a SDK
relay üzerinden video göndermemesi gerektiğini fark etmiş ama tam olarak nasıl
çalıştığını bilmiyordu. İki AskUserQuestion ile netleştirildi, ikisi de
"Önerilen" seçenekle onaylandı:

1. Overview + ayrı Analytics sayfasındaki sahte izleyici/dil dağılımı
   istatistikleri (EN 680/TR 412/ES 148, hiç gerçek veriye bağlanmamıştı)
   Faz 1'den TAMAMEN kaldırıldı.
2. Backend `PUT /api/relay/targets` artık `platform=="youtube"` (case-insensitive)
   hedefini 422 ile reddediyor — YouTube zaten OBS'ten doğrudan yayın alıyor,
   relay üzerinden bir de ona göndermek gereksiz/yanlış; YouTube'un
   altyazı/multi-audio'su `backend/src/integrations/youtube.rs` OAuth
   akışından geliyor.

Her ikisi de kodlandı, TEST EDİLDİ (workspace `cargo test` 288+ yeşil,
`cargo clippy --workspace --all-targets -D warnings` temiz, frontend
`tsc/lint/vitest(119)/build` temiz) ve **worktree'de commit'lendi**
(`c321f3e`, branch `worktree-logical-twirling-harp`,
`.claude/worktrees/logical-twirling-harp`). **PUSH EDİLEMEDİ** — bu
background job sandbox'ında GitHub için kimlik bilgisi yok (`origin` HTTPS,
`gh` CLI kurulu değil, uygun SSH anahtarı yok). Kullanıcıya bildirildi,
worktree bilerek dokunulmadan bırakıldı.

## Değiştirdiğim Alanlar (bu oturum, 2026-07-08)
- `frontend/app/[locale]/dashboard/page.tsx` — languageBreakdown bloğu
  kaldırıldı, quickStart tek sütuna indirildi
- `frontend/app/[locale]/dashboard/analytics/page.tsx` — SİLİNDİ (route +
  nav girdisi de kaldırıldı)
- `frontend/src/i18n/dictionaries/en.json` — `dashboard.overview.
  languageBreakdown`, `dashboard.analytics`, nav'daki "Analytics" girdisi
  kaldırıldı
- `backend/src/relay/mod.rs` — `set()` içine youtube reddi + yeni test
  (`validation_rejects_youtube_as_relay_target`) + mevcut testlerdeki
  "youtube" platform verisi twitch/kick'e çevrildi
- `backend/tests/pairing_api.rs` — yeni e2e test
  (`relay_targets_reject_youtube`) + mevcut testlerdeki youtube kullanımı
  değiştirildi
- `sdk/tests/relay_wiring.rs` — test verisi youtube→twitch (backend artık
  youtube'u reddettiği için gerekli)
- `streamdubber_roadmap_final.md` — Ana Sayfa'dan "İzleyici dil dağılımı"
  maddesi çıkarıldı + V9 altına youtube-relay-reddi kararı not edildi

## Son Kararlar (bu oturum)
- Viewer/dil istatistikleri Faz 1 kapsamı DIŞI — basit tekil "Live viewers"
  metrik kartı kaldı, dil kırılımı/Analytics sayfası tamamen gitti.
- YouTube relay hedefi olarak YASAK (backend validasyonu), sadece
  Twitch/Kick/serbest RTMP etiketleri relay'e girebilir. Bu, mevcut frontend
  `sdk-stream-key-panel.tsx`'i ETKİLEMEDİ (o zaten tek sabit "obs" hedefi
  kullanıyor, per-platform seçim henüz frontend'de yok).

## Sonraki Adım (TAM OLARAK)
1. **[TAMAMLANDI]** Bu worktree branch'i main'e lokal merge edildi (bkz. bu
   girdinin üstündeki merge commit) — kullanıcı ayrıca push etmek isterse
   kendi kimlik bilgileriyle yapabilir, engel değil artık.
2. Bu iş, aşağıdaki (kronolojik olarak önceki) oturum notlarındaki Task 2.2/2.3
   çalışmalarıyla TAMAMEN BAĞIMSIZ — birbirini etkilemedi, ayrı dosyalarda.

## Blokerlar / Dikkat (bu oturum)
- Worktree'deki commit main'e **lokal olarak merge edildi** (GitHub'a push
  edilmedi — bu sandbox'ta hâlâ kimlik bilgisi yok). Kullanıcı isterse kendi
  makinesinden `git push` ile uzağa gönderebilir.
- Merge sırasında bu dosyada (`docs/sessions/claude.md`) tek çakışma çıktı —
  bu girdinin en/eski hâli ile main'in 2026-07-07 "cihazda mux" oturum notu
  aynı başlığı (`## Neredeyim?`) farklı şekilde güncellemişti; ikisi de
  korunarak elle çözüldü (aşağıda "önceki oturum" olarak devam ediyor).
  `en.json`, `streamdubber_roadmap_final.md`, `backend/src/relay/mod.rs`
  çakışmasız otomatik birleşti.

---

## Neredeyim? (2026-07-07, cihazda mux oturumu sonu — önceki oturum)

### Bu oturum (devam) — Task 2.3'ün kendisi: cihazda mux GERÇEKTEN kodlandı
Önceki oturum (aşağıda "SDK ses wiring oturumu" başlığı altında) Task 2.3'ün ön koşulunu bitirmişti.
Kullanıcı devamında **"2.3 ün kendisini nası yapcaz peki"** dedi — meetily adlı referans bir proje
klonlanıp incelendi (aynı problem sınıfını, yani kesikli TTS'i sürekli akışa çevirmeyi, ÇÖZMEDİĞİ
görüldü — kullanılabilir tek fikir reaktif silence-fill oldu), sonra plan-mode'da detaylı bir
implementasyon planı yazıldı ve onaylandı. **Tüm plan kodlandı, test edildi, çalışıyor:**

1. **Ön-adım:** `gateway::elevenlabs::OUTPUT_FORMAT` `mp3_44100_128` → `pcm_24000` (SDK'da mp3 decode
   bağımlılığı hiç eklenmedi — her yerde zaten PCM s16le var). `pipeline::ASSUMED_TTS_BITRATE_BPS`
   384_000'e güncellendi (hâlâ kesin hesap, format sabit CBR PCM). Bir test (`cloud_tts_charges_
   usage_pool_shared_with_stt`) yeni bitrate'te round'lanan saniyenin 0 çıkmaması için metin
   uzunluğu 20_000→60_000 karaktere çıkarıldı (UsageTracker saniyeyi en yakın tama yuvarlıyor).
2. **`RelayTarget.dub_language: Option<String>`** backend (`relay/mod.rs`) + SDK (`rtmp/target.rs`
   — `with_dub_language()` chainable setter, `parse()` imzası değişmedi) + `relay_sync.rs` (backend'den
   çekip taşır) — mux'ın yönlendirme anahtarı.
3. **`sdk/src/core/mux/`** (mux.rs dizine dönüştü, eski `OutputPlan` enum'u korundu):
   - `flv.rs` — `RtmpMessage`→FLV byte serileştirme (video+script tag'leri, audio ATLANIR — ses
     ayrı bir input'tan geliyor). RTMP media payload'ı zaten FLV tag body'siyle birebir aynı format.
   - `audio_scheduler.rs` — `DubAudioScheduler`: kesikli dublaj sesini sürekli PCM'e çevirir
     (`push_dub_audio`/`next_chunk(n)` — kuyruk tükenirse sessizlik). Kullanıcı kararı: uzun
     gecikmede sessizlik SÜRER, orijinal mikrofona düşülmez (mic PCM'i mux koduna hiç taşınmıyor).
   - `ffmpeg_process.rs` — ffmpeg child process lifecycle (`gpu_probe.rs`'nin best-effort
     external-binary deseni: `ffmpeg_available()`, spawn/stdin/wait, `kill_on_drop` — manuel `Drop`
     gerekmedi, tokio zaten hallediyor).
4. **`rtmp/relay.rs`**: `run_dub_target`/`run_dub_session` eklendi — `dub_language`'i olan hedefler
   `run_target` yerine buraya yönlendirilir. Yerel bir `TcpListener` açılır, ffmpeg
   `-f flv -i pipe:0 -f s16le -ar 24000 -ac 1 -i tcp://127.0.0.1:<port> -map 0:v -map 1:a -c:v copy
   -c:a aac -f flv <tc_url>/<stream_key>` komutuyla spawn edilir (RTMP çıkışı ffmpeg'in KENDİ
   muxer'ı üzerinden gider — bizim `rtmp` modülümüz yalnız "publish kabul et"/"publish et" biliyor,
   "bir client'a stream servis et" rolü hiç yok, bu yüzden ffmpeg'e yerel bir RTMP ayna sunucusu
   YAPMADIK, video+metadata'yı FLV olarak stdin'e, sesi TCP'ye veriyoruz — ekstra fd Windows'ta
   taşınabilir değil). `RelayEngine.dub_audio_sinks()` (dil→`mpsc::Sender<Vec<i16>>` haritası) dışa
   açıldı.
5. **KRİTİK bulgu, elle debug edildi:** ffmpeg girişleri SIRAYLA açıp analiz ediyor — stdin (canlı,
   hiç EOF vermeyen) girişi sınırsız analiz edilmeye çalışılırsa ikinci girişi (TCP) HİÇ açmıyor
   (gerçek OBS akışıyla üretimde SONSUZA dek kilitlenir). Elle terminal'de doğrulandı: EOF'suz 5sn
   tutulan bir FIFO'da ffmpeg TCP'ye ancak FIFO kapandıktan SONRA bağlandı. Çözüm: `-probesize 32k
   -analyzeduration 500000` (stdin girişine) — bu LOAD-BEARING bir ayar, ince ayar değil.
6. **`cloud_session.rs`**: `run_cloud_session` yeni bir `dub_audio_sinks: HashMap<String,
   mpsc::Sender<Vec<i16>>>` parametresi aldı; `Subtitle.dubbed_audio` geldikçe her track base64→PCM
   `i16`'ya çevrilip (`decode_dub_audio`) dile göre ilgili sink'e yönlendirilir (`route_dub_audio` —
   sink yoksa/decode hatası varsa sessizce düşer, session'ı düşürmez). SDK'nın `ServerMessage::
   Subtitle`'ına (`cloud.rs`) `dubbed_audio` alanı + `DubbedAudioTrack` tipi eklendi (backend'de
   vardı, SDK'da eksikti — serde `#[serde(default)]` sayesinde önceden sessizce yok sayılıyordu).
7. **`lib.rs::run_desktop_shell`**: `engine.dub_audio_sinks()` alınıp cloud session'a geçiriliyor —
   iki alt-sistem (relay + cloud session) artık gerçekten birbirine bağlı.
8. **Testler:** `flv.rs`/`audio_scheduler.rs` saf birim testleri (network/process YOK) +
   `ffmpeg_process.rs`'in GERÇEK ffmpeg'e karşı smoke testi + `rtmp/relay.rs`'te tam bir e2e test
   (`dub_target_mux_republishes_video_with_pushed_dub_audio` — sahte OBS'ten GERÇEK ffmpeg'le
   üretilmiş H.264 video + pushlanan dublaj sesi, GERÇEK ffmpeg mux'tan geçip sahte-ama-gerçek bir
   RTMP hedefine hem video hem AAC-encoded ses tag'i olarak ulaştığı doğrulandı — fixture'ın kendisi
   de yine gerçek ffmpeg'le `testsrc`'ten üretildi). `cloud_session_wiring.rs`'e 2 yeni e2e (dub
   audio doğru dile yönlendiriliyor + sink yoksa sessizce düşüyor, session çökmüyor). `fake_target`/
   `fake_obs` test yardımcıları ffmpeg'in gerçek RTMP komut sırasını (releaseStream/FCPublish gibi
   ekstra AMF0 komutları da gönderiyor) tolere edecek şekilde genelleştirildi
   (`accept_and_negotiate_publish`, döngü tabanlı, bilmediği komutları atlıyor).
9. **Regresyon:** `cargo test --workspace` → backend 303 unit + sdk 125 unit + tüm e2e suite'ler
   (yeni testler dahil) yeşil, `cargo clippy --workspace --all-targets` temiz. Root `Cargo.toml`'a
   tokio `"process"` feature'ı eklendi (`tokio::process::Command` için — hem backend hem SDK'yı
   etkiler ama zararsız/katkısız). SDK Cargo.toml'a `base64` eklendi (backend'de zaten vardı, SDK'da
   yoktu).
10. **Roadmap:** 2.2'nin "SDK'da video + klonlanmış ses birleştirme" ve 2.3'ün "İkili yayın" satırları
    `[x]`'e çevrildi, "2.3'ten önce" bulgu notu "ÇÖZÜLDÜ" olarak güncellendi.

**Kasıtlı kapsam dışı (bu oturumda da):** buluttan mux (backend'e video-ingest + `core::rtmp`'in
paylaşılan crate'e çıkarılması gerektiriyor — ayrı görev), uzun gecikmede orijinal mikrofona düşme
(kullanıcı kararıyla elendi), ducking/crossfade, dashboard'da mux/ffmpeg durum göstergesi
(`TargetStatus` bugün hiçbir yerde backend'e/dashboard'a sürülmüyor — mux'tan bağımsız, önceden var
olan bir eksiklik), OBS bağlantı kopma/yeniden bağlanma döngüsü (`accept_and_relay` bugün tek
seferlik, değiştirilmedi), İkili yayın bant genişliği uyarı metni güncellemesi (≥25 Mbps — henüz
dokunulmadı), `RelayTarget.dub_language`'in dashboard'da GERÇEKTEN seçilebilir olması (backend/SDK
şeması hazır ama UI'da yok — ayrı, küçük bir iş).

### Sonraki Adım (TAM OLARAK)
1. **`RelayTarget.dub_language` dashboard UI'ı** — şema hazır, backend/SDK çalışıyor, ama yayıncı
   panelden bir hedefe "bu hesap dublaj hesabı, şu dili alsın" diye işaretleyemiyor henüz
   (`sdk-stream-key-panel.tsx` bugün tek bir sabit "obs" hedefi yönetiyor, çoklu hedef editörü yok).
2. İkili yayın bant genişliği uyarısı (≥25 Mbps).
3. Buluttan mux (backend RTMP-ingest + `core::rtmp` paylaşımı) — ayrı, büyük bir görev.
4. Diğer her şey değişmedi: whisper.cpp/XTTS native motor (cmake yok), self-hosted S2S/ElevenLabs
   bağımsızlığı, Task 2.7 (admin routing gerçek toggle), 2.6/2.5/2.4 kalanı.

### Önceki (SDK ses wiring) oturumu — aşağıda, kronolojik olarak ÖNCEKİ

### Bu oturum — Task 2.3'e girişim, mimari bulgu, kapsam değişti, sonunda gerçek bir ön koşul kodlandı
Kullanıcı **"2.3 ü yap"** dedi (İkili Yayın — TR ses → hesap 1, EN klonlanmış ses → hesap 2). İki bulgu
kapsamı tamamen değiştirdi:
1. **Mux motoru yok** — `sdk/src/core/mux.rs` hâlâ sadece `OutputPlan` enum'u; RTMP relay (`rtmp/relay.rs`)
   her hedefe AYNI orijinal sesi opak byte olarak fan-out ediyor, hedef başına farklı ses (dub_language)
   göndermenin fiziksel yolu yok. Kullanıcıya bunu **açık açık "mux ne demek" dahil** anlattım (kullanıcı
   "mux ne anlamadım" dedi, mux/demux kavramını örnekle açıkladım).
2. Kullanıcı mux için karar verdi: **"cihazda mux = az kredi, buluttan mux = çok kredi"** — yayıncı
   seçebilecek, buluttan mux'ta dublaj hedefinin videosu da (SADECE o hedef için) buluta gidecek. Bu,
   CLAUDE.md'nin "video buluta çıkmaz" kuralına istisna gerektirdiği için **CLAUDE.md'yi güncelledim**
   (proje özeti + SDK Mimarisi bölümü — video artık "varsayılan olarak" lokal, ikili yayında opt-in
   "buluttan mux" istisnası var) + `docs/multistream.md`'ye "Mux modu" alt bölümünü ekledim (cihazda/
   buluttan, kredi farkı, Faz 3 Mod C ile ilişkisi).
3. Plan-mode'da 2.3'ü gerçekten kodlamaya girişirken İKİNCİ, daha büyük bir bulgu çıktı: **SDK'nın ses
   buluta gönderme istemcisi (`core::cloud::CloudClient`) hiçbir çalışan yere bağlı değildi.**
   `run_desktop_shell` sadece RTMP relay'i başlatıyordu; capture (`wasapi.rs`) → VAD/preprocess/framing
   (`gate.rs`) → `CloudClient` zinciri hiç kurulmamıştı (her parça izole test edilmişti ama birbirine
   bağlı değildi). Bu, roadmap'te 758. satırın NOT'unda zaten "ayrı iş" olarak işaretliydi. Kullanıcıya
   sordum: 2.3'ü yarım/yanıltıcı şema olarak mı bırakalım, yoksa önce bu ön koşulu mu bitirelim —
   **"Önce SDK ses wiring'ini bitirelim"** dedi.
4. **Kodlandı ve TAMAMLANDI:** yeni `sdk/src/core/cloud_session.rs` — `GET /api/translate/languages`
   çeker (relay_sync.rs deseni), `StreamConfig` kurar, `CloudClient` connect→authenticate→start_stream,
   `CapturePipeline`'ı (capture+preprocess+VAD, zaten var olan `gate.rs`) `spawn_blocking`'te çalıştırıp
   `tokio::mpsc` ile async tarafa köprüler, `tokio::select!` ile ses gönderip `Subtitle`/`UsageUpdate`/
   `Error` okur, kapanışta 500ms drain penceresiyle son altyazıyı kaçırmaz. `capture.rs`'e
   `impl CaptureSource for Box<dyn CaptureSource>` eklendi (generic pipeline boxed capture'ı kabul etsin
   diye). `run_desktop_shell` (lib.rs) relay başladıktan sonra bunu ayrı `tokio::spawn` ile başlatıyor
   (stt_control'ün "logla+yut, join etme" duruşuyla aynı — capture açılamazsa/Linux-Mac'te sadece loglar,
   RTMP relay'i etkilemez). `sdk/tests/cloud_session_wiring.rs` (2 yeni e2e, gerçek backend'e karşı):
   tam ses→altyazı döngüsü (MockCapture, 1 konuşma karesi→1 altyazı) + dil seçilmemişse NoTargetLanguages
   reddi. Roadmap 2.0'ın sonuna yeni checkbox + 758'in NOT'u "ÇÖZÜLDÜ" olarak güncellendi.
5. **Regresyon:** `cargo build -p streamdubber-sdk` temiz, `cargo test --workspace` → backend 301 unit +
   sdk 109 unit (yeni 3 test dahil: `ws_url_converts_http_schemes`, `ws_url_passes_through_already_ws_schemes`,
   `generate_session_id_is_unique_across_calls`) + tüm e2e suite'ler (yeni `cloud_session_wiring.rs` 2/2
   dahil) yeşil. `cargo clippy --workspace --all-targets` temiz. Windows cross-check (`x86_64-pc-windows-gnu`)
   bu makinede `x86_64-w64-mingw32-gcc` eksik olduğu için DENENEMEDİ — ortam eksikliği (mingw-w64
   kurulu değil), koddan kaynaklanmıyor, önceki oturumların cross-check'i bu ortamda artık tekrarlanamıyor.
   Frontend'e dokunulmadı (bu oturum saf backend/SDK).
6. **Task 2.3'ün kendisi (gerçek mux) hâlâ kodlanmadı** — sadece ön koşulu (SDK ses wiring) bitti ve
   mux kararı (cihazda/buluttan) CLAUDE.md+docs'a işlendi. Mux'un GERÇEK implementasyonu (ffmpeg
   sidecar önerisi — bu makinede `ffmpeg` binary'si KURULU, cmake gibi bir blocker YOK, ama backend'e
   yeni video-ingest transport + SDK'da ffmpeg process orkestrasyonu gerektiren büyük bir iş) ayrı bir
   oturumda ele alınmalı — henüz sorulmadı/planlanmadı.

### Sonraki Adım (TAM OLARAK)
1. **Task 2.3'ün gerçek mux implementasyonu** — cihazda mux (SDK ffmpeg sidecar) + buluttan mux
   (backend'e yeni video-ingest + ffmpeg + RTMP publish) ayrı ayrı tasarlanmalı. `ffmpeg` bu makinede
   kurulu (native derleme blocker'ı yok) ama iş büyük — kapsam kullanıcıyla netleştirilmeli (hangisi
   önce, ikisi birden mi).
2. `RelayTarget.dub_language` alanı henüz eklenmedi (mux olmadan yanıltıcı olurdu, kasıtlı).
3. Diğer her şey önceki oturumdan değişmedi: whisper.cpp/XTTS native motor (cmake yok), self-hosted
   S2S/ElevenLabs bağımsızlığı, Task 2.7 (admin routing gerçek toggle), 2.6/2.5/2.4 kalanı.

### Önceki oturum (2026-07-06, doğrulama oturumu sonu) — aşağıda, kronolojik olarak ÖNCEKİ

### Bu oturum — SADECE doğrulama + git senkronu, KOD YAZMADI
Önceki (uygulama) oturumu bittikten sonra kullanıcı **"@CLAUDE.md @docs/sessions/claude.md şuanki
durumu analiz et"** dedi. Bu oturumda yapılanlar:
1. Aşağıdaki "BU OTURUMDA TAMAMLANAN"/"Regresyon durumu" bölümlerindeki HER iddia dosya okunarak +
   testler YENİDEN çalıştırılarak doğrulandı: `sdk/src/core/mux.rs` hâlâ sadece `OutputPlan` enum'u
   (gerçek kod yok), `backend/src/relay/mod.rs::RelayTarget`'ta `dub_language` alanı hâlâ yok,
   `VoiceSampleStore`/`DubbedAudioTrack`/`synthesize_dubs`/`tts_audio_seconds` gerçekten var ve
   `state.rs`/`main.rs`/`ws/protocol.rs`'e wire edilmiş. **Hiçbir iddia stale çıkmadı.**
2. `cargo test --workspace` (backend 301 unit + sdk 106 unit + tüm e2e suite'ler — `voice_models_api.rs`
   8/8, `ws_integration.rs` 7/7 dahil) ve `cargo clippy --workspace --all-targets` YENİDEN çalıştırıldı
   → ikisi de yeşil/0 uyarı. Frontend `tsc --noEmit` / `npm run lint` / `npm run test` (121/121) /
   `npm run build` (35 sayfa) YENİDEN çalıştırıldı → hepsi yeşil.
3. Kullanıcı **"main'i fast-forward et"** dedi → ana checkout'ta (`/home/adem/Projects/GitHub/
   Stream-translator-live`, `main` orada checked-out) `git merge --ff-only worktree-task-2.2-realtime-tts`
   çalıştırıldı (önce `git status` ile temiz olduğu doğrulandı). **`main` (`4cb01cb`) bu worktree
   branch'ine (`73ef3c7`) fast-forward edildi — artık ikisi AYNI commit'te, divergence yok.**
   `origin/main`'e push YAPILMADI (kullanıcı istemedi) — origin'e göre şu an 3 commit ileride.
4. Auto-memory'de 2 gün önceki bir not ("dashboard/SDK tamamen mockup, backend'e bağlı değil")
   `sdk-pairing-panel.tsx`/`sdk-stream-key-panel.tsx`/`voice-clone-studio.tsx` içinde gerçek
   `fetch`/`onClick`/`useState` bulunarak stale çıktı ve "superseded" olarak güncellendi (bu repo'nun
   dosyası değil, global `~/.claude/.../memory/` — sadece bilgi için not düşülüyor).
5. Aşağıdaki "Sonraki Adım #1" o sırada "commit henüz atılmadı" diyordu — bu artık YANLIŞ/stale
   (o commit, yani `73ef3c7`, zaten atılmıştı). Aşağıda düzeltildi, DONE olarak işaretlendi.

**Bu oturumda kod/task ilerlemesi YOK.** Sıradaki gerçek iş hâlâ değişmedi: "Sonraki Adım #2" —
mux/audio-splice kullanıcı kararı (aşağıya bkz., **hâlâ sorulmadı**).

### Önceki (uygulama) oturumunun anlatısı — hâlâ geçerli, kronolojik olarak ÖNCEKİ oturuma ait
Kullanıcı isteği zinciri (hepsi aynı gün): **"@CLAUDE.md roadmapte faz 2 görevlerinden devam et"**
→ **"Faz 2 yi tamamla — elinde olmayan bir şey olunca admin paneline benim veri girişimi bekle,
windows veya linux gerçek testler için beni bekle, admin panelde olmalı ama her şey"** → (task 2.2
yarım kalınca context handoff notu yazıldı) → yeni bir oturumda kullanıcı önce büyük bir tasarım
genişletmesi (bkz. aşağıda "Öz Kaynaklı Ses Klonlama" özeti) yazdı, sonra **"S2S en sona not olarak
atansın onun dışını faz 2 ye ekleyip faz2yi ve sessiondaki taskları bitirmeye başla"** dedi — yani
genişletmenin S2S/self-hosted-motor-özel kısmı roadmap'in sonunda bir araştırma notu olarak kalsın,
GERİ KALANI (dil ataması şeması, şifreli örnek saklama, admin ataması, kota) Faz 2'ye (2.1/2.2)
girip HEMEN kodlansın. Bu oturum tam olarak bunu yaptı — bkz. "BU OTURUMDA TAMAMLANAN" altında.

Anlaşılan çalışma politikası (kullanıcı onayladı, tüm oturum boyunca izlendi):
- Kod tarafında yapılabilecek HER ŞEYİ yap (gerçek API entegrasyonu, gerçek
  endpoint, gerçek test — mock/placeholder DEĞİL).
- Gerçek API key/secret gerekiyorsa: admin panelden girilebilir hale getir
  (zaten var olan `/admin/secrets` genel mekanizmasına ekle), kendin key'e
  sahip değilsin, mock-fallback ile test et.
- Native derleme gerektiren şeyler (whisper.cpp, XTTS-v2, self-hosted S2S — hepsi cmake
  ister, bu makinede cmake YOK) → ERTELE, roadmap'e açıkça "KALAN" notu düş.
- Gerçek donanım/OBS/Windows testi gerekenler → kullanıcı aksiyonu olarak işaretle.
- Mimari çatal niteliğindeki kararlar (whisper.cpp/cmake kurulumu, mux/audio-splice tasarımı gibi)
  varsayımla geçilmez — kullanıcıyla AYRI görüşülür.
- Her task: TDD, ayrı commit, workspace regresyon (`cargo test --workspace` +
  `cargo clippy --workspace --all-targets` + frontend `tsc/lint/test/build`)
  yeşilken commit'le. Roadmap'teki `[ ]` → `[x]` + `<!-- Claude TARIH: ... -->`
  detaylı not.

### Önceki oturumda TAMAMLANIP COMMIT'LENEN (main branch, hepsi yeşil regresyonla)
```
6e2937a docs: oturum notu — Faz 2 durumu + task 2.2 tasarım kararları (context handoff)
c65a947 2.2b: TTS kaynağı Bulut/Cihazında — 2.6b'nin birebir aynısı
4bcad2a 2.1: Ses klonlama — ElevenLabs gateway, stüdyo, admin onay kuyruğu
e25311c 2.8: /dashboard/sdk — gerçek pairing kodu + stream key wiring
fe0bb1b 2.0: DeepL dil matrisi genişletme — tam katalog, admin+yayıncı seçimi, auto-detect
ff379ac roadmap: Faz 2 görev detaylandırma (önceki oturumdan commit'lenmemiş not)
9c84f9d 2.6b: tray döngüsüne periyodik STT-control poll wiring
d4cc7e0 2.6b: model indirme + SHA-256 doğrulama — sdk model_fetch
605661b 2.6b: GPU/VRAM yoklama — nvidia-smi tabanlı donanım probe'u
```

### BU OTURUMDA TAMAMLANAN — Task 2.2 (Gerçek Zamanlı TTS) + kullanıcının genişletme kararı
Önceki oturumun context handoff notunda sorduğu 2 soru zaten yanıtlanmıştı (voice model dil ataması:
admin curation; dublaj tetikleme: toggle+Kick 2 hesap+kredi). Bu oturum ÖNCE kota biriminin ne
olacağını netleştirdi (kullanıcı: **"Saniye ama müşteriye saat başına fiyatlandırma"** — iç takip
saniye, ama plan/kota kullanıcıya hep saat cinsinden gösteriliyor, zaten mevcut `UsageTracker`
deseni bu), SONRA kullanıcı roadmap'e büyük bir "Öz Kaynaklı Ses Klonlama + Speech-to-Speech" bölümü
ekledi (ElevenLabs bağımlılığını kaldırma, ham örnek saklama, language_mode şeması, dublaj/S2S
toggle, seçili sesler, admin-konfigüre kredi sistemi) ve **"S2S dışındakini Faz 2'ye ekle"** dedi.
Sonuç: aşağıdaki HEPSİ TEK OTURUMDA kodlandı, test edildi, regresyon yeşil:

**1) `VoiceModel` şema genişletmesi (2.1'e eklendi):**
- `language_mode: LanguageMode` (`SingleToSingle`/`MultiToSingle`/`SingleToMulti`) — düz
  `language: String` yerine (kullanıcının "ikisine de uyumlu olsun" kararı). `SingleToSingle`/
  `MultiToSingle` = model tek bir hedef dile kilitli (N hedef = N ayrı model); `SingleToMulti` =
  ElevenLabs'in bugünkü gerçek davranışı, aynı model N hedef dilde yeniden kullanılabilir.
- `source_languages`/`target_languages: Vec<String>` — `target_languages` clone anında BOŞ başlar,
  admin `assign` ile doldurur (aşağıda).
- `mode: VoiceMode` (`Dubbing`/`SpeechToSpeech`, varsayılan `Dubbing`) + `source: VoiceSource`
  (`UserCloned`/`Preset`, varsayılan `UserCloned`) — ŞEMA hazır ama dashboard'da `SpeechToSpeech`/
  `Preset` SEÇİLEMEZ (ikisi de self-hosted motor gerektiriyor, KALAN — bkz. roadmap sonu).
- Ham ses örneği artık **AES-256-GCM şifreli saklanıyor** (`voice_models::VoiceSampleStore`, yeni
  dosya `sample_store.rs`) — önceki "hiç saklama" kararının BİLİNÇLİ tersi. `relay::
  RelayTargetStore` ile BİREBİR AYNI desen (tüm JSON blob'u tek parça şifreli, `VOICE_SAMPLES_FILE`,
  `main.rs`'te `streamdubber.voice_samples.v1` context'iyle key derive). Model metadata'sından
  (gizli değil) kasıtlı ayrı dosyada. `clone_voice` best-effort persist eder — şifreleme/disk hatası
  model oluşturmayı ENGELLEMEZ, sadece `has_stored_sample=false` kalır.
- `backend/tests/voice_models_api.rs`'e yeni e2e testler (language_mode doğrulama, assign akışı).

**2) Admin dil ataması (2.2'ye eklendi):**
- `VoiceModelStore::set_active_for_language(model_id, lang)` + `POST /admin/voice/models/{id}/
  assign` — model onaylı değilse 422, `SingleToSingle`/`MultiToSingle` zaten farklı bir dile
  atanmışsa 409 (bu modlar tek hedefe kilitli), `SingleToMulti` çoklu dile atanabilir.
  `active_model_for(user_id,lang)` admin modeli SONRADAN reddederse stale atamayı görmezden gelir
  (status filtresi — atama haritası ayrıca temizlenmiyor, gerek yok).
- `admin-voice-models-panel.tsx`: dil sütunu (language_mode + source→target özeti) + onaylı
  modeller için dil seçici + "Assign" butonu.

**3) Gateway::synthesize → pipeline dispatch (2.2'nin çekirdeği):**
- `pipeline::StreamSession.synthesize_dubs` — final (`is_final=true`) çeviri sonrası, `tts_source=
  Cloud` VE hedef dil için aktif+onaylı model varsa `gateway.synthesize` GERÇEKTEN çağrılır. Sonuç
  `ws::protocol::ServerMessage::Subtitle.dubbed_audio` (yeni alan — `Vec<DubbedAudioTrack{lang,
  audio_base64}>`, `translations` ile paralel, eski SDK'lar serde default ile yok sayar) içine
  eklenir. Model yoksa/sağlayıcı hata verirse o dil sessizce atlanır (`translate_all`'ın best-effort
  deseniyle aynı) — bir dilin dub hatası ne diğer dilleri ne altyazının kendisini etkiler.
  `tts_source=Local`'de backend HİÇ sentezlemez (SDK kendi motoruyla sentezleyecek, motor KALAN).
- 6 yeni pipeline testi (dispatch var/yok, local hiç dispatch etmiyor, interim hiç dispatch etmiyor,
  sağlayıcı hatası fatal değil, kota gerçekten işleniyor).
- "2 sn altı" gecikme hedefi CANLI ElevenLabs key'i olmadan ÖLÇÜLEMEDİ — dispatch kodu gerçek ve
  testli, uçtan uca gecikme doğrulaması kullanıcı aksiyonu.

**4) TTS kota/kredi tüketimi (2.2'ye eklendi, kullanıcının "saniye ama saat-fiyatlandırma" kararı):**
- `gateway::elevenlabs` artık `output_format="mp3_44100_128"` AÇIKÇA istiyor (tahmini karakter/
  saniye oranı DEĞİL — kesin bilinen bitrate). `pipeline::tts_audio_seconds` bu sabitten
  `bytes*8/128_000` ile gerçek süreyi hesaplar, indirimsiz uygular (STT'nin `process_audio`
  1.0-tarife mantığıyla aynı — cloud sağlayıcı tam maliyetli). Aynı `UsageTracker` havuzuna
  (`usage.record`) işler — STT ile PAYLAŞILAN, ayrı bir sayaç YOK (kullanıcının isteği buydu).
  Limit kontrolü burada ayrıca yapılmıyor — aynı havuzu paylaştığı için STT'nin bir sonraki tick'i
  zaten yakalıyor (bir tick gecikmeli ama doğru, yeni bir "session kapatma" yolu icat edilmedi).
- Yerel TTS (`tts_source=local`) kullanım tüketimi KALAN — gerçek motor yok (2.2b), backend'e ses/
  karakter hiç ulaşmıyor, ölçülecek bir "TTS anı" henüz yok.

**5) Frontend:**
- `voice-clone-studio.tsx`: `/api/translate/catalog`'tan (2.0) beslenen dil kataloğu + 3 modlu
  language_mode seçici + moda göre tekli/çoklu kaynak-dil seçici; `POST /api/voice/clone` artık
  `language_mode`+`source_languages` gönderiyor (backend şemasıyla birlikte güncellendi — 422
  riskinden kaçınıldı). Model listesinde language_mode/source/target özeti gösteriliyor.
  `voice-cloning.test.ts`'e yeni testler (catalog fetch, request body alanları, 3 mod için dictionary
  kapsaması).

**6) Roadmap yeniden düzenleme (kullanıcının "S2S sona, gerisi Faz 2'ye" kararı):**
- 2.1/2.2'ye yukarıdaki maddeler `[x]` + detaylı Claude notu olarak eklendi.
- Dosya sonundaki "Öz Kaynaklı Ses Klonlama..." bölümü TRIM edildi: language_mode şeması, şifreli
  saklama, mode/source alanları, admin ataması, kota — hepsi "Faz 2'ye Taşınanlar" notuyla
  çıkarıldı. Kalan (gerçekten ayrı, hâlâ faza atanmamış): gerçek S2S sentez motoru (metin ara
  katmanı olmadan çalışan ses-dalgasından-ses-dalgasına model), ElevenLabs bağımlılığının
  kaldırılması, self-hosted klonlama+TTS motoru, "seçili sesler" (self-hosted çıktısı gerektiriyor),
  çok-sağlayıcılı kredi/kota admin ekranı (bugün tek sağlayıcı olduğu için erken).

### Regresyon durumu (bu oturum sonu)
- **Backend:** `cargo build` temiz, `cargo test --workspace` → 301 unit + tüm e2e testler (yeni
  `voice_models_api.rs` testleri dahil) yeşil, `cargo clippy --workspace --all-targets` temiz.
- **Frontend:** `npx tsc --noEmit` temiz, `npm run lint` temiz, `npm run test` → 121 test yeşil,
  `npm run build` başarılı (35 sayfa).
- `sdk` crate'i bu oturumda DEĞİŞMEDİ (TTS dispatch tamamen backend tarafında, `tts_source=local`
  hâlâ protokol-seviyesinde taşınıyor, SDK motoru KALAN).

### Kasıtlı Yapılmayanlar (bu oturumda, gerekçesiyle — hiçbiri unutulmadı)
1. **SDK'da video + klonlanmış ses birleştirme (mux)** — `sdk/src/core/mux.rs` hâlâ sadece bir enum
   (`OutputPlan`), gerçek kod yok. Gerekçe: OBS'in orijinal ses tag'lerini bu oturumda üretilen
   `dubbed_audio` ile GERÇEKTEN değiştirmek yeni bir encoder+senkron stratejisi gerektiriyor —
   whisper.cpp/cmake sorusuyla AYNI ciddiyette bir mimari çatal (AAC pass-through mı, ffmpeg
   sidecar mı, Faz 3'e mi ertelenir). Kullanıcıyla AYRI görüşülmeden koda dökülmemeli — bu oturumda
   hiç SORULMADI (kapsam/zaman nedeniyle, kasıtlı erteleme). **Sonraki oturumun İLK işi bu olmalı.**
2. **Gerçek Speech-to-Speech motoru + ElevenLabs bağımlılığının kaldırılması** — roadmap'in sonunda
   ayrı bir araştırma bölümünde duruyor (kullanıcının kendi "S2S sona not olarak kalsın" kararı).
   Gerekçe: native motor derlemesi (bu makinede cmake yok) + STT→çeviri→TTS'ten TAMAMEN farklı bir
   pipeline dalı (metin ara katmanı olmayan ses-dalgasından-ses-dalgasına akış) gerektiriyor.
3. **`VoiceModel.mode=SpeechToSpeech` / `source=Preset` dashboard'da seçilemez** — şema hazır (madde
   2'ye bağlı — bkz. yukarıda), ama UI'da SUNULMADI çünkü seçilse bile hiçbir şey yapmazdı (motor
   yok, seçenek yanıltıcı olurdu).
4. **Yerel TTS (`tts_source=local`) kullanım/kota tüketimi** — gerçek local motor (2.2b'nin KALAN
   maddesi) olmadan backend'e ses/karakter hiç ulaşmıyor, ölçülecek bir "TTS anı" yok. Cloud yol
   (bu oturumda kodlanan) tam çalışıyor, local yol bilerek dokunulmadı.
5. **Admin panelde çok-sağlayıcılı kredi/oran konfigürasyon ekranı** — bugün TEK cloud sağlayıcı
   (ElevenLabs, sabit 1:1 oran) olduğu için karşılaştırılacak ikinci bir oran yok; inşa etmek
   spekülatif olurdu (CLAUDE.md'nin "hipotetik gelecek için tasarlama" ilkesine aykırı düşerdi).
6. **whisper.cpp / XTTS-v2 / self-hosted S2S native motor bağlama** — bu makinede `cmake` yok
   (Arch/CachyOS, kurulu değil); kurulum önerilirse kullanıcıya AYRICA sorulmalı, bu oturumda
   sorulmadı (önceki oturumdaki "önce diğerini bitir" kararı hâlâ geçerli sayıldı).
7. **"2 sn altı" TTS gecikme hedefinin canlı ölçümü** — kod yolu gerçek ve 6 yeni testle doğrulandı,
   ama canlı ElevenLabs key'i/gerçek ağ gecikmesi olmadan uçtan uca SÜRE ölçülemedi — kullanıcı
   aksiyonu (aynı şekilde DeepL EN/PT bölgesel kod nüansı da önceki oturumdan beri canlı key
   bekliyor).
8. **Task 2.3 (İkili Yayın, `RelayTarget.dub_language`)** — 2.2'nin dispatch mekanizması artık hazır
   olduğu için daha az belirsiz, ama bu oturumda YAPILMADI — sıradaki iştir, henüz kodlanmadı.

### Sonraki Adım (TAM OLARAK)
1. ~~Roadmap'te 2.1/2.2'nin yeni checkbox'larını + trim edilen "Öz Kaynaklı..." bölümünü commit'le~~
   — **YAPILDI** (`73ef3c7`). Doğrulama oturumunda `main`'e de fast-forward edildi — iki branch
   şu an aynı commit'te, bu adımda bekleyen bir şey YOK.
2. **[CHECKPOINT — SIRADAKİ GERÇEK İŞ] Task 2.2'nin TEK kalan maddesi: SDK'da video + klonlanmış ses birleştirme (mux)**
   — `sdk/src/core/mux.rs` şu an SADECE bir enum (`OutputPlan`, gerçek kod YOK). RTMP relay
   (`sdk/src/core/rtmp/relay.rs`) audio/video tag'lerini OPAK byte-blob olarak fan-out ediyor,
   codec'e dokunmuyor. OBS'in orijinal ses tag'lerini bu oturumda üretilen `dubbed_audio` ile GERÇEKTEN
   değiştirmek yeni bir encoder+senkron stratejisi gerektirir — whisper.cpp/cmake sorusuyla AYNI
   ciddiyette bir mimari çatal (basit AAC pass-through encoder mı, ffmpeg sidecar mı, Faz 3'e mi
   ertelenir). **AskUserQuestion ile AYRI görüşülmeli, bu oturumda sorulmadı/kodlanmadı.**
3. **Task 2.3 (İkili Yayın)** — `RelayTarget`'a `dub_language: Option<String>` eklemek (kullanıcının
   önceki oturumda verdiği "Kick'te dublaj için 2 hesap" kararı) — 2.2'nin dispatch mekanizması
   hazır olduğu için artık daha az belirsiz, muhtemelen aynı oturumda gider.
4. **Task 2.7** (Admin — hibrit kontrol genişletme + canlı maliyet) — `/admin/routing` sayfasının
   STT/TTS/Translate provider toggle'ları hâlâ TAMAMEN statik/dekoratif (onClick yok, DeepL kartı
   hariç). Bu task o sayfayı gerçek `PUT /admin/config`'e bağlamalı. Ayrıca "canlı maliyet
   göstergesi" — `UsageTracker` üzerine config-seeded $/birim tablosu (çok-sağlayıcılı kredi
   konfigürasyonu da muhtemelen buraya girer, bkz. roadmap sonu).
5. **Task 2.6** (faster-whisper bulut) — sadece `gateway/whisper_cloud.rs` (deepgram.rs deseni,
   self-hosted REST endpoint) + admin toggle. Hetzner GPU sunucu provisioning + TR kalite testi
   KULLANICI AKSİYONU.
6. **Task 2.5** (Chrome Extension) — `extension/js/shared.js` Twitch-agnostic, doğrudan taşınabilir.
   Chrome Web Store başvurusu KULLANICI AKSİYONU.
7. **Task 2.4 kalanı** — Twitch console kaydı/Hosted Test/review başvurusu, sertifikalar,
   releases.strovak.com hosting, Hetzner — HİÇBİRİ kodla tamamlanamaz.

## Değiştirdiğim Alanlar (bu oturum — dosya/klasör bazında özet)
**Backend (Rust):**
- `backend/src/voice_models/sample_store.rs` (yeni) — `VoiceSampleStore`, AES-256-GCM şifreli ham
  örnek deposu (`relay::RelayTargetStore` deseni).
- `backend/src/voice_models/mod.rs` — `LanguageMode`/`VoiceMode`/`VoiceSource` enum'ları,
  `VoiceModel` şeması genişletildi, `set_active_for_language`/`active_model_for`, `assign_voice_model`
  handler'ı, `clone_voice` request/validasyon güncellendi.
- `backend/src/pipeline/mod.rs` — `StreamSession.voice_models` alanı, `synthesize_dubs`,
  `tts_audio_seconds`, `subtitle_for` artık `dubbed_audio` üretiyor.
- `backend/src/ws/protocol.rs` — `DubbedAudioTrack` + `ServerMessage::Subtitle.dubbed_audio`.
- `backend/src/ws/mod.rs` — `StreamSession::new` çağrısına `voice_models` eklendi.
- `backend/src/gateway/elevenlabs.rs` — `output_format="mp3_44100_128"` açıkça isteniyor.
- `backend/src/state.rs`, `backend/src/main.rs`, `backend/src/app.rs` — `VoiceSampleStore` wiring'i
  (state field+FromRef+skeleton, main.rs encrypted-store construction, `/admin/voice/models/{id}/
  assign` route), `overlay/mod.rs` ve `ext_twitch/mod.rs`'teki `ServerMessage::Subtitle` literal'leri
  yeni `dubbed_audio` alanı için güncellendi.
- `backend/tests/voice_models_api.rs` — language_mode validasyon + assign akışı e2e testleri.

**Frontend (Next.js):**
- `voice-clone-studio.tsx` — language_mode + kaynak dil seçici, `/api/translate/catalog` fetch,
  model listesinde dil özeti.
- `admin-voice-models-panel.tsx` — dil sütunu + assign seçici/butonu.
- `src/i18n/dictionaries/en.json` — `dashboard.voice.form`/`modelMeta` ve `admin.voiceModels` yeni
  alanlar.
- `src/i18n/dictionaries/voice-cloning.test.ts` — yeni testler.

**Roadmap/docs:**
- `streamdubber_roadmap_final.md` — 2.1/2.2 checkbox'ları + notlar; "Öz Kaynaklı Ses Klonlama..."
  bölümü trim edildi (yalnız gerçek S2S/self-hosted-motor kısmı kaldı) + Watch Party bölümünün
  sondaki notunun yanlış yerde kalması düzeltildi (araya girmişti).

## Son Kararlar (bu oturum + önceki oturumdan taşınanlar)
- **Kota birimi: saniye (iç), saat (kullanıcıya gösterilen)** — kullanıcının net kararı. Cloud TTS
  gerçek ses baytından KESİN hesaplanır (tahmini karakter/saniye oranı DEĞİL — `output_format`
  açıkça istendiği için). Bu, mevcut `UsageTracker`/`monthly_hours` deseniyle bire bir örtüşüyor,
  yeni bir "kredi" soyutlaması İCAT EDİLMEDİ (kodda "kredi" diye bir şey hâlâ yok — kullanıcının
  "kredi mantığı" dediği şey, zaten var olan saat havuzunun ta kendisi).
- **Mimari desen: "mirror, generalize etme."** Her STT-analog TTS parçası (TtsSettingsStore/
  SttSettingsStore, tts_policy/stt_policy modülleri vb.) PARALEL DUPLICATE olarak yazıldı, ortak
  soyutlamaya ÇEKİLMEDİ — roadmap metninin kendisi "aynı desen" diyor, "aynı kod" demiyor.
- **whisper.cpp, XTTS-v2 VE self-hosted S2S native motor bind'i ERTELENDİ** — bu makinede `cmake`
  yok (Arch/CachyOS, kurulu değil). Üçü de AYNI blocker kategorisi; kullanıcı "önce diğer parçaları
  bitir" dedi (whisper.cpp sorusu, önceki oturum) — self-hosted S2S için de aynı mantık, ayrıca
  sorulmadı çünkü kullanıcının kendisi "S2S sona not olarak kalsın" dedi.
- **`VoiceModel.mode`/`source` şemada var ama dashboard'da sadece `Dubbing`/`UserCloned` seçilebilir**
  — `SpeechToSpeech`/`Preset` gerçek bir self-hosted motor gerektiriyor, o olmadan UI'da sunmak
  yanıltıcı olurdu (çalışmayan bir seçenek gibi görünürdü).
- **Admin panelde çok-sağlayıcılı kredi/oran konfigürasyonu YAPILMADI** — bugün TEK cloud sağlayıcı
  (ElevenLabs, sabit 1:1 oran) olduğu için karşılaştırılacak ikinci bir oran yok; self-hosted motor
  gelince (2.7 veya ayrı bir admin ekranı) anlamlı olacak.
- **Bu makinede GERÇEK donanım var:** NVIDIA RTX 3070 Laptop GPU (8GB), `nvidia-smi` çalışıyor.
- **`ws_integration.rs` ve `sdk/tests/cloud_e2e.rs`'teki bazı testler "free" plandan "pro"ya
  çekildi** (önceki oturum) — 2.0'daki plan-bazlı hedef-dil sınırı testleri artık haklı reddediyordu.
- **`run_desktop_shell` (SDK'nın gerçek giriş noktası) HÂLÂ gerçek bir `stream_start` kurmuyor** —
  sadece relay wiring + opsiyonel stt-control polling loop var. TTS dispatch bu oturumda TAMAMEN
  backend tarafında kodlandı ve test edildi (pipeline unit testleri) ama SDK'nın gerçek bir yayın
  sırasında bunu uçtan uca tetiklediği hiç gösterilmedi — ayrı, büyük bir iş.

## Blokerlar / Dikkat
- **cmake yok** (bu makine) — whisper.cpp, XTTS-v2 VE self-hosted S2S native bind'ini engelliyor.
  Kurulum önerilirse kullanıcıya sorulmalı (sistem paketi, onay gerekir).
- **Mux/audio-splice tasarımı ayrı bir kullanıcı görüşmesi gerektirir** (bkz. "Sonraki Adım" #2).
  Bu konuda AŞIRI VARSAYIM yapıp direkt kod yazmaya BAŞLAMA.
- **`/admin/routing` sayfası** DeepL kartı hariç TAMAMEN statik mockup — BİLİNEN, task 2.7'ye atanmış.
- **`VoiceModel` şeması artık NİHAİ** (bu oturumda tamamlandı) — ileride `mode=SpeechToSpeech`/
  `source=Preset`'i gerçek anlamda kullanılabilir hâle getirmek şemayı DEĞİŞTİRMEZ, sadece yeni bir
  sentez pipeline'ı/motor bağlar. Frontend/backend `clone_voice` request şekli artık senkron —
  ikisi ayrı güncellenirse yine 422 riski var, unutma.
- **Yerel TTS (tts_source=local) kullanım tüketimi HÂLÂ YOK** — gerçek motor gelmeden ölçülecek
  bir "TTS anı" yok, bilerek ertelendi.
- Workspace regresyon komutları (HER commit'ten önce mutlaka koş):
  ```bash
  cargo test --workspace && cargo clippy --workspace --all-targets
  cd frontend && npx tsc --noEmit && npm run lint && npm run test && npm run build
  ```

---
---

## Faz 1 arşivi (eski oturumlar — aşağıdaki içerik güncel değil, referans için tutuluyor)

## Faz 1.3 — Komple (Codex iskeletinin üstüne)
- core/preprocess.rs — DC giderme + pre-emphasis; gate'e opsiyonel adım
- core/cloud.rs — CloudClient (ws/wss), auth→stream_start→audio→subtitle; backend'e karşı e2e test
- core/pairing.rs — 6 haneli kod doğrulama + PairingRequest
- core/mux.rs — Faz1 Mod A çıktı planı (video lokal, sadece ses buluta)
- integrations/obs.rs — obs-websocket v5 mesajları (Identify, SetInputSettings)
- tauri.conf.json — updater + bundle signing config; docs/sdk-distribution.md
- sdk/tests/cloud_e2e.rs — backend dev-dep ile gerçek interop (2 test)

## Faz 1.3 gerçek vs iskelet (dürüstlük)
- GERÇEK+TEST: preprocess, VAD, capture abstraction, cloud client + e2e, pairing/OBS mesaj tipleri.
- İSKELET/CONFIG (sonraki tasklar): gerçek WASAPI cihaz I/O (Windows), canlı OBS bağlantısı, backend pairing endpoint (1.4), mTLS client-cert connector, sertifikalar + release CI + updater runtime init, gerçek video mux (Faz 2).

## Faz 1.3 — Yapılan
- Codex'in 1.3 task-1'i (Tauri kurulumu + platform-bağımsız çekirdek) doğrulandı: builds, 7 test, clippy temiz.
- `core/capture.rs` — CaptureSource trait + CaptureConfig (16k mono, 20ms frame) + MockCapture
- `audio/open_capture` — platform dispatch; WASAPI aktif nokta (NotImplemented stub, Windows'ta yazılacak), CoreAudio/PipeWire iskelet
- `core/vad.rs` — RMS enerji VAD + hangover (VadGate), boş ses tespiti
- `core/gate.rs` — CapturePipeline: capture→VAD→backend frame, sessizlik dropped (STT maliyeti ↓), GateStats

## Faz 1.3 Açık / Dikkat
- Gerçek WASAPI cihaz I/O yazılmadı (Linux'ta test edilemez) — capture trait + MockCapture ile akış tam test edildi; cihaz binding Windows'ta (cpal/windows crate).
- Sıradaki: gürültü temizleme/ön-işleme, video+ses birleştirme, bulut WebSocket client (sadece ses).
- ÖNEMLİ — multi-agent: SDK çalışma ağacında commit'siz. Codex iskeleti + Claude'un capture/vad/gate eklemeleri + workspace Cargo.toml member + backend dosyalarında rustfmt diff'leri karışık. Commit koordinasyonu kullanıcı/Codex ile yapılmalı.

## Faz 1.2 — Yapılan
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

## Faz 1.1 — Yapılan
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

## Faz 1 Son Kararlar
- STT/DeepL gerçek entegrasyonu YOK (1.7/1.8 taskları) — şimdilik MockStt/MockTranslate, aynı trait'lerle takılacak.
- session_ready.remaining_seconds = -1 (bilinmiyor/sınırsız) — usage tracking 1.11'de gelince hesaplanacak.
- Transit AES yok (TLS 1.3 kararıyla uyumlu); JWT secret env'den, koda gömülmedi.
- Transit şifreleme: TLS 1.3 yeterli, üstüne AES eklenmez. AES-256 sadece at-rest (DB, ses modelleri, lokal stream key'ler).
- RTMP relay kesinlikle Faz 2 — Faz 1'de yayıncı kendi Restream/Castr hesabını bağlar. (NOT: sonradan roadmap değişti, RTMP relay Faz 1.6'ya alındı — bu karar GEÇERSİZ, güncel roadmap'e bak.)
- WebSocket binary frame: 8 byte header (magic + codec + len) + Opus payload.
- DB primary key: UUID. Refresh token DB'de hash olarak saklanır, JWT stateless.
- `service_config` tablosu başlangıç değerleriyle hazır — admin toggle için.

## Faz 1 Blokerlar / Dikkat (arşiv)
- Migration canlı Postgres'e karşı çalıştırılmadı (yerel DB yok) — SQL schema.md'den birebir, sqlx derleme zamanı gömme başarılı.
- Gerçek STT/DeepL key'leri yok — entegrasyon (1.7/1.8) için Deepgram/DeepL hesabı gerekli.
