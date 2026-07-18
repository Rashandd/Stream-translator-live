# Güvenlik Kararları

## Ses Akışı Şifreleme
TLS 1.3 yeterli — ses akışı zaten şifreli taşınıyor.
Üstüne AES-256 ekleme: double encryption = gereksiz CPU + latency.
MVP'de ve muhtemelen sonrasında da gerek yok.

## Key Yönetimi
- HashiCorp Vault self-hosted (bağımsızlık için)
- DeepL key'i Vault'tan runtime'da çekilir
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

## Browser Kimlik Doğrulama

- Password, signup, Google ve Twitch login aynı `HttpOnly` access/refresh cookie
  session'ını kullanır; browser JWT'si URL veya Web Storage'a yazılmaz.
- Refresh token yalnız SHA-256 hash'iyle `browser_sessions` tablosunda tutulur,
  her yenilemede döner; eski token replay edilirse session family iptal edilir.
- State değiştiren cookie isteklerinde `sd_csrf` double-submit kontrolü vardır.
- E-posta 2FA kimliği frontend'e verilmez. Kısa ömürlü opaque
  `sd_auth_challenge` cookie'si server-side subject/e-posta/role kaydına bağlanır;
  admin ve yayıncı aynı doğrulama endpoint'ini kullanır.
- Logout server-side revoke tamamlanmadan yönlendirme yapmaz ve diğer sekmelere
  `BroadcastChannel`/storage olayıyla bildirilir.
- Auth hata nedeni query string'e yazılmaz; tek kullanımlık, sabit frontend
  notice koduyla gösterilir.

## Consent Kaydı

Signup sırasında zorunlu legal belge sürümleri ve ayrı marketing tercihi server
timestamp'iyle append-only olarak kaydedilir. Şifreli `USERS_FILE` doğruluk
kaynağıdır; `user_consents` PostgreSQL aynasıdır. OAuth authorize anındaki sürüm
snapshot'ı callback'e kadar server-side state'te taşınır; config değişikliği
kullanıcının gördüğü sürümü değiştirmez.
