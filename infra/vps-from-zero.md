# Sıfır VPS'ten Staging'e — Adım Adım Kurulum

> Hedef: boş bir Ubuntu VPS'ten `https://dev.strovak.com` / `https://dev.api.strovak.com` /
> `https://ext-staging.strovak.com` adreslerinde çalışan, yedeklenen staging ortamına gitmek.
> Bu rehber `infra/staging.md`'nin "First deployment" bölümünün **öncesini** anlatır ve onun
> başladığı yerde biter. Komutlar `development-vps` branch'indeki script'lere göre yazıldı;
> script'lerin yaptığı işi tekrar elle yapma.

Kısa yol (her adımın ayrıntısı aşağıda):

1. VPS + DNS → 2. operatör hesabı + SSH anahtarı → 3. paketler → 4. repo (deploy key ile clone)
→ 5. `staging-host-bootstrap.sh` → 6. `staging.env`'e admin + basic-auth değerleri
→ 7. `staging-deploy.sh` → 8. `staging-status.sh` → 9. panelden provider/OAuth secret'ları.

## 0. Ne gerekiyor

| Şey | Neden |
| --- | --- |
| Ubuntu 24.04 LTS VPS, sabit IPv4 | Script'ler Debian/Ubuntu varsayar (`ufw`, `ssh.service`, `sudo` grubu). Paket listesi 24.04'te doğrulandı. |
| Diskte en az **12 GiB boş** (deploy ön kontrolü), toplam 40 GB önerilir | Rust + Next.js image build'leri BuildKit cache ile çok yer tutar; `staging-deploy.sh` 12 GiB altında başlamaz. |
| RAM: konteyner limitleri toplamı ~2,3 GiB; derleme için 8 GB önerilir | 4 GB'lık makinede ilk `cargo build --release` sırasında swap aç (aşağıda). |
| `strovak.com` DNS yönetimi | Üç A kaydı gerekir (adım 1). |
| GitHub repo'ya erişim | Repo private; VPS'e salt-okunur **deploy key** eklenir (adım 4). |
| Geliştirme makinende Rust toolchain | `ADMIN_PASSWORD_HASH` için `cargo run --bin hash_password` (adım 6). VPS'te cargo yok ve gerekmez. |

Sağlayıcı tarafında bir "cloud firewall" varsa SSH (22), 80 ve 443'ü orada da aç; UFW'yi
bootstrap script'i kendisi ayarlar.

## 1. DNS kayıtları

Üç A kaydı, üçü de VPS'in IPv4'üne:

```
dev.strovak.com          A  <VPS-IPv4>
dev.api.strovak.com      A  <VPS-IPv4>
ext-staging.strovak.com  A  <VPS-IPv4>
```

