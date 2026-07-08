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
1. **Bu worktree'deki commit'i push et + PR aç** — kullanıcı ya bu ortama
   GitHub kimlik bilgisi eklemeli ya da kendi makinesinde
   `git fetch && git checkout worktree-logical-twirling-harp` ile alıp
   push etmeli. Worktree/commit SİLİNMEDİ, olduğu gibi duruyor.
2. Bu iş, aşağıdaki (2026-07-06 tarihli, hâlâ güncel/bitmemiş) Task 2.2 TTS
   pipeline tasarımıyla TAMAMEN BAĞIMSIZ — birbirini etkilemiyor, aynı
   anda ilerleyebilir.

## Blokerlar / Dikkat (bu oturum)
- **Bu worktree'de commit var ama push edilmedi** (sandbox'ta GitHub
  kimlik bilgisi yok). Başka bir Codex/Claude oturumu bu değişikliği
  main'de GÖRMEYECEK — aynı dosyalarda (özellikle `en.json`,
  `streamdubber_roadmap_final.md`) tekrar çalışmaya başlamadan önce bu
  worktree/branch'in push edilip edilmediğini kontrol et.
- `frontend/src/i18n/dictionaries/en.json`, ana checkout'ta bu oturumdan
  BAĞIMSIZ olarak zaten uncommitted değişikliklere sahipti (legal sayfalar,
  macOS/Linux indirme durumu, voice cloning language_mode) — bu worktree
  o değişiklikleri İÇERMİYOR (worktree origin/main'den fresh branch'lendi).
  Push/merge sırasında `en.json`'da conflict beklenebilir, veri kaybı değil.

---

## YENİ KULLANICI İSTEĞİ (2026-07-06, en güncel — bu oturum sadece NOT DÜŞTÜ, kod YAZMADI)
Kullanıcı, ses klonlama/dublaj konusunda mevcut ElevenLabs-bağımlı tasarımı GENİŞLETME kararı verdi.
Tam detay + görev listesi roadmap'in EN SONUNA eklendi: bkz.
`streamdubber_roadmap_final.md` → "Öz Kaynaklı Ses Klonlama + Speech-to-Speech (Faz 3 sonrası /
araştırma)" bölümü. Özet (sonraki ajan roadmap'teki bölümü OKUMADAN kod yazmaya başlamasın):

1. Yayıncı dashboard'dan **"Dublaj" (klonlanmış ses+TTS) vs "Kendi Sesim" (S2S, metin ara katmanı
   yok, orijinal tonlama korunur)** arasında seçim yapabilmeli.
2. Eğitim için ses örneği artık **biz saklayacağız** (şu anki "ham örnek hiç saklanmaz" kararının
   BİLİNÇLİ tersine çevrilmesi) — yayıncıya panelden birkaç cümle okutulacak, **kendi yerel
   dilinde**, eğitim için teknik olarak en verimli sayıda cümle/saniye ne ise.
3. **ElevenLabs bağımlılığı kaldırılmalı** — kendi self-hosted klonlama+TTS/S2S motorumuz olmalı
   (native derleme gerektirir, whisper.cpp'deki cmake blocker'ıyla AYNI kategori — kullanıcıyla
   ayrı görüşülmeli).
4. **"Seçili sesler"** (bizim ürettiğimiz hazır ses kütüphanesi) seçeneği de olmalı — klonlama
   yapmadan hızlı başlangıç.
