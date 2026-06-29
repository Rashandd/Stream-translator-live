---
name: security-check
description: Deploy öncesi güvenlik kontrol listesi
---

Deploy öncesi güvenlik kontrolü:

1. Kodda hardcoded API key / secret ara: `grep -r "sk-\|Bearer \|password" --include="*.rs" --include="*.ts" src/`
2. Ses akışı TLS 1.3 üzerinden mi gidiyor — çift şifreleme (AES üstüne TLS) var mı kontrol et, varsa kaldır
3. AES-256 at-rest şifreleme: DB, ses modelleri, lokal stream key'ler şifreli mi
4. Vault'tan çekilmeyen key var mı: `.env` dosyasına bak
5. `cargo audit` çalıştır (Rust bağımlılık açıkları)
6. `npm audit` çalıştır (JS bağımlılık açıkları)
7. mTLS sertifikası süresi: `openssl x509 -in cert.pem -noout -dates`
8. Raporu yaz: güvenli mi, risk var mı
