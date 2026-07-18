# Self-Hosted GPU STT — Sunucu Provisioning Planı (Faz 2, roadmap 2.6)

> Karar (2026-07-18): STT için admin **herhangi bir sağlayıcıdan**
> (Hetzner, OVH, Vast.ai, kendi donanımı — fark etmez) GPU'lu
> bir VPS/VDS açar, admin panelden sadece bağlantı bilgilerini girer; backend
> SSH ile bağlanıp gerekli yazılımı kurup sunucuyu "hazır" hale getirir.
> Bu, `docs/local-stt-plan.md`'nin **yayıncının kendi cihazında** çalışan
> yerel STT planından farklıdır — burası **bizim işlettiğimiz, bulut
> tarafındaki** GPU sunucusudur.

## 1. Neden bu tasarım

Roadmap'in STT kararı self-hosted `faster-whisper-large-v3` modelidir. Sorun:
GPU sunucusu boşta bile pahalıdır
— müşteri gelmeden önce ödemeye başlamak istemiyoruz. Çözüm: provisioning'i
**talep üzerine** admin panelden tetiklenebilir hale getirmek, VM'in kendisini
değil kurulumu otomatikleştirmek.

Üç mimari karar (kullanıcıyla netleştirildi):

| Karar | Seçilen | Neden |
|---|---|---|
| VM oluşturma | Manuel (admin kendi açar) | Hangi sağlayıcı/ne zaman/ne tip tamamen admin kontrolünde; Cloud API entegrasyonu kapsamı önemli ölçüde büyütürdü |
| SSH kimlik doğrulama | Backend keypair üretir | Private key hiçbir zaman tarayıcıya gitmez; admin sadece public key'i sunucuya ekler |
| STT servis yazılımı | Kendi minimal faster-whisper sarmalayıcımız | Model kararı zaten `docs/local-stt-plan.md §2`'de verilmiş (large-v3-turbo, CTranslate2); roadmap'in "faster-whisper" ifadesiyle uyumlu |

## 2. Mimari — uçtan uca akış

```
Admin panel                Backend (Rust)                    GPU VPS/VDS
─────────────              ────────────────                  ───────────
"Yeni Sunucu Ekle"    →    SttServerStore::add()
  (ad, host, ssh_port,        - ed25519 keypair üretir (ssh-keygen)
   ssh_user)                  - private key AES-256-GCM şifreli saklanır
                              - public key admin'e döner
                         ←    { id, ssh_public_key, status: "pending" }

Admin, public key'i           (admin sunucuyu oluştururken SSH key
sunucusuna ekler                olarak ekler / authorized_keys'e yapıştırır)

"Kur" butonu          →    POST /admin/stt-servers/{id}/provision
                              - status → "provisioning", 202 döner
                              - tokio::spawn: Provisioner::provision()
                                  - SSH bağlan (TOFU fingerprint pin/doğrula)
                                  - infra/stt-server/bootstrap.sh'i çalıştır
                                                                    →  nvidia-smi kontrolü
                                                                       Docker + nvidia-container-toolkit kur
                                                                       API_TOKEN üret
                                                                       docker compose up -d (stt-server image)
                                                                       /health poll
                                                                    ←  {"status":"ready","gpu_name":...,
                                                                         "vram_mb":...,"api_token":...,"port":...}
                              - store.mark_ready(...) → status: "ready"
Panel polling          →    GET /admin/stt-servers/{id}
                         ←    { status: "ready", endpoint_url, gpu_name, vram_mb }

Admin: stt.provider    →    PUT /admin/config {"stt.provider":"selfhosted"}
  = "selfhosted"            (mevcut whitelist — yeni config şeması gerekmedi)

Canlı yayın            →    Gateway::transcribe() → SelfHostedStt
                              - SttServerStore::active_ready() okur
                              - POST {endpoint}/transcribe (Bearer token)
                                                                    →  faster-whisper inference
                                                                    ←  {"text": ..., "is_final": true}
                              Hiç Ready sunucu yoksa → açık STT kullanılamıyor hatası
```

## 3. Bileşenler (dosya haritası)

| Bileşen | Dosya | Sorumluluk |
|---|---|---|
| Sunucu kayıt deposu | `backend/src/stt_servers/mod.rs` | CRUD, keypair üretimi, AES-256-GCM şifreli kalıcılık (`RelayTargetStore` şablonu) |
| Provisioning soyutlaması | `backend/src/stt_servers/provision.rs` | `Provisioner` trait, test için `FakeProvisioner`, prod için `SshProvisioner` |
| Admin HTTP uçları | `backend/src/stt_servers/mod.rs` (aynı dosya) | `/admin/stt-servers*` — `AdminUser` + audit log, `put_secret` deseniyle aynı |
| STT sağlayıcısı | `backend/src/gateway/selfhosted_stt.rs` | `SttProvider` impl'i, `SttServerStore::active_ready()` okur |
| STT servis yazılımı | `stt-server/main.py` | faster-whisper FastAPI sarmalayıcısı — `/transcribe`, `/health` |
| Servis imajı | `stt-server/Dockerfile` + `.github/workflows/stt-server-image.yml` | GHCR'a build+push |
| Kurulum script'i | `infra/stt-server/bootstrap.sh` | İdempotent: Docker+toolkit kur, image pull, compose up, health poll |

