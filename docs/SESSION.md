# Aktif Multi-Agent Session Durumu

<!-- Ana koordinasyon dosyasi. Agentlar normalde kendi dosyalarini gunceller:
- Codex: docs/sessions/codex.md
- Claude: docs/sessions/claude.md
-->

## Genel Proje Durumu
Repo blueprint/dokumantasyon asamasinda. Backend/frontend/sdk/infra kod klasorleri henuz olusturulmamis.

## Aktif Agentlar
- Codex: proje analizi ve multi-agent session yapisi.
- Claude: durum bilinmiyor; `docs/sessions/claude.md` guncellenmeli.

## Cakisma Kurali
- Agentlar calisma sonunda sadece kendi session dosyasini gunceller.
- `docs/SESSION.md` yalnizca koordinasyon ozeti degistiginde veya kullanici acikca istediginde guncellenir.
- Ayni anda ayni klasor veya dosya uzerinde calisilmadan once `git status --short` kontrol edilir.

## Neredeyim?
Proje dokümantasyonu analiz edildi. Repo şu an blueprint aşamasında; backend/frontend/sdk/infra kod klasörleri henüz oluşturulmamış. Faz 1 MVP için ilk somut adım backend iskeleti ve WebSocket protokolünün TDD ile kurulması.

## Son Kararlar
- Faz 1 kapsamı: Deepgram + DeepL + kendi SDK gömülü RTMP relay'iyle (+ opsiyonel Streamlabs) ücretsiz altyazı; Restream/Castr desteği kaldırıldı. Ses klonlama, Twitch/Chrome extension ve local AI sonraki fazlarda.
- Transit güvenlikte TLS 1.3 + SDK-backend mTLS kullanılacak; ses akışına ayrıca AES eklenmeyecek. AES-256 sadece at-rest veriler, lokal stream key'ler ve ileride ses modelleri için.
- Konfigürasyonlar DB'deki service_config benzeri katmandan okunacak; provider/fallback/plan limitleri koda gömülmeyecek.
- `docs/websocket-protocol.md` mevcut fakat git'e eklenmemiş; SDK-backend protokolü için referans alınmalı.

## Sonraki Adım
Backend iskeletini oluştur: Rust workspace/backend crate, Axum health endpoint, WebSocket route modülü, audio frame parser için önce testler, sonra implementasyon. Ardından `cargo test` ve `cargo clippy` çalıştır.

## Blokerlar / Dikkat
- Dokümanlarda küçük tutarsızlık var: roadmap ve `docs/commands/test-pipeline.md` bazı yerlerde ses akışı için AES ifadesi kullanıyor; güncel güvenlik kararı TLS 1.3 + mTLS, AES yalnızca at-rest.
- Henüz Cargo/Node/Tauri proje iskeleti yok; komutlar çalıştırılabilir durumda değil.
- Vault, PostgreSQL, Redis ve docker-compose henüz kurulmamış.