5. Self-hosted mod ElevenLabs'e göre **daha az kredi tüketmeli**.
6. **Kredi/kota tamamen admin panelden konfigüre edilebilir olmalı** (her mod/sağlayıcı için
   birim-kredi oranı, restart'sız, hard-coded değil).
7. **(2026-07-06, ek karar) `language` alanı yerine `language_mode` gelecek** — hem "hedef dil
   bazlı model" hem "kaynak dilden eğitilip çok dile sentezleyen model" senaryosuyla UYUMLU tek
   şema. Üç değerli enum: `SingleToSingle` (tek kaynak dili algılar, tek hedef dile çevirir — N
   hedef dil = N ayrı model), `MultiToSingle` (birden fazla kaynak dilini algılar, tek sabit hedef
   dile çevirir), `SingleToMulti` (tek kaynak dilinden eğitilir — ElevenLabs/XTTS-v2 gibi çok
   dilli motor sayesinde AYNI model birden fazla hedef dile sentez yapabilir; **şu anki ElevenLabs
   entegrasyonu bu moda en yakın**). Şema: `language_mode` + `source_languages: Vec<String>` +
   `target_languages: Vec<String>` (SingleToMulti'de boş = "hepsinde kullan"). Tam detay roadmap'teki
   bölümün madde 7'sinde.

**Önemli:** Bu, aşağıdaki "ŞU AN YARIM KALAN: Task 2.2" ile AYNI ŞEMAYA (`VoiceModel`) dokunuyor —
düz `language: String` alanı yerine artık yukarıdaki madde 7'deki üçlü (`language_mode`/
`source_languages`/`target_languages`) + `mode`/`source`/şifreli-örnek-referansı ekleniyor.
Sonraki ajan **ikisini birlikte** tasarlamalı, ayrı ayrı yapılırsa `VoiceModel` şeması iki kez
migrate edilir. Roadmap'teki yeni bölümdeki "Görevler" listesi + "Bu, Şu Anki Mimariyi Nasıl
Etkiliyor" alt başlığı doğrudan implementasyon sıralaması olarak kullanılabilir.

## Neredeyim? (GÜNCEL — 2026-07-06, bu oturumun sonu)
Kullanıcı isteği: **"@CLAUDE.md roadmapte faz 2 görevlerinden devam et"**, sonra
**"Faz 2 yi tamamla — elinde olmayan bir şey olunca admin paneline benim veri
girişimi bekle, windows veya linux gerçek testler için beni bekle, admin
panelde olmalı ama her şey"**.

Anlaşılan çalışma politikası (kullanıcı onayladı, tüm oturum boyunca izlendi):
- Kod tarafında yapılabilecek HER ŞEYİ yap (gerçek API entegrasyonu, gerçek
  endpoint, gerçek test — mock/placeholder DEĞİL).
- Gerçek API key/secret gerekiyorsa: admin panelden girilebilir hale getir
  (zaten var olan `/admin/secrets` genel mekanizmasına ekle), kendin key'e
  sahip değilsin, mock-fallback ile test et.
- Native derleme gerektiren şeyler (whisper.cpp, XTTS-v2 — ikisi de cmake
  ister, bu makinede cmake YOK) → ERTELE, roadmap'e açıkça "KALAN" notu düş.
- Gerçek donanım/OBS/Windows testi gerekenler → kullanıcı aksiyonu olarak işaretle.
- Her task: TDD, ayrı commit, workspace regresyon (`cargo test --workspace` +
  `cargo clippy --workspace --all-targets` + frontend `tsc/lint/test/build`)
  yeşilken commit'le. Roadmap'teki `[ ]` → `[x]` + `<!-- Claude TARIH: ... -->`
  detaylı not.

### Bu oturumda TAMAMLANIP COMMIT'LENEN (main branch, hepsi yeşil regresyonla)
```
c65a947 2.2b: TTS kaynağı Bulut/Cihazında — 2.6b'nin birebir aynısı
4bcad2a 2.1: Ses klonlama — ElevenLabs gateway, stüdyo, admin onay kuyruğu
e25311c 2.8: /dashboard/sdk — gerçek pairing kodu + stream key wiring
fe0bb1b 2.0: DeepL dil matrisi genişletme — tam katalog, admin+yayıncı seçimi, auto-detect
ff379ac roadmap: Faz 2 görev detaylandırma (önceki oturumdan commit'lenmemiş not)
9c84f9d 2.6b: tray döngüsüne periyodik STT-control poll wiring
d4cc7e0 2.6b: model indirme + SHA-256 doğrulama — sdk model_fetch
605661b 2.6b: GPU/VRAM yoklama — nvidia-smi tabanlı donanım probe'u
```
(Son 3'ü — 605661b/d4cc7e0/9c84f9d — 2.6b'nin bu oturumdan ÖNCEKİ leftover
maddeleriydi, "Faz 2'den devam et" isteğiyle bu oturumda bitirildi.)

**Regresyon durumu (c65a947 sonrası):** backend 285 unit + tüm e2e; sdk 106
unit + tüm e2e; frontend 119 vitest testi. clippy/lint/tsc/build hepsi temiz.

### ŞU AN YARIM KALAN: Task 2.2 (Gerçek Zamanlı TTS — pipeline wiring)
2.2b bitince sıradaki task 2.2'ye geçtim. Backend'in canlı pipeline'ına
(StreamSession) TTS synthesize çağrısını gerçekten bağlamak, tasarımda
mevcut bir örnek yok (STT'nin `stt_source` deseni gibi 1:1 kopyalanacak bir
şey yok) — bu yüzden kullanıcıya 2 soru sordum, YANITLADI, ama context
bitmeden HİÇBİR KOD YAZILMADI. Yanıtlar:

**Soru 1 — Hangi ses modeli hangi dilde kullanılsın?**
> Kullanıcı: "Admin panelde Voice model kullanabilmeli yayıncı dili
> seçtiğinde benim admin panelden o dil için seçtiğim en iyi modeli
> kullansın."

Yani: **"en son onaylanan modeli kullan"** (benim önerdiğim basit seçenek)
DEĞİL — kullanıcı **ikinci seçeneği** (dil alanı + admin curation) seçti:
- `VoiceModel`'e `language: String` alanı eklenmeli (şu an YOK).
- Admin panelde, bir kullanıcının (user_id, language) çifti için "bu model
  kullanılsın" diye SEÇEBİLECEĞİ bir mekanizma gerekiyor — muhtemelen
  `POST /admin/voice/models/{id}/assign` gibi bir uç + `VoiceModel`'e
  `is_active_for_lang: bool` benzeri bir alan (aynı user+lang için sadece
  biri aktif olabilir, admin manuel seçiyor).
- `voice_models::clone_voice` request'ine `language` alanı eklenmeli (şu an
  sadece `name` + `sample_base64` alıyor).