## 4. Güvenlik

- **SSH kimlik doğrulama**: private key backend'de üretilir (`ssh-keygen`),
  AES-256-GCM ile şifreli saklanır, hiçbir API yanıtında (`#[serde(skip_serializing)]`)
  ya da diskte düz metin görünmez. Admin sadece public key'i görür.
- **Host fingerprint (TOFU)**: ilk bağlantıda `ssh-keyscan` ile alınan
  fingerprint `SttServerStore`'a pinlenir; sonraki her bağlantıda doğrulanır.
  Uyuşmazlık (olası MITM veya sunucunun yeniden kurulmuş olması) sert hatayla
  sonuçlanır — sessizce trust-on-every-connect yapılmaz.
- **API token**: backend↔stt-server arası bearer token, bootstrap script
  tarafından rastgele üretilir, aynı SSH kanalı üzerinden backend'e döner
  (hiçbir zaman düz HTTP'de ilk kez taşınmaz), şifreli saklanır.
- **Ağ**: backend↔stt-server trafiği için genel TLS sertifika yönetimi bu
  aşamada kapsam dışı — `bootstrap.sh` sadece STT portunu (`ufw` ile) açar,
  gerçek yetkilendirme sınırı bearer token'dır. Prod dağıtımında backend ve
  GPU kutusunun aynı özel ağda (VPN/private network) olması önerilir.
- **Faz 1 dışı özellik sormadan implement etme** ilkesiyle: Hetzner Cloud API
  entegrasyonu (VM'in kendisini otomatik oluşturma) bilinçli olarak kapsam
  dışı bırakıldı.

## 5. Vast.ai uyumluluğu (2026-07-08 kullanıcı talebi)

Vast.ai, Hetzner/OVH gibi "kendi VM'ini kirala" modelinden farklı çalışır —
üç somut fark, hepsi ele alındı:

| Fark | Vast.ai gerçeği | Bu tasarımdaki karşılığı |
|---|---|---|
| SSH portu | Sabit 22 değil, instance başına rastgele atanır | `ssh_port` zaten serbest alan — admin Vast.ai'nin verdiği portu girer |
| SSH key ekleme zamanı | Hesap ayarlarına eklenen key sadece **yeni** instance'lara uygulanır; var olan instance'a eklemek için instance'a özel arayüz gerekir | Panel kopyası bunu açıkça anlatır (`publicKeyIntro`) — admin key'i Vast.ai hesabına instance'ı OLUŞTURMADAN ÖNCE ekler |
| Docker/systemd | Vast.ai'nin varsayılan instance tipi **kendisi zaten bir Docker container'ıdır** (host üzerinde) — içinde nested Docker daemon/systemd genelde yok | `bootstrap.sh` önce Docker'ın gerçekten kullanılabilir olup olmadığını tespit eder (`docker info` başarılı mı, systemd var mı); kullanılamıyorsa STT servisini Docker'a hiç dokunmadan bir Python venv içinde çalıştırır ("bare mode" — `nohup` + kendi kendini yeniden başlatan `while` döngüsü). `SshProvisioner` her provision çağrısında `main.py`/`requirements.txt`'i (base64 ile, heredoc/tırnak çakışması riski olmadan) uzak makineye zaten yazar — bare mode bunları kullanır, Docker modunda zararsızca kullanılmadan durur |
| Port NAT eşleme | Dışarıdan erişilebilir port, servisin bağlandığı iç porttan farklı olabilir (host port-forwarding yapar) | Yeni **`public_endpoint_override`** alanı (sunucu eklerken opsiyonel) — doluysa `SshProvisioner` bunu `endpoint_url` yerine kullanır |

Vast.ai'nin "Ubuntu VM" instance tipi (gerçek VM, systemd var) seçilirse
normal Docker yolu sorunsuz çalışır — bare mode sadece varsayılan container
instance tipi için devreye girer.

## 6. Durum (2026-07-08)

Tamamlanan: sunucu kayıt deposu + keypair üretimi, provisioning durum
makinesi + admin route'ları, gerçek `SshProvisioner` (ssh/ssh-keyscan/
ssh-keygen shell-out, TOFU fingerprint pinleme, Docker/bare-mode tespiti —
Vast.ai dahil), `SelfHostedStt` gateway sağlayıcısı, `stt-server` FastAPI
servisi + Docker image + CI, `bootstrap.sh` kurulum script'i, admin
frontend paneli ("STT Servers").

Kalan: gerçek bir GPU VPS/VDS'te (hem Hetzner/OVH tipi hem Vast.ai container
instance tipi için ayrı ayrı) canlı smoke test + Türkçe kalite ölçümü.
