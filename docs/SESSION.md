# Aktif Multi-Agent Session Durumu

<!-- Ana koordinasyon dosyasi. Agent bazli session dosyasi kullanilmaz. -->

## Genel Proje Durumu
Backend (Rust/Axum) + Frontend (Next.js) + SDK (Tauri/Rust) kod klasorleri mevcut ve calisir durumda. Faz 1 kapsaminin buyuk bolumu tamamlandi (bkz. `streamdubber_roadmap_final.md`).

## Neredeyim?
2026-07-09 session: STT/TTS/pricing stratejisi netleşti — Deepgram/AssemblyAI/
ElevenLabs/Cartesia/PlayHT gibi 3. parti API'ler STT ve TTS/ses klonlama için
tamamen devre dışı; onların yerine (a) yayıncının kendi PC'si (SDK local,
zaten `local_stt.rs`/`local_tts.rs` ile var) ve (b) StreamDubber'ın kendi
kiraladığı Vast.ai/Hetzner GPU'sunda self-hosted açık kaynak modeller
(STT: faster-whisper/Voxtral; TTS/klonlama: Chatterbox, VibeVoice-Realtime,
Qwen3-TTS-CustomVoice, VoxCPM2 — hepsi MIT/Apache-2.0, lisansları tek tek
HuggingFace'ten doğrulandı; XTTS-v2/F5-TTS/OmniVoice/fish-speech/higgs-tts
ticari kullanım için kapalı lisanslı, KULLANILMAYACAK). Çeviri (DeepL) bu
kuralın dışında, API olarak kalıyor.

Önceki session'da (bu daldan önce, ayrı bir worktree'de) SDK ↔ hesap pairing
workflow'unun güvenlik sertleştirmesi tamamlanmıştı: `POST /api/pairing/claim`
auth gerektirmiyordu ve 6 haneli kod rate-limit'siz brute-force'a açıktı;
`pairing::ClaimLimiter` (waitlist limiter ile aynı desen, dakikada 10 deneme)
ile kapatıldı, testler+clippy temiz, zaten `main`'de.

Bu session'da yapılanlar:
1. `streamdubber_roadmap_final.md`: STT tablosundan Deepgram/AssemblyAI/
   whisper.cpp silindi; TTS tablosuna Pocket-TTS eklendi; pricing tablosu
   yeni rakamlarla güncellendi; yeni "Fiyatlandırma — Kredi Sistemi" bölümü
   eklendi (bileşen×konum kredi ağırlık tablosu + hangisi kodlandı/hangisi
   bekliyor).
2. `backend/src/voice_models/mod.rs`: `VoiceConsent` eklendi
   (`consent_text_version`, `is_own_voice`, sunucu-taraflı `accepted_at`) —
   `clone_voice`'un zorunlu ön koşulu. Testler: `backend/tests/
   voice_models_api.rs` + birim testler (fail-closed legacy deserialize dahil).
3. Kredi sisteminin ilk kod adımı: `stt_policy::cloud_credit_weight` /
   `tts_policy::cloud_credit_weight` (config: `stt.cloud.credit_weight`,
   `tts.cloud.credit_weight`, varsayılan 1.0) + `pipeline::StreamSession::
   with_credit_weights` + `ws::handle_control`'da wiring. `cargo test` (tüm
   workspace, backend+sdk) ve `cargo clippy --all-targets -- -D warnings` temiz.

## Son Kararlar
- 2026-07-09: STT+TTS için sıfır 3.parti API — sadece broadcaster-PC (SDK)
  veya StreamDubber'ın kendi GPU'su. Çeviri DeepL'de kalıyor (ucuz, API).
- 2026-07-09: Fiyatlandırma "saat" yerine "kredi" birimine geçiyor; kredi
  ağırlığı = bileşen (STT/TTS) × konum (broadcaster-PC/StreamDubber-GPU) ×
  o konumdaki modelin gerçek $/saat maliyeti. Tam `monthly_hours→
  monthly_credits` şema geçişi henüz yapılmadı (bkz. Sonraki Adım).
- 2026-07-09: Ses klonlama rızası artık ayrı, zorunlu bir kayıt
  (`VoiceConsent`) — admin onay kapısı (kötüye kullanım denetimi) ile karışık
  değil, hukuki rızanın kendisi.
- Commit akışı: Testler ve lint temizse anlamlı mesajla commit oluşturulur;
  `git push` kullanıcı açıkça istemedikçe yapılmaz.
- Faz 1 kapsamı: kendi SDK gömülü RTMP relay'iyle (+ opsiyonel Streamlabs)
  ücretsiz altyazı; Restream/Castr desteği kaldırıldı.
- Transit güvenlikte TLS 1.3 + SDK-backend mTLS kullanılıyor (backend/src/
  tls.rs — WebPkiClientVerifier, `TLS_CLIENT_CA_PATH` verilirse zorunlu);
  ses akışına ayrıca AES eklenmez. AES-256 sadece at-rest veriler (pairing
  token, stream key, ses örnekleri) için.
- Konfigürasyonlar DB'deki service_config benzeri katmandan okunur;
  provider/fallback/plan limitleri koda gömülmez.

## Sonraki Adım
Kredi sisteminin geri kalanı (roadmap'teki "Henüz yapılmayan" listesiyle
birebir):
1. `PlanDefinition.monthly_hours` → `monthly_credits` (plans/mod.rs +
   docs/schema.md + frontend pricing sayfası) — ayrı bir PR (tek sorumluluk).
2. `usage::UsageTracker`'ı saniye yerine kredi biriktirecek şekilde genişlet.
3. Çoklu self-hosted TTS modeli için `tts_servers` modülü (stt_servers'ın
   kopyası) + model başına `credit_weight`/`min_plan` — bugün gerçek model
   entegrasyonları (Chatterbox/VibeVoice/Qwen3-TTS/VoxCPM2) `gateway::`
   altında YOK, önce en az biri (öneri: Chatterbox + VibeVoice-Realtime,
   ikisi de düşük VRAM) gerçek bir Vast.ai kutusunda kurulup ölçülmeli.
4. Ağırlık tablosundaki tahmini $/saat rakamları gerçek ölçümle kalibre et.
5. Ayrıca: ses klonlama rızası geri çekme akışı (UI) + AB AI Act şeffaflık
   etiketi (overlay'de "AI ile dublajlanmış ses" bildirimi, yürürlük 2026
   ortası yaklaşıyor).

## Blokerlar / Dikkat
- Bu iş bir worktree branch'inde (`worktree-witty-forging-snail`) — bu
  ortamda GitHub kimlik bilgisi olmadığı için `git push` başarısız oluyor,
  elle push/PR gerekiyor.
- `tts_servers` inşa edilmeden credit_weight tablosu sadece TEK aktif
  `tts.provider` için anlamlı (config: `tts.cloud.credit_weight`) — çoklu
  model/plan gating'i henüz yok.
- `pairing::ClaimLimiter` per-IP bellek içi — çoklu backend instance
  (horizontal scale) senaryosunda paylaşılmıyor; MVP tek-instance için
  yeterli (waitlist/ext_twitch limiter'ları için de aynı durum).
