---
name: deploy-staging
description: main'in en güncel halini staging'e (dev.strovak.com / dev.api.strovak.com) deploy et
---

Staging'i main ile güncelle ve deploy et:

1. YENİ bir worktree/branch AÇMA. Tek staging deploy branch/worktree'si vardır:
   branch `development-vps`, worktree `streamdubber-development-full-vps`
   (bkz. `CLAUDE.md` "İstisna — staging deploy"). O worktree'ye geç; yoksa
   `git worktree add ../streamdubber-development-full-vps development-vps` ile aç
   (VPS'te bu dizin düz bir clone'dur, bkz. `infra/vps-from-zero.md`). `git fetch origin`
   sonrası `origin/main`'i `development-vps`'e merge et (`development` entegrasyon
   branch'i origin'de varsa onun üzerinden). Not: main'deki PR #17 staging katmanını
   revert etmişti; o revert `development-vps`'e bir kez `-s ours` ile alındığı için
   normal merge artık `infra/staging-*` dosyalarını silmez — yine de merge sonrası
   `git diff --stat origin/main..HEAD -- infra/` ile katmanın yerinde durduğunu doğrula.
2. Merge sonrası bu branch'in `infra/` altında main'de olmayan commit'i var mı kontrol et: `git log --oneline origin/main..HEAD -- infra/`. Staging'e özel bir infra düzeltmesi (ör. kalıcı bir altyapı kararı) main'e henüz merge edilmemişse onu main'e ayrıca PR'la — bu branch'teki infra farkını main'e kaybettirme. Çakışma çıkarsa DURDUR, kullanıcıya sor.
3. `bash infra/staging-validate.sh` ile ön kontrolü geç (disk, config, caddy syntax).
4. `sudo infra/staging-deploy.sh` ile deploy et. Bu script nginx/eski release'i devralıp Caddy ile cutover yapar; kesinti minimum olmalı.
5. Deploy sonrası doğrula:
   - `infra/staging-status.sh` ve `infra/staging-health.sh`
   - `https://dev.strovak.com` ve `https://dev.api.strovak.com/health` gerçek Let's Encrypt sertifikasıyla dönmeli
   - WSS ve CORS kontrolü
   - Backup dry-run (`infra/staging-backup.sh`), ikinci bir gerçek release varsa rollback dry-run (`infra/staging-rollback.sh`)
6. OAuth/provider secret'ları (Google/Twitch/YouTube/Streamlabs/Deepgram/DeepL) eksikse deploy'u DURDURMA — uygulama ayağa kalksın, eksikleri final raporda "insan işi" olarak listele.
7. Sonucu özetle: hangi main commit'i deploy edildi, bu branch'in önceki deploy'una göre hangi yeni özellikler/commit'ler geldi, health check sonuçları, kalan insan-işi kalemleri.
8. Onay bekleme; yalnız gerçekten insan gerektiren adımda (secret, DNS, hesap yetkisi) dur ve tam olarak ne gerektiğini raporla. `git push` veya main'e merge yalnızca kullanıcı açıkça isterse yapılır.
