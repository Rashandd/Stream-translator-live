# Bağımsız Üçüncü Taraf Denetimi — StreamDubber

**Tarih:** 2026-07-12 · **Repo durumu:** commit `8917232`, temiz working tree, branch `main`
**Yöntem:** `streamdubber_roadmap_final.md`, `docs/`, `CLAUDE.md`, `AGENTS.md`, `infra/` ve backend/frontend/SDK/stt-server kaynak kodunun salt-okunur incelemesi. Bu rapor dışında hiçbir dosya değiştirilmedi. Satır numaraları yukarıdaki commit'e göredir.

Önem derecesi ölçeği — **kritik:** insanları/agent'ları aktif olarak yanlış veya güvensiz kararlara yönlendiriyor; **majör:** yeniden iş, veri kaybı veya boşa ops eforuna yol açacak somut sapma; **minör:** hijyen/kalıntı.

---

## 1. Roadmap sapması (`streamdubber_roadmap_final.md` vs kod tabanı)

### 1.1 Bitti işaretli ama (tam olarak) implemente edilmemiş maddeler

- **kritik** — `streamdubber_roadmap_final.md:593` — "[x] mTLS SDK ↔ bulut doğrulaması" tikli, ama yalnızca sunucu yarısı var (`backend/src/tls.rs:118-135`); SDK istemci sertifikası sunamıyor (`sdk/src/core/cloud.rs:151-153` açık bir `TODO(mTLS)`), ve aynı roadmap bunu 343 ve 705. satırlarda açık iş olarak izliyor.
  **Düzeltme:** 593. satırın tikini kaldır (veya "yalnızca sunucu tarafı verifier" olarak yeniden yaz) ve mTLS için tek bir checkbox bırak.

- **majör** — `streamdubber_roadmap_final.md:628` — 1.5 "Servis toggle" `/admin/routing`'i gerekçe göstererek tikli, ama o sayfa silindi (commit `00a7c90`; `infra/deploy.md:258-266` sağlayıcı geçişinin artık yalnızca ham config-API/curl olduğunu doğruluyor).
  **Düzeltme:** maddeyi "yalnız config API, panel UI kaldırıldı" olarak yeniden not düş, ya da Production Setup'a minimal bir toggle kartı geri koy.

- **majör** — `streamdubber_roadmap_final.md:872` — 2.0 "[x] Admin panelden hedef dil seçimi… admin-deepl-languages-panel.tsx" — o bileşen `frontend/` içinde hiçbir yerde yok; routing sayfasıyla birlikte gitti. Admin artık `translate.deepl.enabled_targets`'ı yalnızca `PUT /admin/config` ile düzenleyebiliyor.
  **Düzeltme:** tiki kaldır veya yeniden not düş; DeepL hedef dilleri UI'ının artık nerede yaşayacağına karar ver.