- Host'ta global IPv6 yoksa AAAA kaydı ekleme.
- İlk kurulumda üçünü de **proxy'siz (DNS-only)** bırak; Caddy sertifikaları doğrudan
  Let's Encrypt'ten alır. Sonradan `dev`/`ext-staging` için CDN proxy açacaksan `infra/staging.md`
  "First deployment" bölümündeki şartlar geçerli: CDN→origin modu *tam sertifika doğrulama*
  (Full strict), ve edge sertifikası üç adı da kapsamalı (`*.strovak.com` wildcard'ı
  `dev.api.strovak.com`'u kapsamaz). `dev.api.strovak.com` proxy'siz kalmalı: SDK ve tarayıcı
  WebSocket'leri doğrudan origin'e gider ve `STAGING_PUBLIC_IPV4` bu kaydın çözüldüğü adrestir.
- DNS yayılması deploy'u bekletmez: `staging-deploy.sh --origin-only` DNS'e bakmadan kurar
  (adım 7), DNS oturunca `staging-status.sh` koşulur.

## 2. Operatör hesabı ve SSH anahtarı

Sağlayıcının verdiği ilk oturumla (root ya da `ubuntu`) gir. Bootstrap script'i **root olarak
değil, sudo'lu bir operatör hesabından** çalışır ve şunları ister: hesap `sudo` grubunda,
`~/.ssh/authorized_keys` dolu, `sudo -n true` parolasız geçiyor.

```bash
# root iken:
adduser --disabled-password --gecos "" operator
usermod -aG sudo operator
echo 'operator ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/90-operator && chmod 0440 /etc/sudoers.d/90-operator
install -d -m 0700 -o operator -g operator /home/operator/.ssh
# Kendi public key'ini yapıştır (ör. yerelde: cat ~/.ssh/id_ed25519.pub):
install -m 0600 -o operator -g operator /dev/stdin /home/operator/.ssh/authorized_keys <<'KEY'
ssh-ed25519 AAAA... senin-anahtarin
KEY
```

Devam etmeden önce **ikinci bir terminalden** `ssh operator@<VPS-IPv4>` ile girebildiğini
doğrula. Bootstrap, parola ile SSH'ı ve root girişini kapatır; anahtar çalışmıyorsa kilitlenirsin
(kurtarma: sağlayıcının web konsolu).

RAM 4 GB veya altıysa derleme için swap aç (bir kerelik):

```bash
fallocate -l 4G /swapfile && chmod 600 /swapfile && mkswap /swapfile && swapon /swapfile
echo '/swapfile none swap sw 0 0' >> /etc/fstab
```

## 3. Paketler

Operatör hesabıyla devam et. Bootstrap'in aradığı komutlar: `docker openssl ufw iptables sshd
systemctl curl git tar flock python3 df ip`. Ubuntu 24.04'te hepsi şu paketlerden gelir
(Docker 29.x, Compose 2.40.x, Buildx 0.30.x — Ubuntu'nun kendi paketleri yeterli):

```bash
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  docker.io docker-compose-v2 docker-buildx \
  ufw git curl openssl python3 iptables iproute2 util-linux tar ca-certificates
sudo systemctl enable --now docker
docker --version && docker compose version && docker buildx version
```

Operatörü `docker` grubuna **ekleme**: bütün staging script'leri zaten `sudo` ile koşar ve
`staging-status.sh` host'ta fazladan açık yüzey arar.

## 4. Repo'yu VPS'e al (deploy key)

Repo private olduğu için VPS'e salt-okunur bir deploy key tanımla; kişisel anahtarını VPS'e
kopyalama.

```bash
ssh-keygen -t ed25519 -N "" -C "streamdubber-staging-vps" -f ~/.ssh/github_streamdubber
cat ~/.ssh/github_streamdubber.pub   # → GitHub: repo → Settings → Deploy keys → Add (read-only)
cat >> ~/.ssh/config <<'CFG'
Host github.com
  IdentityFile ~/.ssh/github_streamdubber
  IdentitiesOnly yes
CFG
chmod 0600 ~/.ssh/config
ssh -T git@github.com   # "Hi Rashandd/Stream-translator-live! You've successfully authenticated" görmelisin
```

Clone — dizin adı sabittir, `CLAUDE.md`'deki tek staging worktree kuralıyla aynı
(`streamdubber-development-full-vps`), branch **`development-vps`**:

```bash
git clone -b development-vps git@github.com:Rashandd/Stream-translator-live.git ~/streamdubber-development-full-vps
cd ~/streamdubber-development-full-vps
git log -1 --oneline
```

`main`'i clone'lama: staging katmanı (`infra/staging-*`, `compose.staging.yml`,
`Caddyfile.staging`) bilinçli olarak yalnız `development-vps`'te durur.

Deploy script'i çalışma ağacının **temiz** olmasını ister: izlenen değişiklik ve izlenmeyen dosya
olmamalı (`claude.md` / `codex.md` gitignore'da, sorun değil). VPS'te kod düzenleme; değişiklik
geliştirme makinesinde yapılır, push edilir, VPS'te `git pull --ff-only` ile alınır.

## 5. Host bootstrap (bir kez, idempotent)

```bash
cd ~/streamdubber-development-full-vps
sudo infra/staging-host-bootstrap.sh
```

Ne yapar: `streamdubber` sistem kullanıcısı ve `/srv/streamdubber/{releases,backups}`;
`/etc/streamdubber/staging.env` (root, 0600) içine Postgres/Redis parolası, `JWT_SECRET`,
`SECRETS_ENC_KEY`, `STAGING_PUBLIC_IPV4` üretir (değerleri **basmaz**); SSH'ta parola ve root
girişini kapatır; UFW'yi yalnız SSH portu + 80/443 TCP + 443 UDP'ye indirir; Docker'ın UFW'yi
baypas etmesini engelleyen `DOCKER-USER` zincirini ve günlük yedek timer'ını kurar.

Script bir şartı sağlayamazsa değişiklik yapmadan açık mesajla durur (operatör `sudo` grubunda
değil, `authorized_keys` boş, komut eksik, `STAGING_PUBLIC_IPV4` çözülemedi). Mesajı düzeltip
tekrar çalıştır.

`STAGING_PUBLIC_IPV4`'ü `ip route get` ile buluyor. Sağlayıcı NAT arkasında özel IP veriyorsa
(ör. bazı bulutlar) dosyadaki değer özel IP olur; bu Caddy'nin bind adresidir ve doğrudur,
DNS'teki A kaydı yine genel IP'yi gösterir.

## 6. staging.env'i tamamla

Bootstrap iki şeyi boş bırakır; ikisi de senin:

```bash
sudo nano /etc/streamdubber/staging.env
```

**a) Admin girişi** — `ADMIN_EMAIL` ve `ADMIN_PASSWORD_HASH` (argon2id). Hash'i geliştirme
makinende üret, parola komut satırına girmez:

```bash
# geliştirme makinesinde, repo kökünde
cargo run --bin hash_password        # Password: ... → $argon2id$v=19$m=...$...$...
```

Çıkan satırı **tek tırnak içinde** yaz; yoksa Compose `$` işaretlerini değişken sanır ve deploy
script'i tırnaksız hash'i reddeder:

```dotenv
ADMIN_EMAIL=sen@ornek.com
ADMIN_PASSWORD_HASH='$argon2id$v=19$m=19456,t=2,p=1$...$...'
```

**b) Edge Basic Auth** — `dev.strovak.com` URL'yi bilen herkese açık olmasın diye Caddy önüne
HTTP Basic Auth koyar (`dev.api.strovak.com` ve `ext-staging.strovak.com` açık kalır; SDK,
tarayıcı API çağrıları ve Twitch Hosted Test bozulmaz). İki değer **zorunludur**: biri boşsa
Caddy yine ayağa kalkar ama herkese 401 döner; bu yüzden `staging-deploy.sh` ikisi dolmadan
build'e başlamaz. Hash bcrypt olmalı, VPS'te üret (parola prompt'ta sorulur, komut satırına
yazılmaz):

```bash
docker run --rm -it caddy:2.11.4-alpine caddy hash-password    # → $2a$14$...
```

```dotenv
STAGING_BASIC_AUTH_USER=sectigin-kullanici
STAGING_BASIC_AUTH_HASH='$2a$14$...'
```

Dosyanın sonunda şu altı anahtar dolu olmalı: `ADMIN_EMAIL`, `ADMIN_PASSWORD_HASH`,
`STAGING_BASIC_AUTH_USER`, `STAGING_BASIC_AUTH_HASH` (+ bootstrap'in ürettiği
`POSTGRES_PASSWORD`, `REDIS_PASSWORD`, `JWT_SECRET`, `SECRETS_ENC_KEY`, `STAGING_PUBLIC_IPV4`).
Bu dosya repo'ya **asla** girmez; yedeklerde de yoktur, ayrı bir kanaldan sakla.

## 7. İlk deploy

```bash
cd ~/streamdubber-development-full-vps
bash infra/staging-validate.sh          # script sözdizimi, compose şeması, Caddyfile kontrolleri (root gerekmez)
sudo infra/staging-deploy.sh --origin-only
```

`--origin-only`: her şeyi kurar ve origin'e karşı TLS, HTTPS yönlendirme, güvenlik başlıkları,
CORS, WebSocket upgrade, EBS kısıtı ve frontend gate kontrollerini koşar; **yalnız** üç genel DNS
sorgusunu atlar. DNS zaten oturduysa `--origin-only`'siz çalıştırabilirsin.

İlk çalıştırma uzun sürer: Postgres/Redis/Caddy image'ları çekilir, backend (Rust, release) ve
frontend (Next.js) image'ları sıfırdan derlenir — makineye göre on dakikalardan yarım saate.
Sonraki deploy'lar cache sayesinde çok daha kısa.

Script sonunda `Activated immutable staging release <commit>` yazmalı. Kurulum
`/srv/streamdubber/releases/<commit>` altına yapılır; `/srv/streamdubber/current` ona işaret eder.

Bir adım başarısız olursa script kendi geri alma yolunu izler ve nedenini stderr'e yazar; sık
görülenler için aşağıdaki "Sorun giderme" bölümüne bak. Düzeltip **aynı komutu** tekrar çalıştır;
deploy idempotenttir.

## 8. Doğrulama

```bash
sudo /srv/streamdubber/current/infra/staging-status.sh
```

Bu, sürüm kapısıdır: beş konteynerin sağlığı, `staging.env` izinleri, iç ağın gerçekten
`internal` olması, tüm health kontrolleri (bu kez genel DNS dâhil), yalnız Caddy'nin port
yayınlaması, yasak portların (3000/5432/6379/8080/8200) loopback dışında dinlenmemesi, UFW ve
`DOCKER-USER` kuralları, nginx'in Caddy'yle yarışmaması. Son satır
`Staging service, edge, firewall, and isolation checks passed.` olmalı.

Tarayıcıdan:

- `https://dev.strovak.com` → önce Basic Auth penceresi (adım 6b), sonra site. Admin paneline
  `ADMIN_EMAIL` + parolanla gir.
- `https://dev.api.strovak.com/health` → 200.
- `https://ext-staging.strovak.com/api/plans` → 404 (EBS host'u yalnız `/health` ve
  `/ext/twitch/*` servis eder; bu beklenen davranış).

Health script'i `dev.strovak.com` için **401 + `WWW-Authenticate: Basic`** yanıtını sağlıklı sayar
(gate'in arkasındaki frontend'e Caddy konteynerinin içinden ayrıca bakar); kimlik bilgisi
olmadan 2xx dönmesi gate'in kapalı olduğu anlamına gelir ve kontrol **başarısız** olur.

## 9. Panelden girilecekler

Deploy, `service_config`'e staging URL'lerini ve `providers.mock_fallback=false` değerini
yazar (`infra/staging-config.sql`). Yani sahte sağlayıcı yok: STT/çeviri/OAuth çalışsın
istiyorsan gerçek staging anahtarları Admin → Secrets'tan girilir. Hangi secret adı hangi
callback URL ile, `infra/staging.md` "OAuth console configuration" tablosunda. Kurallar:

- Yalnız staging'e özel OAuth uygulamaları; production client secret'ı, refresh token'ı ya da
  kullanıcı veritabanı bu ortama kopyalanmaz.
- Değerler `staging_backend_data` volume'ündeki `/data/secrets.enc` içinde, staging'in kendi
  `SECRETS_ENC_KEY`'iyle şifreli durur.

Bilinen sınır: OBS Browser Source için üretilen overlay URL'si (`https://dev.strovak.com/o/<token>`)
de Basic Auth'un arkasındadır ve OBS parola soramaz. Staging'de overlay denemesi için URL'ye
kimlik gömmen gerekir (`https://kullanici:parola@dev.strovak.com/o/<token>`); production'da bu
gate yoktur.

## 10. Günlük işletim

Komutların tamamı ve yedek/geri yükleme/rollback akışı `infra/staging.md` "Routine operations"
bölümünde. Özet:

```bash
sudo /srv/streamdubber/current/infra/staging-status.sh            # sağlık + izolasyon kapısı
sudo /srv/streamdubber/current/infra/staging-logs.sh backend caddy # log takibi
sudo /srv/streamdubber/current/infra/staging-backup.sh             # elle yedek (timer her gün 02:15 UTC)
```

Yeni bir sürüm almak için VPS'te `git pull --ff-only origin development-vps` sonra
`sudo infra/staging-deploy.sh`; `main`'deki değişiklikleri `development-vps`'e taşıma akışı
`docs/commands/deploy-staging.md`'de. Deploy her seferinde önce mevcut sürümün veritabanı +
şifreli dosya deposu yedeğini alır (`.predeploy-backup`), rollback bu yedeğe döner.

## Sorun giderme

| Mesaj / belirti | Anlamı ve çözüm |
| --- | --- |
| `Invoke this script through sudo from the non-root operator account.` | Bootstrap root'tan çağrıldı. Operatör hesabına geç, `sudo infra/staging-host-bootstrap.sh`. |
| `The operator account cannot complete a non-interactive sudo check.` | `sudo -n true` parola istiyor. Adım 2'deki `sudoers.d` satırı eksik. |
| `... is missing a non-empty STAGING_BASIC_AUTH_USER value` / `STAGING_BASIC_AUTH_HASH must be a single-quoted bcrypt value` | Adım 6b. Hash `$2a$14$` ile başlar, tek tırnak içinde olur. |
| `ADMIN_PASSWORD_HASH must be empty or a single-quoted argon2id PHC value.` | Hash tırnaksız yazılmış. Adım 6a. |
| `Tracked changes are not committed` / `Untracked files are present` | Deploy çalışma ağacı temiz ister. `git status`; VPS'te düzenleme yapma, `git checkout -- .` ve fazla dosyaları sil. |
| `Insufficient free disk space for staging Docker builds` | Üç dosya sisteminde de ≥12 GiB gerekir. `docker system df`, gereksiz cache/image temizliği ya da disk büyüt. |
| Health `public_get`/DNS adımında düşüyor, origin kontrolleri geçiyor | DNS/CDN henüz oturmamış. `--origin-only` ile kur, DNS düzelince `staging-status.sh`. |
| `Nginx still competes for the Caddy edge ports.` | Host'ta nginx kurulu ve devre dışı bırakılamadı. `systemctl disable --now nginx`, tekrar deploy. (Nginx hiç kurulu değilse bu adım otomatik atlanır.) |
| `Another staging deploy, backup, restore, or rollback is already running` | `/etc/streamdubber/staging-operation.lock` tutuluyor. Diğer işlemin bitmesini bekle; kilidi elle silme. |
| `OpenClaw port 18789 is not loopback-only; refusing deployment.` | Host'ta 18789'u genel adrese bağlayan bir servis var. Loopback'e al ya da durdur. |
| SSH'a giremiyorum | Bootstrap parola girişini kapattı. Sağlayıcı konsolundan gir, `/home/operator/.ssh/authorized_keys`'i düzelt; `PasswordAuthentication` ayarı `/etc/ssh/sshd_config.d/00-streamdubber-hardening.conf`. |