- Onay kuyruğu admin UI'ında (`admin-voice-models-panel.tsx`) dil sütunu +
  "bu dil için aktif yap" aksiyonu eklenmeli.

**Soru 2 — Dublaj ne zaman devreye girsin?**
> Kullanıcı: "Yayıncı açıp kapayabilecek zaten kickte dublaj için 2 hesap
> gerekecek orda her hesap için dublaj dili seçecek dublajda kredi yesin
> bütün herşey kredi mantığında iyiymiş."

Bu yanıt **hem 2.2'yi hem 2.3'ü (İkili Yayın) etkiliyor**, ve benim
sunduğum 2 basit seçenekten HİÇBİRİ tam karşılamıyor — üçüncü, daha büyük
bir tasarım ortaya çıktı:
- Yayıncı dublajı **açıp kapayabilmeli** (explicit toggle — muhtemelen
  `stream_start`'a yeni alan VEYA platform bağlantısı bazında bir ayar).
- **Kick'te dublaj için 2 hesap gerekiyor** — bu, 1.6/2.3'teki "İkili Yayın"
  konseptiyle birebir örtüşüyor (TR ses → hesap 1, EN klonlanmış ses →
  hesap 2). Şu an `RelayTarget{platform, rtmp_url, stream_key}`'de dil
  bilgisi YOK — her bağlı hesaba (`RelayTarget` veya gelecekteki OAuth
  Platform Hesapları kaydı) **kendi dublaj dili** atanabilmeli.
- **Dublaj kota/kredi tüketmeli** — "bütün herşey kredi mantığında" demek,
  mevcut `UsageTracker`/saat kotası sistemine (STT'nin
  `stt.local.usage_multiplier` deseni gibi) TTS için de gerçek bir kullanım
  ölçümü+düşümü eklenmeli. Şu an `tts.local.usage_multiplier` config'i VAR
  ama HİÇBİR YERDE tüketilmiyor (2.2b'de sadece policy'de expose edildi).

**Bu ikisi birlikte şu anlama geliyor:** 2.2 artık basit bir "pipeline'a
senkron bir synthesize çağrısı ekle" işi değil — gerçek bir şema
değişikliği (VoiceModel+language, RelayTarget+dub_language veya benzeri) +
yeni admin endpoint'leri + kota/kredi entegrasyonu gerektiriyor. **Sonraki
ajan muhtemelen kullanıcıya 1-2 ek soru sormalı** (örn: "RelayTarget'a mı
yoksa yeni bir PlatformDubSettings tablosuna mı dil ekleyeyim?", "kota
TTS için AYRI bir sayaç mı yoksa STT ile aynı 'saat' havuzunu mu paylaşsın,
hangi birimde (karakter/saniye) düşülsün?") — küçük klarifikasyonlarla
devam edilebilir, sıfırdan tasarlamaya gerek yok, ANA KARARLAR YUKARIDA VERİLDİ.

**ÖNERİLEN SIRALAMA (yarım kalan tasarımı tamamlamak için):**
1. `VoiceModel`'e `language: String` ekle (backend/src/voice_models/mod.rs)
   — migration yok (plain JSON store), sadece struct+endpoint güncellemesi.
   `clone_voice` request'ine `language` alanı ekle, frontend
   `voice-clone-studio.tsx`'e dil seçici ekle (muhtemelen
   `translate_languages::DEEPL_TARGET_LANGUAGES` kataloğundan — Faz 2.0'da
   zaten var, aynı listeyi burada da kullan).
2. Admin "bu model bu dil için aktif" ataması — `VoiceModelStore`'a
   `set_active_for_language(user_id, lang, model_id)` benzeri bir metod +
   `POST /admin/voice/models/{id}/assign` + admin UI'da buton.
3. `Gateway::synthesize` çağrısını gerçekten pipeline'a bağla — StreamSession
   çeviri sonrası, EĞER (a) `tts.provider != disabled`, (b) yayıncının
   planı `has_voice_clone`, (c) o hedef dil için aktif/approved bir model
   VARSA, `gateway.synthesize(model.provider_voice_id, translated_text)`
   çağırsın. `tts_source` alanına göre: cloud→ses baytlarını yeni bir
   ServerMessage ile SDK'ya gönder; local→SADECE metni gönder (SDK kendi
   motoruyla sentezleyecek, motor kendisi Faz 3/ayrı iş).