- **majör** — `streamdubber_roadmap_final.md:588-592` — 1.2 tikleri kendi notlarına göre bile abartılı: Vault kurulumu [x] "skeleton" (opsiyonel taban store), DB at-rest AES [x] "uygulama ilgili özellik gelince", ses modeli şifreleme [x] "Faz 2".
  **Düzeltme:** bu maddeleri gerçekten shiplenen kapsama göre yeniden yaz (Vault-opsiyonel SecretStore, dosya store'larına uygulanan `crypto::Cipher`) ve kalanı doğrulama listesine taşı.

- **majör** — `streamdubber_roadmap_final.md:605` — "[x] AES-256 şifreli sadece-ses akışı" — başlıktaki özellik bilerek implemente *edilmedi* (transport yalnız TLS 1.3, bkz. `docs/security.md:3-6`); tik, metni güvenlik kararıyla çelişen bir maddeyi "geçirmiş" oluyor.
  **Düzeltme:** başlığı "TLS 1.3 sadece-ses akışı (AES yok — bilinçli karar)" yap.

### 1.2 Roadmap'in kendi içinde çelişmesi (bayat notlar / mükerrer checklistler)

- **majör** — `streamdubber_roadmap_final.md:290-294, 328-333, 343` — giriş bölümündeki "SDK Dağıtım ve Güvenlik" ve "Güvenlik Prensipleri" checklistleri Faz-1 maddelerini kopyalıyor ve onlarla çelişiyor (code signing/otomatik güncelleme 290-291'de boş ama 606-607'de tikli; secret audit-log 332'de boş ama 695. satıra göre implemente).
  **Düzeltme:** faz bölümlerini tek doğruluk kaynağı yap; giriş checklistlerini düz yazıya veya linke çevir.

- **majör** — `streamdubber_roadmap_final.md:668, 671` — 1.11 notları Stripe webhook'larının hâlâ "sadece loglanıyor" ve kullanımın "bellek içi" olduğunu söylüyor, ama ikisi de sonradan düzeltildi (V6 satır 735, V7 satır 737 ve `backend/src/main.rs:358-371`).
  **Düzeltme:** 1.11'deki KALAN notlarını V6/V7'yi gösterecek şekilde güncelle.

- **minör** — `streamdubber_roadmap_final.md:670` — "[x] Starter/Pro/Streamer… fiyatlandırma tablosuyla birebir" artık seed'lerle uyuşmuyor: `backend/src/plans/mod.rs:36-64` 30sa/$9.99, 80sa/$24.99, 200sa/$99.99 shipliyor (2026-07-09 kredi kararı sonrası, satır 364).
  **Düzeltme:** notu güncelle (ve `docs/schema.md` örneklerini, bkz. §2).

### 1.3 Var olan ama roadmap'te (doğru) yansıtılmayan kod

- **minör** — `backend/src/secrets/mod.rs` (secret güncelleme audit izi), `backend/src/legal.rs` + CSP işi (commit `bd173ce`, `d3d7e08`) ve `/admin/legal`, `/admin/multistream` sayfalarının roadmap'te hiç satırı yok; buna karşılık `HETZNER_API_KEY`/`RAILWAY_TOKEN` yalnızca `.env.example`'da yaşamaya devam ediyor.
  **Düzeltme:** bir sonraki roadmap düzenleme geçişine kısa bir "yapıldı ama listede yok" taraması ekle.

### 1.4 Sıralama riskleri

- **majör** — Faz 2 büyük ölçüde inşa edilmiş (2.0–2.6b, 2.8, 2.9 tikli) ama *asıl* MVP kapısı — "Faz 1 — MVP Öncesi Canlı Doğrulama Listesi" (satır 686-737) — hâlâ ~13 açık ops/kullanıcı maddesi barındırıyor (gerçek key'ler, gerçek OBS, gerçek OAuth, gerçek Stripe, code-signing sertifikaları, mTLS connector) ve Founding-5 beta başlangıcına (satır 761) dokunulmamış. Bu arada `docs/backend-hardening-captions-plan.md:3` hardening için "implementasyon başlamadı" diyor.
  **Düzeltme:** doğrulama listesi + hardening P1'i, herhangi bir yeni Faz 2/3 özellik işinden önceki kapı olarak ele al; eksik olan iş operasyonel, kod değil.

---

## 2. Docs vs kod çelişkileri

- **kritik** — mTLS'in dört çelişen hikâyesi var: `docs/security.md:15-17` "Faz 1'de kurulur", `infra/deploy.md:88-90` "Faz 1'de gerekmez", roadmap satır 593 "bitti", ve `sdk/src/core/cloud.rs:151` "TODO". Roadmap+security.md okuyan bir operatör ses kanalını karşılıklı doğrulamanın koruduğuna inanır; korumuyor (koruyan: TLS üstünde cihaz JWT'si).
  **Fix:** tek bir karar kaydet (gerçekçi olanı: "mTLS ertelendi; Faz-1 transport'u TLS 1.3 + cihaz JWT") ve dört yeri de güncelle.

- **majör** — Redis her yerde deklare edilmiş, hiçbir yerde kullanılmıyor: `CLAUDE.md:20` (stack), `infra/deploy.md:111` (zorunlu env), `infra/docker-compose.yml:78-86` + backend'in redis-healthy `depends_on`'u (`:35-36`), `.env.example:33` — ama hiçbir `Cargo.toml`'da redis crate'i, kodda tek bir referans yok. Compose'ta bir redis arızası, ona hiç bağlanmayan bir backend'i bloke ediyor.
  **Düzeltme:** hardening BH-04/BH-06 gerçekten yapılana kadar Redis'i zorunlu env tablosundan ve `depends_on` listesinden çıkar; yalnız hardening planında kalsın.

- **majör** — `docs/websocket-protocol.md` (son gerçek güncelleme 5 Tem) shiplenen üç protokol değişikliğini içermiyor: `stream_start`'taki `tts_source` (`backend/src/ws/protocol.rs:61`), `subtitle`'daki `dubbed_audio` (`protocol.rs:129-133`) ve 4008/4009/4010 hata kodları (`protocol.rs:165-171` — dokümandaki tablo `docs/websocket-protocol.md:225`'te 4007'de bitiyor).
  **Düzeltme:** iki alanı ve üç hata satırını ekle.

- **majör** — `docs/schema.md` yalnızca 0001 migration'ını belgeliyor: `channel_bindings` (0002), `waitlist_signups` (0003), `browser_sessions` (0005), `user_oauth_links` (0006) eksik; `users.password_hash TEXT NOT NULL` diyor (`schema.md:40`) ama 0004 NOT NULL'ı kaldırıp oauth kolonları ekledi; plan örnek fiyat/saatleri (`schema.md:17-21`) `plans/mod.rs:36-64` ile uyuşmuyor; ve `auth_tokens` (`schema.md:188-204`) `backend/src` içinde sıfır referansa sahip (refresh akışı `browser_sessions` kullanıyor).
  **Düzeltme:** dokümanı `backend/migrations/`'tan yeniden üret; `auth_tokens`'ı ölü-veya-gelecek olarak işaretle.

- **majör** — `docs/schema.md` / `CLAUDE.md:168` Postgres'i "tüm tablolar" doğruluk kaynağı gibi sunuyor, ama users/secrets/relay/usage/voice/sessions için asıl kayıt deposu şifreli **dosyalar**; DB opsiyonel best-effort ayna (`backend/src/main.rs:342-371` ve `*_FILE` zinciri).
  **Düzeltme:** schema.md'ye "varlık başına kayıt deposu" tablosu ekle ki operatör doğru şeyi yedeklesin (`/data`, yalnız pg_dump değil).

- **majör** — Kodun desteklediği dört kalıcılık env değişkeni *tüm* infra yüzeylerinde yok (`infra/deploy.md` env tablosu, `infra/docker-compose.yml`, `.env.example`): `STREAM_SETTINGS_FILE` (`main.rs:419-433`), `VOICE_MODELS_FILE` (`main.rs:438-450`), `TTS_SETTINGS_FILE` (`main.rs:470-482`), `VOICE_SAMPLES_FILE` (`main.rs:488-501`). Belgelenen production deploy'unda bu store'lar sessizce bellek içi çalışır — yayıncının hedef dil seçimleri ve ses modeli kayıtları her backend restart'ında yok olur.
  **Düzeltme:** dört satırı deploy.md/compose/.env.example'a ekle (`/data/*.json|.enc`).

- **majör** — `docs/audio-pipeline.md:40-48` streaming sınıfı gecikme vadediyor (Deepgram "<300ms", toplam "<2sn"), ama shiplenen entegrasyon VAD kesiti başına prerecorded REST (`backend/src/gateway/deepgram.rs:1-18`); streaming WS açıkça kapsam dışına alındı (roadmap satır 648) ve her iki plan da dışlıyor. Gerçek gecikme = kesit uzunluğu + HTTP gidiş-dönüşü.
  **Düzeltme:** "mevcut implementasyon: kesit-başına batch" notunu gerçekçi rakamlarla ekle; roadmap 1.9 gecikme maddesini canlı ölçülene kadar boş bırak.

- **minör** — `infra/deploy.md:80-85` Caddy tarifi varsayılanları kullanıyor, bu da TLS 1.2'yi de müzakere eder — oysa `CLAUDE.md:85` ve roadmap satır 587 "TLS 1.3 tüm bağlantılarda" iddia ediyor (yalnız backend TLS'i kendisi sonlandırınca doğru, `backend/src/tls.rs:114-116`).
  **Düzeltme:** Caddyfile parçasına `tls { protocols tls1.3 }` ekle, veya iddiayı "edge'de TLS 1.2+" olarak yumuşat.

- **minör** — `docs/dashboard.md:14` ve `docs/audio-pipeline.md:76` hâlâ panel tabanlı sağlayıcı geçişi vadediyor ("admin panelden anlık"); panel yüzeyi gitti (bkz. §1).
  **Düzeltme:** metni config-API gerçekliğiyle hizala.

- **minör** — `docs/docker-config.md:36-41` fallback zincirlerinde `whisper`, `cartesia`, `xtts`, `hy-mt1` listeliyor — hiçbiri `backend/src/main.rs:259-283`'te kayıtlı değil (yalnız deepgram/assemblyai/selfhosted, deepl/google, elevenlabs); env listesi de çoğu `*_FILE` değişkenini atlıyor.
  **Düzeltme:** "bugün implemente" ile "planlanan sağlayıcı"yı ayır.

- **minör** — `.env.example:159` `RAILWAY_TOKEN` (platform arşivlendi, bkz. `infra/railway.md:1-6`) ve `.env.example:160-164` "kullanılmıyor" diye belgelenmiş `HETZNER_API_KEY`.
  **Düzeltme:** ikisini de sil.

---

## 3. CLAUDE.md + AGENTS.md hijyeni (doğruluk kaynağı değil, denetlenen artefakt olarak)

- **kritik** — `CLAUDE.md:148-155` "Faz 1 Sınırları — Bu Fazda Yok … sormadan implement etme" — ses klonlama, gömülü RTMP relay, faster-whisper ve Twitch/Chrome extension'larını yasaklıyor; hepsi zaten repo'da implemente (`backend/src/voice_models/`, `sdk/src/core/rtmp/`, `stt-server/`, `backend/src/ext_twitch/`, `extension/`) ve kullanıcı onaylı roadmap tikleri var. Dosya kendi içinde de çelişiyor: satır 14 "kendi SDK multistream relay'i"ni Faz-1 hedefinin *içine* koyarken satır 150 Faz 2'ye itiyor. Kurallara uyan bir agent projenin çoktan shiplediği işi reddeder veya yeniden tartışmaya açar.
  **Düzeltme:** bölümü mevcut faz gerçekliğine göre yeniden yaz (ör. "Faz 2 aktif; hâlâ kapsam dışı: Kick OAuth/PKCE, bulut multistream (Mod C), mobil, …").

- **majör** — `CLAUDE.md:46,111-112` (ve `AGENTS.md:111-112`) `sdk/src/audio/coreaudio.rs` ile `pipewire.rs`'i tarif ediyor; iki dosya da yok — macOS/Linux yakalama `sdk/src/audio/cpal_capture.rs` (`docs/audio-pipeline.md:13-24`'te doğru anlatılmış). CLAUDE.md'nin satırları metinsel olarak da bozuk ("(iskelet )"), AGENTS.md'nin "devre dışı — Faz 4"ü ise geçersiz (macOS/Linux indirmeleri açıldı, roadmap satır 966).
  **Düzeltme:** wasapi.rs + cpal_capture.rs gerçekliğiyle değiştir.

