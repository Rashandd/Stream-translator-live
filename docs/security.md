# Güvenlik Kararları

## Ses Akışı Şifreleme
TLS 1.3 yeterli — ses akışı zaten şifreli taşınıyor.
Üstüne AES-256 ekleme: double encryption = gereksiz CPU + latency.
MVP'de ve muhtemelen sonrasında da gerek yok.

## Key Yönetimi
- HashiCorp Vault self-hosted (bağımsızlık için)
- Deepgram / DeepL / ElevenLabs key'leri Vault'tan runtime'da çekilir
- JWT secret, stream key'ler env var veya Vault — koda gömülmez
- `.env` → .gitignore, `.env.example` → commit
- Key rotasyonu periyodik otomatik

## mTLS
- SDK ↔ bulut karşılıklı sertifika doğrulaması
- **Durum (2026-07-12): ERTELENDİ.** Sunucu tarafı hazır (`backend/src/tls.rs`,
  `TLS_CLIENT_CA_PATH` verilirse istemci sertifikası zorunlu kılar), ama SDK
  istemci sertifikası sunmuyor (`sdk/src/core/cloud.rs` — connector TODO).
  Bugünkü SDK ↔ bulut karşılıklı doğrulaması **TLS 1.3 + cihaz JWT'sidir**
  (pairing/claim akışı, 30 gün geçerli token) — gömülü sertifika değil.
  mTLS connector bağlanınca bu not güncellenir; bkz. roadmap "Faz 1 — MVP
  Öncesi Canlı Doğrulama Listesi" (SDK bölümü).

## GDPR/KVKK
- Hesap silinince ses modeli kalıcı silinir
- Audit log: hangi key ne zaman kullanıldı
