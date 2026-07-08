# Self-Hosted GPU STT — Sunucu Provisioning Planı (Faz 2, roadmap 2.6)

> Karar (2026-07-08): Deepgram maliyeti ölçeklenmediğinde admin **herhangi bir
> sağlayıcıdan** (Hetzner, OVH, Vast.ai, kendi donanımı — fark etmez) GPU'lu
> bir VPS/VDS açar, admin panelden sadece bağlantı bilgilerini girer; backend
> SSH ile bağlanıp gerekli yazılımı kurup sunucuyu "hazır" hale getirir.
> Bu, `docs/local-stt-plan.md`'nin **yayıncının kendi cihazında** çalışan
> yerel STT planından farklıdır — burası **bizim işlettiğimiz, bulut
> tarafındaki** GPU sunucusu (Deepgram'ın doğrudan yerine geçer).

## 1. Neden bu tasarım

Roadmap zaten bunu 2.6'da öngörmüştü: "STT: Deepgram API → faster-whisper
(local), maliyet %92 ↓" (satır 176). Sorun: GPU sunucusu boşta bile pahalıdır
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
                              Hiç Ready sunucu yoksa → stt.fallback (Deepgram)
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

## 5. Durum (2026-07-08)

Tamamlanan: sunucu kayıt deposu + keypair üretimi, provisioning durum
makinesi + admin route'ları (`FakeProvisioner` ile uçtan uca test edilmiş),
`SelfHostedStt` gateway sağlayıcısı, `stt-server` FastAPI servisi + Docker
image + CI, `bootstrap.sh` kurulum script'i.

Kalan: gerçek `SshProvisioner` (ssh/scp shell-out + TOFU fingerprint
pinleme — bugün `FakeProvisioner` her zaman açık bir hatayla başarısız olur,
sessizce "hazır" gibi davranmaz), admin frontend paneli ("STT Sunucuları"),
gerçek bir GPU VPS/VDS'te canlı smoke test + TR kalite karşılaştırması
(Deepgram vs kendi faster-whisper'ımız).