- **majör** — `CLAUDE.md:25` "Altyapı: Railway (MVP) → Hetzner GPU" — deploy hedefi 2026-07-05'te Vercel + VPS'e değişti (`infra/deploy.md:1-5`; railway.md ARŞİV işaretli).
  **Düzeltme:** stack satırını güncelle ve `infra/deploy.md`'yi referans ver.

- **majör** — `CLAUDE.md:144` "Session dosyasi kullanilmaz" üç kez yalanlanıyor: `docs/SESSION.md:3` kendini aktif "Ana koordinasyon dosyasi" ilan ediyor (git-tracked, 11 Tem güncel), `docs/sessions/{claude,codex}.md` `.gitignore:75-76` o adları ignore etmeye çalışırken tracked, ve `scripts/session-start.sh`/`session-end.sh` hâlâ eski akışı uyguluyor.
  **Düzeltme:** tek mekanizma seç (fiilî olan SESSION.md), CLAUDE.md/AGENTS.md'de belirt, track/ignore durumunu tutarlı hale getir, script'leri sil veya güncelle.

- **majör** — `CLAUDE.md` ile `AGENTS.md` %95 kopya ama çoktan ayrışıyor (AGENTS.md, `CLAUDE.md:169`'daki hardening-plan referans satırından yoksun; SDK satırları farklı). Elle bakılan iki kural kitabı gelecekteki çelişkiyi garantiler.
  **Düzeltme:** birini kanonik yap (ör. AGENTS.md = "bkz. CLAUDE.md" veya üretilen kopya).

- **minör** — `CLAUDE.md:3,7` — BAŞLARKEN/BİTERKEN başlıkları boş ve yazım hatalı ("calsimaya basliyacakisen"); hiçbir şey talimat etmiyorlar.
  **Düzeltme:** somut regresyon komutlarıyla doldur (`cargo test -p …`, `npm run test`) veya sil.

- **minör** — `CLAUDE.md:35-37` komut listesi: `docker compose up` repo kökünden çalışmaz (compose `infra/docker-compose.yml`'de; doğru çağrı dosyanın 3. satırında), `tauri dev` global CLI varsayıyor (gerçek: `sdk/` içinde `npm run tauri:dev`, bkz. `sdk/package.json`).
  **Düzeltme:** iki komutu da düzelt.

- **minör** — `CLAUDE.md:40-49` klasör haritası `extension/`, `stt-server/`, `scripts/`'i atlıyor; referans tablosu (157-169) ~9 canlı dokümanı atlıyor (`production-operator-guide.md`, `twitch-extension-*.md`, `sdk-distribution.md`, `local-stt-plan.md`, `selfhosted-stt-plan.md`, `frontend-config.md`, `overlay-builder-plan.md`, `frontend-phase2-plan.md`, kökteki `DESIGN.md`).
  **Düzeltme:** girdileri ekle; agent'lara işe başlamadan tabloyu okumaları söyleniyor, boşluklar onları en güncel spec'lerden uzaklaştırıyor.

- **minör** — `CLAUDE.md:103` "STT, çeviri, TTS sağlayıcıları admin panelden toggle edilebilir" — restart'sızlık doğru, "panelden" kısmı `00a7c90` commit'inden beri değil (yalnız config API).
  **Düzeltme:** metni değiştir veya panel kartını geri getir (çapraz-ref §1.1).

---

## 4. Kod sağlığı — mimari riskler ve belgelenmiş kararların ihlalleri

**Doğrulanan sağlıklı taraf (pozitif güvence):** backend ve SDK'da test-dışı `unwrap()` sayısı 0; shiplenen kodda `println!`/`console.log` yok; her iki Dockerfile ve tüm compose servislerinde healthcheck var; backend TLS'i kendisi sonlandırdığında gerçekten yalnız-TLS-1.3 (`tls.rs:116`); secret'lar maskeli okunuyor, çağrı anında çözülüyor, at-rest AES-256-GCM; buluta-yalnız-ses kuralı geçerli — SDK bulut bağlantısı yalnızca binary *ses* frame'i gönderiyor (`sdk/src/core/cloud.rs:200-202`), video sadece lokal RTMP relay'den akıyor; fiyat/planlar `GET /api/plans`'tan render ediliyor, belgelenmiş statik fallback ile (`frontend/src/lib/plans.ts:47-69`); admin legal HTML'i nonce'lu CSP ile sanitize ediliyor.

- **majör** — Tek-instance, restart'a kırılgan çekirdek: overlay hub, aktif stream'ler, pairing kodları ve rate limiter'lar bellek içi; domain verisi dosya-başına store'larda; overlay token her stream'de yeniden üretiliyor. Hepsi kabul edilmiş (`infra/deploy.md:298-309`) ve çare planı mevcut ama "implementasyon başlamadı" (`docs/backend-hardening-captions-plan.md:3`) — Faz-2/3 özellikleri üstüne inmeye devam ederken.
  **Düzeltme:** hardening P1/P2'yi (BH-03/04/06) sonraki özellik fazlarından önce takvime al.

- **majör** — Çekirdek gecikme vaadi vs batch STT: kesit başına prerecorded REST (`backend/src/gateway/deepgram.rs`) artı GPU worker'a periyodik health check yok (`infra/deploy.md:307-309`) — "<2 sn altyazı" ürün vaadi risk altında; roadmap 1.9 gecikme/3-platform testleri (doğru şekilde) hâlâ boş.
  **Düzeltme:** rakamı pazarlamadan önce Founding-5 betasında canlı gecikmeyi ölç; streaming-STT trait'ini planla.

- **minör** — Gerçek OAuth kimlik bilgileri working tree'de düz metin duruyor: repo kökünde `google-client-secret.json` (gerçek web `client_id`+`client_secret`) ve `twitch-keys`. İkisi de gitignore'lu ve hiç commit edilmemiş (doğrulandı), ama bu "Vault veya env var, koda asla gömme" kuralının ruhuyla çelişiyor ve her dizin kopyası/yedeğinde yaşamaya devam ediyor.
  **Düzeltme:** değerleri admin paneline/SECRETS_FILE'a veya repo-dışı bir yola taşı, sonra dosyaları sil.

- **minör** — `backend/src/stt_servers/provision.rs:254` `#[cfg(unix)]` kullanıyor — `sdk/src/audio/` dışında platform-koşullu kod; `CLAUDE.md:93` bunu tek izinli konum olarak ayırıyor (meşru bir SSH-key chmod'u).
  **Düzeltme:** kodu değil kuralı düzelt ("backend'de dosya-izni gibi gerekçeli istisnalar not düşülür").

- **minör** — Commit mesaj kuralı (`CLAUDE.md:82`: feat/fix/chore/test zorunlu) uygulanmıyor: yakın geçmişte `docs:`, `merge:` ve prefix'siz "Update .env.example" (`c6c6f6a`) var.
  **Düzeltme:** izinli prefix listesini genişlet (docs, merge) veya hook ile zorla — şu an kural kâğıt üstünde.

- **minör** — Kalıcılık env ile opsiyonel: herhangi bir `*_FILE` boşken backend yalnızca uyarıyor (`main.rs:139, 352, 367…`) — dev için güvenli, ama tek satırı atlayan bir production operatörü restart'ta hesapları/kullanımı/limitleri sessizce kaybeder.
  **Düzeltme:** kritik store'lar bellek-tabanlıyken `deploy.profile=production` fail-fast'i (veya kırmızı bir Production-Setup kartı) ekle.

---

## İlk 5 öncelik

1. **CLAUDE.md/AGENTS.md'yi gerçekliğe göre yeniden yaz** — faz sınırları (§3 kritik), stack/Railway, SDK dosya haritası, session-dosyası kuralı, komutlar — ve iki dosyayı tek kanonik kural kitabında birleştir. Gelecekteki her agent oturumu önce bu hataları devralıyor. -> ikiside ayni olsun sesion olayina gerek yok ve worktreeler halinde calissinlar
2. **mTLS hikâyesini bir kez karara bağla** (ertele-veya-yap) ve roadmap:593, `docs/security.md`, `infra/deploy.md` ve `cloud.rs` TODO'sunu hizala — bir güvenlik kontrolü şu an iki yerde iddia edilip iki yerde reddediliyor.
3. **Eksik dört `*_FILE` env değişkenini ekle** (`STREAM_SETTINGS_FILE`, `VOICE_MODELS_FILE`, `VOICE_SAMPLES_FILE`, `TTS_SETTINGS_FILE`) — deploy.md/compose/.env.example'a; belgelenen production deploy'u restart'ta yayıncı dil ayarlarını ve ses modeli kayıtlarını sessizce kaybediyor.
4. **Tek seferlik roadmap defter düzeltmesi:** mükerrer giriş checklistlerini kaldır, yanlış tikleri düzelt (mTLS 593, routing/DeepL UI 628/872, AES/Vault 588-592/605), bayat KALAN notlarını tazele (668/671) — sonra "Canlı Doğrulama Listesi"ni tek MVP kapısı olarak tut ve daha fazla Faz-2/3 kodu öncesi eriterek bitir.
5. **Redis kararını şimdi ver:** ya hardening BH-04/06 gerçekten planlanana kadar stack/compose/deploy-env'den çıkar, ya da o işi başlat — bugün zorunlu-ama-kullanılmayan, backend açılışını bloke edebilen bir altyapı; aynı karar §4'teki tek-instance risklerini de kapılıyor. -> O işi Başlat Redis hazir ve prod-ready documentli olsun
