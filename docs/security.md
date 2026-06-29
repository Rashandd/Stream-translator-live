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
- Faz 1'de kurulur

## GDPR/KVKK
- Hesap silinince ses modeli kalıcı silinir
- Audit log: hangi key ne zaman kullanıldı
