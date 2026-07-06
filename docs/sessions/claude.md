# Claude Session

## Agent
Claude (Sonnet 5, claude-sonnet-5)

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

### Sonraki Adım (TAM OLARAK)
1. **Roadmap'te 2.1/2.2'nin yeni checkbox'larını + trim edilen "Öz Kaynaklı..." bölümünü commit'le**
   (bu oturumun son işi — commit henüz atılmadı, bu notla birlikte atılacak).
2. **[CHECKPOINT] Task 2.2'nin TEK kalan maddesi: SDK'da video + klonlanmış ses birleştirme (mux)**
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
