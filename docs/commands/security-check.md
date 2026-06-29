---
name: security-check
description: Deploy öncesi güvenlik kontrol listesi
disable-model-invocation: true
---

Deploy öncesi güvenlik kontrolü:

1. Kodda hardcoded API key / secret ara: `grep -r "sk-\|Bearer \|password" --include="*.rs" --include="*.ts" src/`
2. AES-256 şifreli olmayan ses akışı var mı kontrol et
3. Vault'tan çekilmeyen key var mı: `.env` dosyasına bak
4. `cargo audit` çalıştır (Rust bağımlılık açıkları)
5. `npm audit` çalıştır (JS bağımlılık açıkları)
6. mTLS sertifikası süresi: `openssl x509 -in cert.pem -noout -dates`
7. Raporu yaz: güvenli mi, risk var mı
