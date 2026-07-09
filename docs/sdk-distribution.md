# SDK Dağıtım — Code Signing & Otomatik Güncelleme

Faz 1.3 kapsamı. SDK Tauri 2 ile paketlenir. Bu doküman code signing ve
otomatik güncelleme kurulumunu açıklar. Konfigürasyon `sdk/src-tauri/tauri.conf.json`.

> Not: İmzalama sertifikaları ve özel anahtarlar **koda/commit'e girmez** — CI
> secret'ı veya Vault'tan gelir (bkz. `docs/security.md`).

## Canlıya çıkmadan önce — bloker listesi

Kod tarafı tamam; aşağıdakilerin hepsi **ops/deploy zamanı işi**, koda ihtiyaç
duymuyor. Bir yayıncıya gerçek dağıtım yapılmadan önce bu liste sıfırlanmalı:

| # | Bloker | Ne gerekiyor | Etkisi çözülmeden |
|---|---|---|---|
| 1 | Updater imza anahtar çifti | `tauri signer generate` → public key `tauri.conf.json`, private key CI secret `TAURI_SIGNING_PRIVATE_KEY` | Otomatik güncelleme çalışmaz (`app.updater()` hata verir, kod bunu loglar ve sessizce devam eder — uygulama çökmez, sadece güncellenmez) |
| 2 | `releases.strovak.com` statik host | S3/Cloudflare kurulumu (infra) | İndirme linkleri şimdilik GitHub Releases'e gider (`NEXT_PUBLIC_SDK_RELEASES_BASE`, bkz. `frontend/src/lib/sdk-releases.ts`) — işlevsel ama marka alan adı yok |
| 3 | Windows Authenticode sertifikası | OV/EV sertifika + CI secret `WINDOWS_CERTIFICATE` | İmzasız `.exe` — Windows SmartScreen uyarısı gösterir, kurulum sürtünmesi yaratır |
| 4 | macOS Developer ID + notarization | Apple Developer hesabı + `APPLE_ID`/`APPLE_PASSWORD`/`APPLE_TEAM_ID` CI secret'ları | İmzasız `.dmg` — Gatekeeper engeller, kullanıcı manuel "yine de aç" yapmalı |
| 5 | Gerçek cihazda uçtan uca doğrulama | Windows'ta gerçek mikrofon + gerçek OBS ile smoke test; macOS'ta derleme + smoke test (repo'da darwin toolchain yok, hiç denenmedi) | WASAPI/OBS entegrasyonu kod olarak tamam ama gerçek donanımda hiç çalıştırılmadı — Linux mikrofon yakalaması gerçek cihazla doğrulandı, bu ikisi doğrulanmadı |

Bunların hiçbiri kod değişikliği gerektirmiyor — hepsi hesap açma, sertifika
satın alma, DNS/host kurulumu veya elle test. Detaylar aşağıdaki bölümlerde.

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

### Runtime wiring — TAMAMLANDI
- `tauri-plugin-updater` bağımlılığı `sdk/src-tauri/Cargo.toml`'a eklendi, `main.rs`'te `.plugin(tauri_plugin_updater::Builder::new().build())` ile kaydedildi.
- SDK açılışta (`setup()` içinde, arka planda) `app.updater()?.check()` çağırır; yeni imzalı sürüm varsa `download_and_install` ile indirip kurar. **Bilinçli olarak `app.restart()` çağırmaz** — yayıncı yayın ortasındayken zorla yeniden başlatma riskini almaz; güncelleme bir sonraki manuel açılışta devreye girer.
- Not: `run_desktop_shell()` (platform-bağımsız çekirdek, `sdk/src/lib.rs`) kasıtlı olarak `AppHandle` almaz — updater `main.rs`'teki Tauri `Builder::setup()` katmanında yaşar, çekirdeğin Tauri'den habersiz kalması kuralına uygun.
- Kalan tek eksik: `plugins.updater.pubkey` boş (aşağıdaki "Yapılacaklar — ops" listesine bakın) — anahtar üretilip doldurulana kadar `app.updater()` runtime'da hata döner, kod bunu `tracing::warn!` ile loglar ve sessizce devam eder (uygulamayı bloklamaz).

### Yapılacaklar (ops — kod tarafı bitti)
- İmza anahtar çifti üret (`tauri signer generate`) → public key `tauri.conf.json`'a, private key `TAURI_SIGNING_PRIVATE_KEY` CI secret'ına.
- `releases.strovak.com` statik host (S3/Cloudflare) kurulumu (infra) — şimdilik indirme linkleri GitHub Releases'e gidiyor (`NEXT_PUBLIC_SDK_RELEASES_BASE`, bkz. `frontend/src/lib/sdk-releases.ts`); özel domain ops task'ı.

## Durum (Faz 1.3)
- ✅ `sdk/ui` — gerçek Tauri penceresi (pairing formu + durum ekranı), `tauri build` artık gerçekten çalışıyor (önceden `frontendDist` boştu, hiç derlenmemişti).
- ✅ `.github/workflows/sdk-release.yml` — Windows/macOS/Linux matrix, `tauri-action` ile build→sign→GitHub Release. Updater imzalama (`TAURI_SIGNING_PRIVATE_KEY`) ve macOS notarization (`APPLE_*`) secret'ları boşken sessizce imzasız derler.
- ✅ Updater runtime wiring — SDK artık açılışta kendi güncelleme kontrolünü yapıyor (yukarıya bakın).
- ⚠️ Windows Authenticode: workflow PFX'i runner'a import edip thumbprint'i `tauri build --config` ile geçiriyor — gerçek bir sertifikayla hiç denenmedi (best-effort, `WINDOWS_CERTIFICATE` secret'ı yok). Ops bir sertifika sağladığında ilk gerçek release'te doğrulanmalı.
- ⏳ Gerçek sertifikalar (Windows/macOS), updater imza anahtar çifti, `releases.strovak.com` custom domain — deploy zamanı ops işi (bkz. yukarıdaki "Yapılacaklar — ops").
- ⏳ Gerçek OBS + gerçek mikrofon (Windows/macOS) ile uçtan uca canlı doğrulama hiç yapılmadı — Linux mikrofon yakalaması gerçek cihazla doğrulandı, Windows/macOS kod tamam ama denenmedi.
