---
name: fix-issue
description: GitHub issue'yu analiz et ve düzelt
---

Şu issue'yu düzelt: $ARGUMENTS

1. `gh issue view $ARGUMENTS` ile detayları al
2. İlgili dosyaları ve kodu anla
3. Rust ise: `cargo clippy` geç, test yaz önce
4. Next.js ise: `npm run lint` geç, component test yaz
5. Fix'i implement et
6. `cargo test` veya `npm run test` çalıştır — hepsi geçmeli
7. Conventional commit: `fix: <kısa açıklama>` (#issue-no)
8. PR aç