4. Kota/kredi: `UsageTracker`'a TTS için ayrı bir tick mekanizması (veya
   aynı saat havuzuna ekleme) — `stt.local.usage_multiplier` deseniyle
   tutarlı, `tts.local.usage_multiplier` zaten config'te var, sadece
   TÜKETİLMİYOR.
5. 2.3 (İkili Yayın): dublaj dilinin RelayTarget/platform bağlantısı bazında
   seçilmesi — muhtemelen `RelayTarget`'a `dub_language: Option<String>`
   eklemek en basiti (yeni tablo yerine).

Bunların HİÇBİRİ bu oturumda yazılmadı — SIFIRDAN başlanacak, ama tasarım
kararları netleşti, direkt implementasyona geçilebilir.

## Değiştirdiğim Alanlar (bu oturum — dosya/klasör bazında özet)
**Backend (Rust):**
- `backend/src/translate_languages.rs` (yeni) — DeepL 29 dil kataloğu
- `backend/src/stream_settings/mod.rs` (yeni) — hedef dil seçimi store+endpoint
- `backend/src/gateway/tts.rs`, `gateway/elevenlabs.rs` (yeni) — TtsProvider
- `backend/src/voice_models/mod.rs` (yeni) — ses modeli + onay kuyruğu
  **(bkz. yukarıda — language alanı EKSİK, sonraki iş)**
- `backend/src/tts_policy/mod.rs` (yeni) — stt_policy'nin TTS aynası
- `backend/src/sdk_link/mod.rs` — TtsMode/TtsSettingsStore/set_tts_mode/
  report_tts_benchmark eklendi (STT karşılıklarıyla aynı desen)
