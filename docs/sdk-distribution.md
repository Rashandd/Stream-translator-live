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
| 1 | Updater imza anahtar çifti | `tauri signer generate` → public key `tauri.conf.json`, private key CI secret `TAURI_SIGNING_PRIVATE_KEY` | Otomatik güncelleme çalışmaz (`app.updater()` hata verir, kod bunu loglar ve sessizce devam eder — uygulama çökmez, sadece güncellenmez, UI'da hiçbir banner görünmez) |
| 2 | `releases.strovak.com` statik host | S3/Cloudflare kurulumu (infra) | `plugins.updater.endpoints` şimdilik GitHub Releases'in `latest.json` asset'ine işaret ediyor (`tauri-action` her release'te bunu otomatik üretip yüklüyor) — işlevsel, ama marka alan adı geldiğinde `tauri.conf.json`'da tek satır değişir |
| 3 | Windows Authenticode sertifikası | OV/EV sertifika + CI secret `WINDOWS_CERTIFICATE`/`WINDOWS_CERTIFICATE_PASSWORD` | İmzasız `.exe` — Windows SmartScreen uyarısı gösterir, kurulum sürtünmesi yaratır |
| 4 | macOS Developer ID + notarization | Apple Developer hesabı + `APPLE_CERTIFICATE`/`APPLE_ID`/`APPLE_PASSWORD`/`APPLE_TEAM_ID` CI secret'ları | İmzasız `.dmg` — Gatekeeper engeller, kullanıcı manuel "yine de aç" yapmalı |
| 5 | Gerçek cihazda uçtan uca doğrulama | Windows'ta gerçek mikrofon + gerçek OBS ile smoke test; macOS'ta gerçek cihaz smoke test (CI'da artık bir `sdk-macos-build-check` job'ı var — sadece derleme, gerçek donanım testi değil) | WASAPI/OBS entegrasyonu kod olarak tamam ama gerçek donanımda hiç çalıştırılmadı — Linux mikrofon yakalaması gerçek cihazla doğrulandı, bu ikisi doğrulanmadı |

Bunların hiçbiri kod değişikliği gerektirmiyor — hepsi hesap açma, sertifika
satın alma, DNS/host kurulumu veya elle test. Detaylar aşağıdaki bölümlerde.

### 1 — Updater imza anahtar çifti (ops, tek seferlik)

```bash
npx @tauri-apps/cli signer generate -w ~/.tauri/streamdubber.key
```

- Komut `~/.tauri/streamdubber.key` (private) + `~/.tauri/streamdubber.key.pub`
  (public) üretir ve public key'i stdout'a da basar.
- **Public key** → `sdk/src-tauri/tauri.conf.json` → `plugins.updater.pubkey`
  alanına yapıştırılır ve commit edilir (public key secret değildir).
- **Private key** (dosya içeriği) → GitHub repo secret `TAURI_SIGNING_PRIVATE_KEY`;
  anahtar bir parolayla korunduysa `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` da eklenir.
- CI wiring zaten hazır (`sdk-release.yml` bu iki secret'ı `tauri-action`'a
  geçiriyor) — ek workflow değişikliği gerekmez.

### 3 — Windows Authenticode sertifikası (ops)

1. Bir sertifika otoritesinden (DigiCert, Sectigo, vb.) OV veya EV code-signing
   sertifikası satın al (EV, SmartScreen uyarısını anında kaldırır).
2. Sertifikayı `.pfx` olarak export et, base64'e çevir: `base64 -w0 cert.pfx`.
3. GitHub repo → Settings → Secrets → Actions: `WINDOWS_CERTIFICATE` (base64
   içerik) ve `WINDOWS_CERTIFICATE_PASSWORD` secret'larını ekle.
4. Kod tarafında değişiklik gerekmiyor — `sdk-release.yml`'daki "Import
   Windows code-signing certificate" adımı bu iki secret'ı zaten bekliyor.
5. İlk gerçek sertifikalı release sonrası üretilen `.exe`'yi
   `signtool verify /pa` ile doğrula, bu satırı güncelle.

### 4 — macOS notarization (ops)

1. Apple Developer Program üyeliği (yıllık, yoksa satın alınmalı).
2. "Developer ID Application" sertifikası oluştur (Xcode ya da Apple Developer
   portal), `.p12` export edip base64'e çevir.
3. App-specific password oluştur (Apple ID ayarlarından, notarytool için).
4. GitHub secret'ları: `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`,
   `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`.
5. Kod tarafında değişiklik gerekmiyor — `sdk-release.yml` bu 6 secret'ı zaten
   `tauri-action`'a env olarak geçiriyor; `tauri-action` bunları algılayıp
   otomatik signing+notarization+stapling yapıyor.
6. İlk gerçek release sonrası `.dmg`'yi `spctl -a -vv /path/to/App.app` ile
   Gatekeeper doğrulaması yap, bu satırı güncelle.

### 5 — Gerçek cihaz doğrulama (QA, kod değil)

**Windows:**
1. Fiziksel bir Windows 10/11 makinede (VM değil — WASAPI loopback bazı
   sanallaştırılmış ses sürücülerinde farklı davranabilir), gerçek mikrofon +
   hoparlör/kulaklık bağlı olarak.
2. `sdk-v*` tag'i push edip CI'ın ürettiği `StreamDubber-SDK-Setup-x64.exe`'yi
   indir (imzasız olacak — SmartScreen "More info → Run anyway" gerekecek).
3. Kurulum sonrası gerçek OBS Studio ile bir Browser Source üzerinden
   overlay akışını test et (bkz. `docs/websocket-protocol.md`).
4. 6 haneli kodla pairing yap (`verify` skill'inin reçetesi kullanılabilir).
5. Mikrofon yakalamayı doğrula: konuş, `sdk/ui`'nin "Microphone" alanının
   "Capturing" gösterdiğini ve backend'e giden ses akışının gerçek
   transkripsiyon ürettiğini doğrula.
6. Gerçek bir RTMP relay bağlantısıyla (Mod A, `127.0.0.1:19350`) 10+
   dakikalık kesintisiz bir smoke-test yayını yap; ses düşmesi/gecikme
   birikmesi olup olmadığını gözlemle.
7. Sonucu bu tablonun 5. satırına ve "Durum" bölümüne işle.

**macOS:** Aynı checklist, fiziksel bir Mac üzerinde (Apple silicon veya
Intel — takımda yoksa kısa süreliğine cloud Mac kiralanabilir, ops kararı).
`sdk-macos-build-check` CI job'ı sadece derlemeyi doğruluyor, gerçek
mikrofon/hoparlör donanımı gerektiren smoke-test'in yerini tutmaz.

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
2. `plugins.updater.endpoints`: özel domain (`releases.strovak.com`) kurulana
   kadar GitHub Releases'in `latest.json` asset URL'sine işaret ediyor —
   `tauri-action` her `sdk-v*` release'inde bunu otomatik üretip yüklüyor.
3. Her release'te build artefaktları imzalanır; `latest.json` yeni sürümü işaret eder.
4. SDK açılışta endpoint'i sorar, imzayı public key ile doğrular, indirir, uygular.

### Runtime wiring — TAMAMLANDI
- `tauri-plugin-updater` bağımlılığı `sdk/src-tauri/Cargo.toml`'a eklendi, `main.rs`'te `.plugin(tauri_plugin_updater::Builder::new().build())` ile kaydedildi.
- SDK açılışta (`setup()` içinde, arka planda) `app.updater()?.check()` çağırır; yeni imzalı sürüm varsa `download_and_install` ile indirip kurar. **Bilinçli olarak `app.restart()` çağırmaz** — yayıncı yayın ortasındayken zorla yeniden başlatma riskini almaz.
- Her aşama (`Checking`/`UpToDate`/`Available`/`Downloading`/`Ready`/`Failed`) `update-status` Tauri event'i olarak yayınlanır; `sdk/ui/main.js` bunu dinleyip küçük bir banner gösterir. `Ready` durumunda banner bir "Restart now" butonu gösterir — bu, yeni `restart_app` komutunu çağırır (`commands.rs`), yani restart artık **kullanıcının elle onayladığı** bir aksiyon, updater'ın kendi kararı değil.
- Aşama sınıflandırması saf bir fonksiyona (`main.rs::classify_check_result`) çıkarıldı ve Tauri bağımlılığı olmadan unit test edildi.
- Not: `run_desktop_shell()` (platform-bağımsız çekirdek, `sdk/src/lib.rs`) kasıtlı olarak `AppHandle` almaz — updater `main.rs`'teki Tauri `Builder::setup()` katmanında yaşar, çekirdeğin Tauri'den habersiz kalması kuralına uygun.
- Kalan tek eksik: `plugins.updater.pubkey` boş (yukarıdaki "1 — Updater imza anahtar çifti" bölümüne bakın) — anahtar üretilip doldurulana kadar `app.updater()` runtime'da hata döner, kod bunu `tracing::warn!` ile loglar ve sessizce devam eder (uygulamayı bloklamaz, UI'da banner de görünmez).

## Durum (Faz 1.3)
- ✅ `sdk/ui` — gerçek Tauri penceresi (pairing formu + durum ekranı + update banner), `tauri build` artık gerçekten çalışıyor (önceden `frontendDist` boştu, hiç derlenmemişti).
- ✅ `.github/workflows/sdk-release.yml` — Windows/macOS/Linux matrix, `tauri-action` ile build→sign→GitHub Release. Updater imzalama (`TAURI_SIGNING_PRIVATE_KEY`) ve macOS notarization (`APPLE_*`) secret'ları boşken sessizce imzasız derler.
- ✅ `.github/workflows/ci.yml` — `sdk-macos-build-check` job'ı (informational, `continue-on-error`) her PR'da macOS derlemesini kontrol ediyor; Windows hâlâ sadece release workflow'unda deneniyor.
- ✅ Updater runtime wiring + UI banner + kullanıcı onaylı restart — SDK artık açılışta kendi güncelleme kontrolünü yapıyor ve durumunu gösteriyor (yukarıya bakın).
- ⚠️ Windows Authenticode: workflow PFX'i runner'a import edip thumbprint'i `tauri build --config` ile geçiriyor — gerçek bir sertifikayla hiç denenmedi (best-effort, `WINDOWS_CERTIFICATE` secret'ı yok). Ops bir sertifika sağladığında ilk gerçek release'te doğrulanmalı.
- ⏳ Gerçek sertifikalar (Windows/macOS), updater imza anahtar çifti — deploy zamanı ops işi (bkz. yukarıdaki bloker listesi altındaki adım adım bölümler).
- ⏳ Gerçek OBS + gerçek mikrofon (Windows/macOS) ile uçtan uca canlı doğrulama hiç yapılmadı — Linux mikrofon yakalaması gerçek cihazla doğrulandı, Windows/macOS kod tamam ama denenmedi.
