# SDK Dağıtım — Code Signing & Otomatik Güncelleme

Faz 1.3 kapsamı. SDK Tauri 2 ile paketlenir. Bu doküman code signing ve
otomatik güncelleme kurulumunu açıklar. Konfigürasyon `sdk/src-tauri/tauri.conf.json`.

> Not: İmzalama sertifikaları ve özel anahtarlar **koda/commit'e girmez** — CI
> secret'ı veya Vault'tan gelir (bkz. `docs/security.md`).

## Referans proje

[meetily](https://github.com/Zackriya-Solutions/meetily) — Tauri + Rust ile
yazılmış, cross-platform mikrofon+sistem sesi yakalayan açık kaynak bir masaüstü
uygulaması (whisper.cpp ile yerel STT). Tauri masaüstü kabuğu + Rust çekirdek
ayrımı ve çoklu-platform ses yakalama mimarisi bu SDK'nınkiyle aynı sınıf —
ileride whisper.cpp bağlanacağı zaman (roadmap 2.6b, local-STT) da referans
olabilir. Bkz. `docs/audio-pipeline.md` "Yerel Ses Yakalama" bölümü.

## Code Signing

### Windows (SmartScreen)
- Authenticode sertifikası gerekir (OV/EV). EV, SmartScreen uyarısını anında kaldırır.
- `tauri.conf.json` → `bundle.windows`:
  - `certificateThumbprint`: yerel sertifika store'daki parmak izi, **veya**
  - `signCommand`: harici imzalayıcı (örn. `azuresigntool` ile bulut HSM).
- CI: sertifika/PIN secret olarak verilir; timestamp URL ayarlı (offline doğrulama için).

### macOS (Notarization)
- Apple Developer "Developer ID Application" sertifikası.
- `bundle.macOS.signingIdentity` ayarlanır.
- Notarization: CI'da `APPLE_ID`, `APPLE_PASSWORD` (app-specific), `APPLE_TEAM_ID`.
- `tauri build` imzalar; ardından `xcrun notarytool submit` + `stapler staple`.

### Linux
- AppImage/deb imzalama opsiyonel; Faz 4'te aktive edilir (CLAUDE.md: Linux adaptörü sonra).

## Otomatik Güncelleme (tauri-plugin-updater)

1. İmza anahtar çifti üret: `tauri signer generate -w ~/.tauri/streamdubber.key`
   - **Public key** → `tauri.conf.json` → `plugins.updater.pubkey`
   - **Private key** → CI secret `TAURI_SIGNING_PRIVATE_KEY` (asla commit edilmez)
2. `plugins.updater.endpoints`: sürüm sunucusu `latest.json` döndürür
   (`{{target}}/{{arch}}/{{current_version}}` şablonu).
3. Her release'te build artefaktları imzalanır; `latest.json` yeni sürümü işaret eder.
4. SDK açılışta endpoint'i sorar, imzayı public key ile doğrular, indirir, uygular.

### Yapılacaklar (runtime wiring)
- `tauri-plugin-updater` bağımlılığı + `run_desktop_shell()` içinde updater init (SDK runtime tasksıyla) — CI pipeline'ı imzalıyor ama SDK henüz kendi güncelleme kontrolünü yapmıyor.
- `releases.strovak.com` statik host (S3/Cloudflare) kurulumu (infra) — şimdilik indirme linkleri GitHub Releases'e gidiyor (`NEXT_PUBLIC_SDK_RELEASES_BASE`, bkz. `frontend/src/lib/sdk-releases.ts`); özel domain ops task'ı.

## Durum (Faz 1.3)
- ✅ `sdk/ui` — gerçek Tauri penceresi (pairing formu + durum ekranı), `tauri build` artık gerçekten çalışıyor (önceden `frontendDist` boştu, hiç derlenmemişti).
- ✅ `.github/workflows/sdk-release.yml` — Windows/macOS/Linux matrix, `tauri-action` ile build→sign→GitHub Release. Updater imzalama (`TAURI_SIGNING_PRIVATE_KEY`) ve macOS notarization (`APPLE_*`) secret'ları boşken sessizce imzasız derler.
- ⚠️ Windows Authenticode: workflow PFX'i runner'a import edip thumbprint'i `tauri build --config` ile geçiriyor — gerçek bir sertifikayla hiç denenmedi (best-effort, `WINDOWS_CERTIFICATE` secret'ı yok). Ops bir sertifika sağladığında ilk gerçek release'te doğrulanmalı.
- ⏳ Gerçek sertifikalar (Windows/macOS), anahtar üretimi, `releases.strovak.com` custom domain — deploy zamanı ops işi.
