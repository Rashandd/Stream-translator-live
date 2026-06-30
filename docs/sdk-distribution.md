# SDK Dağıtım — Code Signing & Otomatik Güncelleme

Faz 1.3 kapsamı. SDK Tauri 2 ile paketlenir. Bu doküman code signing ve
otomatik güncelleme kurulumunu açıklar. Konfigürasyon `sdk/src-tauri/tauri.conf.json`.

> Not: İmzalama sertifikaları ve özel anahtarlar **koda/commit'e girmez** — CI
> secret'ı veya Vault'tan gelir (bkz. `docs/security.md`).

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
- `tauri-plugin-updater` bağımlılığı + `run_desktop_shell()` içinde updater init (SDK runtime tasksıyla).
- Release CI pipeline: build → sign → `latest.json` yayınla.
- `releases.strovak.com` statik host (S3/Cloudflare) kurulumu (infra).

## Durum (Faz 1.3)
- ✅ `tauri.conf.json` updater + bundle signing iskelet konfigürasyonu (placeholder anahtarlarla)
- ⏳ Gerçek sertifikalar, anahtar üretimi, release CI pipeline ve updater runtime init — SDK runtime/CI tasklarında