- `backend/src/gateway/mod.rs` — tts map + clone_voice/synthesize dispatch
- `backend/src/gateway/deepl.rs` — source_lang=auto düzeltmesi
- `backend/src/ws/protocol.rs` — TtsSource enum + StreamStart.tts_source
- `backend/src/pipeline/mod.rs` — StreamSession.tts_source alanı (henüz
  DISPATCH YOK — sadece taşıma, task 2.2'nin işi)
- `backend/src/provider_check/mod.rs` — ElevenLabs xi-api-key auth stili
- `backend/src/config.rs` — tts.*, translate.deepl.enabled_targets seed'leri
- `backend/src/{app,state,main,lib}.rs` — yeni store'ların wiring'i

**SDK (Rust, pure — native bağımlılık yok):**
- `sdk/src/core/gpu_probe.rs` (yeni) — nvidia-smi tabanlı GPU/VRAM probe
- `sdk/src/core/model_fetch.rs` (yeni) — genel model indirme+SHA-256 (TTS
  modeli için de DOĞRUDAN kullanılabilir, değişiklik gerekmez)
- `sdk/src/core/local_tts.rs`, `tts_watchdog.rs` (yeni) — STT çekirdeğinin
  TTS aynası (LocalTtsEngine/MockTtsEngine/run_benchmark, TtsWatchdog)
- `sdk/src/lib.rs` — run_desktop_shell artık `Option<SttControlHandle>`
  alıyor (periyodik STT-control poll loop) — main.rs bilerek `None` geçiyor
  (gerçek whisper.cpp motoru yok)

**Frontend (Next.js):**
- `/dashboard/captions` — target-language-picker.tsx (2.0)
- `/dashboard/sdk` — sdk-pairing-panel.tsx, sdk-stream-key-panel.tsx (2.8)
- `/dashboard/voice` (yeni sayfa) — tts-processing-panel.tsx +
  voice-clone-studio.tsx (2.1/2.2b)
- `/admin/routing` — admin-deepl-languages-panel.tsx eklendi (sayfanın GERİ
  KALANI hâlâ statik mockup — bu Faz 2.7'nin işi)
- `/admin/voice-models` (yeni sayfa) — admin-voice-models-panel.tsx
- `copy-row.tsx` — "Copy" butonu hiçbir şey yapmıyordu, gerçek
  `navigator.clipboard.writeText` eklendi
- `icon.tsx` — Upload ikonu eklendi

## Son Kararlar (bu oturum)
- **Mimari desen: "mirror, generalize etme."** Her STT-analog TTS parçası
  (TtsSettingsStore/SttSettingsStore, tts_policy/stt_policy modülleri vb.)
  PARALEL DUPLICATE olarak yazıldı, ortak soyutlamaya ÇEKİLMEDİ — roadmap
  metninin kendisi "aynı desen" diyor, "aynı kod" demiyor; codebase'in
  genel eğilimi de (StreamSettingsError/SttSettingsError/VoiceModelError
  gibi) küçük tekrarları kabul ediyor.
- **whisper.cpp VE XTTS-v2 native motor bind'i ERTELENDİ** — bu makinede
  `cmake` yok (Arch/CachyOS, kurulu değil). Kullanıcı onayladı: "önce diğer
  parçaları bitir" (whisper.cpp sorusu — önceki oturum). Aynı mantık
  XTTS-v2 için de geçerli, ayrıca sormaya gerek kalmadı.
- **Bu makinede GERÇEK donanım var:** NVIDIA RTX 3070 Laptop GPU (8GB),
  `nvidia-smi` çalışıyor — GPU probe testleri gerçek donanıma karşı koştu.
- **`/dashboard/sdk` "stream key"** = `RelayTarget{platform:"obs",
  rtmp_url:"rtmp://127.0.0.1:19350/live"}` tek sentetik hedef — çoklu
  platform OAuth hesap bağlama (Platform Hesapları) henüz kurulmadığı için
  basitleştirildi. `/api/relay/targets` LİSTE alıyor ama şu an hep 1 eleman.
- **DeepL EN/PT bölgesel kod nüansı (EN-GB/EN-US, PT-PT/PT-BR) BİLEREK
  atlandı** — Faz 1.8'in basit-kod davranışı korundu (`en`/`pt` düz kod),
  gerçek DeepL API'sinde bu ayrım gerekebilir ama canlı key yokken
  doğrulanamadı; `translate_languages.rs`'te yorum olarak not düşüldü.
- **`ws_integration.rs` ve `sdk/tests/cloud_e2e.rs`'teki bazı testler
  "free" plandan "pro"ya çekildi** — 2.0'da eklenen plan-bazlı hedef-dil
  sayısı sınırı (PlanRegistry.max_languages, ÖNCEDEN VARDI ama hiç enforce
  edilmiyordu) bu testlerin 2 hedef dil göndermesini artık haklı olarak
  reddediyordu; testlerin asıl amacı çoklu-dil akışını kanıtlamaktı, plan
  sınırını değil.
- **`run_desktop_shell` (SDK'nın gerçek giriş noktası) HÂLÂ gerçek bir
  `stream_start` kurmuyor** — sadece relay wiring + opsiyonel stt-control
  polling loop var. Bu, gerçek zamanlı ses/altyazı/TTS akışının SDK
  tarafında UÇTAN UCA henüz hiç çalıştırılmadığı anlamına geliyor (sadece
  `sdk/tests/cloud_e2e.rs`'in kendi test harness'i üzerinden). Task 2.2/2.3
  ilerledikçe bu gerçek wiring boşluğu daha da görünür olacak — ayrı,
  büyük bir iş (muhtemelen kendi checkpoint'ini gerektirir).

## Sonraki Adım (TAM OLARAK)
1. **Task 2.2'yi TAMAMLA** — yukarıdaki "ÖNERİLEN SIRALAMA" (5 madde) ile
   başla. Muhtemelen kullanıcıya 1-2 küçük klarifikasyon sorusu daha
   gerekir (kota biriminin ne olacağı, RelayTarget'a mı yeni bir yapıya mı
   dil ekleneceği) — sıfırdan tasarım DEĞİL, küçük detay soruları.
2. Roadmap'te 2.2'nin checkbox'larını işaretle + commit.
3. **Task 2.3 (İkili Yayın)** — 2.2'nin dub_language kararına bağlı,
   muhtemelen aynı oturumda birlikte gider.
4. **[CHECKPOINT] Task 2.2/2.3'ün mux kısmı** — `sdk/src/core/mux.rs` şu an
   SADECE bir enum (OutputPlan, gerçek kod YOK). RTMP relay
   (`sdk/src/core/rtmp/relay.rs`) audio/video tag'lerini OPAK byte-blob
   olarak fan-out ediyor, codec'e dokunmuyor. Klonlanmış sesi GERÇEKTEN
   yayına koymak (OBS'in orijinal ses tag'lerini TTS çıktısıyla
   değiştirmek) yeni bir encoder+senkron stratejisi gerektirir — bu, önceki
   whisper.cpp/cmake sorusu gibi KULLANICIYLA AYRI GÖRÜŞÜLMELİ (AskUserQuestion
   ile: basit AAC pass-through encoder mı, ffmpeg sidecar mı, Faz 3'e mi
   ertelenir).
5. **Task 2.7** (Admin — hibrit kontrol genişletme + canlı maliyet) —
   `/admin/routing` sayfasının STT/TTS/Translate provider toggle'ları hâlâ
   TAMAMEN statik/dekoratif (onClick yok). Bu task o sayfayı gerçek
   `PUT /admin/config`'e bağlamalı. Ayrıca "canlı maliyet göstergesi" —
   `UsageTracker` üzerine config-seeded $/birim tablosu.
6. **Task 2.6** (faster-whisper bulut) — sadece `gateway/whisper_cloud.rs`
   (deepgram.rs deseni, self-hosted REST endpoint) + admin toggle. Hetzner
   GPU sunucu provisioning + TR kalite testi KULLANICI AKSİYONU.
7. **Task 2.5** (Chrome Extension) — `extension/js/shared.js` Twitch-agnostic,
   doğrudan taşınabilir. `bindings::ChannelBindingStore` zaten
   platform-bağımsız tasarlandı ("Chrome Extension aynı depoyu kullanacak"
   yorumu kodda zaten var). Chrome Web Store başvurusu KULLANICI AKSİYONU.
8. **Task 2.4 kalanı** — Twitch console kaydı/Hosted Test/review başvurusu,
   sertifikalar, releases.strovak.com hosting, Hetzner — HİÇBİRİ kodla
   tamamlanamaz, sadece işaretlemek için task listesinde tutuluyor.

## Blokerlar / Dikkat
- **cmake yok** (bu makine) — whisper.cpp VE XTTS-v2 native bind'i
  engelliyor. Kurulum önerilirse kullanıcıya sorulmalı (sistem paketi,
  onay gerekir) — önceki oturumda bu soru soruldu, kullanıcı "önce diğerini
  bitir" dedi, cmake kurulumu hiç yapılmadı.
- **Mux/audio-splice tasarımı ayrı bir kullanıcı görüşmesi gerektirir**
  (yukarıda #4). Bu konuda AŞIRI VARSAYIM yapıp direkt kod yazmaya
  BAŞLAMA — whisper.cpp/cmake sorusuyla AYNI ciddiyette bir mimari çatal.
- **`/admin/routing` sayfası** DeepL kartı hariç TAMAMEN statik mockup —
  eğer bir gelecek task "bu buton çalışmıyor" bulursa, bu BİLİNEN ve zaten
  task 2.7'ye atanmış bir durum, yeni bir bug değil.
- **VoiceModel şeması değişecek** (language alanı ekleniyor) — eğer bu
  arada frontend/backend ayrı ayrı güncellenirse (`clone_voice` request
  şekli, `voice-clone-studio.tsx`'in gönderdiği body) İKİSİNİ BİRLİKTE
  güncellemeyi unutma, aksi halde 422/deserialize hatası.
- **Kredi/kota sistemi henüz TTS'i hiç düşmüyor** — `tts.local.
  usage_multiplier` config'te var ama tüketen kod YOK. STT'nin
  `tick_usage(scale)` deseni (`backend/src/pipeline/mod.rs`) referans al.
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
- Vault local dev kurulumu (`vault server -dev`) hâlâ dökümente değil; JWT_SECRET şimdilik düz env var.
