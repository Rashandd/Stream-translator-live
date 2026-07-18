---
name: deploy-staging
description: main'in en güncel halini staging'e (dev.strovak.com / dev.api.strovak.com) deploy et
---

Staging'i main ile güncelle ve deploy et:

1. Repo kurallarına göre worktree/branch aç, taban olarak güncel `origin/main`'i kullan (`git fetch origin` sonrası `origin/main`).
2. Az önceki staging deploy branch'i ile şu anki `origin/main` arasında infra'ya özel fark var mı kontrol et: `git log --oneline <önceki-staging-branch>..HEAD -- infra/`. Staging'e özel bir infra düzeltmesi (ör. Rashand kaldırma gibi kalıcı bir altyapı kararı) main'e henüz merge edilmemişse, sadece o infra dosyalarını cherry-pick/merge et — kod tarafına (backend/frontend/sdk) dokunma. Çakışma çıkarsa DURDUR, kullanıcıya sor.
3. `bash infra/staging-validate.sh` ile ön kontrolü geç (disk, config, caddy syntax).
4. `sudo infra/staging-deploy.sh` ile deploy et. Bu script nginx/eski release'i devralıp Caddy ile cutover yapar; kesinti minimum olmalı.
5. Deploy sonrası doğrula:
   - `infra/staging-status.sh` ve `infra/staging-health.sh`
   - `https://dev.strovak.com` ve `https://dev.api.strovak.com/health` gerçek Let's Encrypt sertifikasıyla dönmeli
   - WSS ve CORS kontrolü
   - Backup dry-run (`infra/staging-backup.sh`), ikinci bir gerçek release varsa rollback dry-run (`infra/staging-rollback.sh`)
6. OAuth/provider secret'ları (Google/Twitch/YouTube/Streamlabs/Deepgram/DeepL) eksikse deploy'u DURDURMA — uygulama ayağa kalksın, eksikleri final raporda "insan işi" olarak listele.
7. Sonucu özetle: hangi main commit'i deploy edildi, önceki staging deploy'a göre hangi yeni özellikler/commit'ler geldi, health check sonuçları, kalan insan-işi kalemleri.
8. Onay bekleme; yalnız gerçekten insan gerektiren adımda (secret, DNS, hesap yetkisi) dur ve tam olarak ne gerektiğini raporla. `git push` veya main'e merge yalnızca kullanıcı açıkça isterse yapılır.
