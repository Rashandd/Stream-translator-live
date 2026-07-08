//! Self-hosted STT sunucu kayıtları (roadmap 2.6 — "faster-whisper Geçişi").
//!
//! Deepgram maliyeti ölçeklenmediğinde admin kendi GPU'lu VPS/VDS'ini (hangi
//! sağlayıcıdan olursa olsun — Hetzner/OVH/Vast.ai/kendi donanımı, fark
//! etmez) panelden ekler; backend SSH ile bağlanıp faster-whisper servisini
//! kurar (bkz. `provision` modülü). Bu modül sadece kayıt/durum deposu:
//! `RelayTargetStore` ile aynı desen — in-memory + opsiyonel AES-256-GCM
//! şifreli tek dosya kalıcılığı (`Cipher`). Yeni bir Postgres migration
//! gerekmiyor; bu proje admin-yönetimli kaynaklar için zaten bu daha hafif
//! deseni kullanıyor (bkz. `relay`, `voice_models`, `sdk_link` store'ları).
//!
//! Private key ve API token diskte/JSON yanıtlarında asla düz metin
//! görünmez — `#[serde(skip_serializing)]` + AES-256-GCM (aynı `Cipher`
//! ilkel, `secrets`/`relay` modülleriyle aynı bağlama-özel anahtar türetimi).

pub mod provision;

use std::collections::HashMap;
use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, RwLock};

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::auth::authz::AdminUser;
use crate::crypto::Cipher;
use crate::logging::{LogLevel, Logger};
use provision::Provisioner;

/// Bir sunucu kaydının yaşam döngüsü durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SttServerStatus {
    /// Eklendi, henüz kurulum denenmedi.
    Pending,
    /// Kurulum (SSH bootstrap) sürüyor.
    Provisioning,
    /// Kurulum bitti, `/transcribe` isteklerine hazır.
    Ready,
    /// Son kurulum/health-check denemesi başarısız — `status_detail` sebebi taşır.
    Error,
}

/// Self-hosted bir STT sunucusu.
///
/// Şifreli alanlar (`ssh_private_key_encrypted`, `api_token_encrypted`) bilinçli
/// olarak `pub(crate)` değil `pub`'dır ama `#[serde(skip_serializing)]` ile
/// hiçbir JSON yanıtına karışmaz — panel bunları asla görmez/görmemeli.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SttServer {
    pub id: String,
    pub name: String,
    pub hostname: String,
    pub ssh_port: u16,
    pub ssh_user: String,
    /// Panelde gösterilir — admin bunu sunucusuna `authorized_keys` olarak ekler.
    pub ssh_public_key: String,
    #[serde(skip_serializing)]
    pub ssh_private_key_encrypted: String,
    /// TOFU: ilk başarılı bağlantıda pinlenir, sonrasında değişirse sert hata.
    #[serde(default)]
    pub host_fingerprint: Option<String>,
    /// Dışarıdan erişilebilir gerçek uç nokta NAT'lı port eşleme yüzünden
    /// bootstrap'ın bildirdiğinden farklıysa (ör. Vast.ai: SSH portu ve
    /// STT servis portu host tarafından farklı dış portlara eşlenebilir)
    /// admin bunu sunucu eklerken elle girer — doluysa `SshProvisioner`
    /// bunu `endpoint_url` yerine kullanır.
    #[serde(default)]
    pub public_endpoint_override: Option<String>,
    pub status: SttServerStatus,
    #[serde(default)]
    pub status_detail: Option<String>,
    #[serde(default)]
    pub endpoint_url: Option<String>,
    #[serde(default, skip_serializing)]
    pub api_token_encrypted: Option<String>,
    #[serde(default)]
    pub gpu_name: Option<String>,
    #[serde(default)]
    pub vram_mb: Option<u64>,
    #[serde(default)]
    pub last_health_check_at: Option<i64>,
    pub created_at: i64,
}

/// Store hataları.
#[derive(Debug, thiserror::Error)]
pub enum SttServerError {
    #[error("stt server store io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("stt server store crypto error: {0}")]
    Crypto(String),
    #[error("stt server store serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
    #[error("stt server not found: {0}")]
    NotFound(String),
}

/// Keypair üretim hatası ayrı tutulur — `add()` bunu `SttServerError::Invalid`'e sarar.
#[derive(Debug, thiserror::Error)]
pub enum KeygenError {
    #[error("ssh-keygen failed: {0}")]
    Command(String),
    #[error("ssh-keygen output unreadable: {0}")]
    Io(#[from] std::io::Error),
}

/// `ssh-keygen`'e shell-out ederek ed25519 anahtar çifti üretir.
///
/// Yeni bir Rust SSH crate'i eklemek yerine (bu projenin `gpu_probe.rs`'de
/// `nvidia-smi`'yi shell-out ettiği tercihle tutarlı) sistemde zaten var olan
/// openssh-client paketini kullanır — hem provisioning (adım 6) hem burada
/// aynı araç ailesi. Geçici dosyalar üretim sonrası silinir.
fn generate_keypair() -> Result<(String, String), KeygenError> {
    let dir = std::env::temp_dir().join(format!("sd-sshkey-{}", uuid_like()));
    std::fs::create_dir_all(&dir)?;
    let priv_path = dir.join("id_ed25519");

    let output = std::process::Command::new("ssh-keygen")
        .args([
            "-t",
            "ed25519",
            "-N",
            "",
            "-C",
            "streamdubber-stt-server",
            "-f",
        ])
        .arg(&priv_path)
        .output()
        .map_err(|e| KeygenError::Command(e.to_string()))?;

    if !output.status.success() {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(KeygenError::Command(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let private_key = std::fs::read_to_string(&priv_path)?;
    let public_key = std::fs::read_to_string(priv_path.with_extension("pub"))?;
    let _ = std::fs::remove_dir_all(&dir);
    Ok((private_key, public_key.trim().to_string()))
}

/// Kriptografik olmayan basit benzersiz isim üreticisi (geçici dizin adı için;
/// kimlik/anahtar üretiminde kullanılmaz — sadece dosya sistemi çakışmasını önler).
fn uuid_like() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos}-{}", std::process::id())
}

/// Kayıt kimliği için basit rastgele id (Postgres UUID'e ihtiyaç yok — bu
/// store dosya tabanlı, bkz. modül dokümanı).
fn new_id() -> String {
    uuid_like()
}

fn now_ts() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Self-hosted STT sunucu kayıtları deposu.
pub struct SttServerStore {
    entries: RwLock<HashMap<String, SttServer>>,
    persist: Option<(PathBuf, Cipher)>,
}

impl SttServerStore {
    /// Kalıcılıksız depo (dev/test).
    pub fn in_memory() -> Self {
        SttServerStore {
            entries: RwLock::new(HashMap::new()),
            persist: None,
        }
    }

    /// Şifreli dosya kalıcılığıyla kurar; dosya varsa kayıtları yükler.
    pub fn with_file(path: impl Into<PathBuf>, key: &[u8]) -> Result<Self, SttServerError> {
        let path = path.into();
        let cipher = Cipher::new(key).map_err(|e| SttServerError::Crypto(e.to_string()))?;
        let entries = load_entries(&path, &cipher)?;
        Ok(SttServerStore {
            entries: RwLock::new(entries),
            persist: Some((path, cipher)),
        })
    }

    /// Yeni bir sunucu kaydı ekler: SSH keypair üretir, private key'i
    /// şifreleyip saklar, public key'i (admin'in sunucusuna ekleyeceği)
    /// döner. Kayıt `Pending` durumunda başlar.
    ///
    /// `public_endpoint_override`: NAT'lı port eşleme kullanan sağlayıcılar
    /// için (ör. Vast.ai — STT servis portu dışarıya farklı bir port'tan
    /// açılabilir) admin'in elle girdiği gerçek erişim adresi. Çoğu
    /// sağlayıcıda (Hetzner/OVH/kendi donanımı) `None` yeterlidir.
    pub fn add(
        &self,
        name: &str,
        hostname: &str,
        ssh_port: u16,
        ssh_user: &str,
        public_endpoint_override: Option<String>,
    ) -> Result<SttServer, SttServerError> {
        if name.trim().is_empty() || hostname.trim().is_empty() || ssh_user.trim().is_empty() {
            return Err(SttServerError::Invalid(
                "name, hostname and ssh_user must not be empty".into(),
            ));
        }
        let (private_key, public_key) =
            generate_keypair().map_err(|e| SttServerError::Invalid(e.to_string()))?;
        let encrypted = self.encrypt(&private_key)?;

        let server = SttServer {
            id: new_id(),
            name: name.to_string(),
            hostname: hostname.to_string(),
            ssh_port,
            ssh_user: ssh_user.to_string(),
            ssh_public_key: public_key,
            ssh_private_key_encrypted: encrypted,
            host_fingerprint: None,
            public_endpoint_override: public_endpoint_override
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            status: SttServerStatus::Pending,
            status_detail: None,
            endpoint_url: None,
            api_token_encrypted: None,
            gpu_name: None,
            vram_mb: None,
            last_health_check_at: None,
            created_at: now_ts(),
        };

        {
            let mut map = self
                .entries
                .write()
                .map_err(|_| SttServerError::Crypto("stt server lock poisoned".into()))?;
            map.insert(server.id.clone(), server.clone());
        }
        self.save()?;
        Ok(server)
    }

    /// Tüm kayıtları döner (private key/token zaten `Serialize` düzeyinde
    /// gizli — bu ekstra bir maskeleme gerektirmez, ama çağıran kod yine de
    /// asla ham alanlara `pub` erişimle JSON dışına sızdırmamalı).
    pub fn list(&self) -> Vec<SttServer> {
        self.entries
            .read()
            .map(|m| {
                let mut v: Vec<SttServer> = m.values().cloned().collect();
                v.sort_by_key(|s| s.created_at);
                v
            })
            .unwrap_or_default()
    }

    /// Tek bir kaydı döner.
    pub fn get(&self, id: &str) -> Option<SttServer> {
        self.entries.read().ok().and_then(|m| m.get(id).cloned())
    }

    /// Bir kaydı siler. Yoksa `NotFound`.
    pub fn remove(&self, id: &str) -> Result<(), SttServerError> {
        let removed = {
            let mut map = self
                .entries
                .write()
                .map_err(|_| SttServerError::Crypto("stt server lock poisoned".into()))?;
            map.remove(id).is_some()
        };
        if !removed {
            return Err(SttServerError::NotFound(id.to_string()));
        }
        self.save()
    }

    /// Durumu + (varsa) detay mesajını günceller.
    pub fn set_status(
        &self,
        id: &str,
        status: SttServerStatus,
        detail: Option<String>,
    ) -> Result<(), SttServerError> {
        self.update(id, |server| {
            server.status = status;
            server.status_detail = detail;
        })
    }

    /// İlk başarılı SSH bağlantısında host fingerprint'i pinler (TOFU).
    pub fn pin_fingerprint(&self, id: &str, fingerprint: &str) -> Result<(), SttServerError> {
        let fingerprint = fingerprint.to_string();
        self.update(id, |server| {
            server.host_fingerprint = Some(fingerprint.clone());
        })
    }

    /// Kurulum başarıyla bitince: endpoint + (şifrelenmiş) API token + GPU
    /// bilgisini yazar, durumu `Ready` yapar.
    pub fn mark_ready(
        &self,
        id: &str,
        endpoint_url: &str,
        api_token: &str,
        gpu_name: Option<String>,
        vram_mb: Option<u64>,
    ) -> Result<(), SttServerError> {
        let encrypted_token = self.encrypt(api_token)?;
        let endpoint_url = endpoint_url.to_string();
        let now = now_ts();
        self.update(id, |server| {
            server.status = SttServerStatus::Ready;
            server.status_detail = None;
            server.endpoint_url = Some(endpoint_url.clone());
            server.api_token_encrypted = Some(encrypted_token.clone());
            server.gpu_name = gpu_name.clone();
            server.vram_mb = vram_mb;
            server.last_health_check_at = Some(now);
        })
    }

    /// En son `Ready` olan kaydı + çözülmüş (düz metin) API token'ını döner —
    /// `SelfHostedStt` sağlayıcısı bunu her çağrıda okur (restart gerekmeden
    /// admin yeni sunucuya geçebilir).
    pub fn active_ready(&self) -> Option<(SttServer, String)> {
        let candidate = self
            .entries
            .read()
            .ok()?
            .values()
            .filter(|s| s.status == SttServerStatus::Ready && s.endpoint_url.is_some())
            .max_by_key(|s| s.last_health_check_at.unwrap_or(0))
            .cloned()?;
        let token_enc = candidate.api_token_encrypted.clone()?;
        let token = self.decrypt(&token_enc).ok()?;
        Some((candidate, token))
    }

    /// Bir sunucunun SSH private key'ini çözer (düz metin) — provisioning
    /// (`SshProvisioner`) bunu ihtiyaç anında çağırır, private key hiçbir
    /// yerde uzun süre düz metin tutulmaz.
    pub fn decrypt_private_key(&self, server: &SttServer) -> Result<String, SttServerError> {
        self.decrypt(&server.ssh_private_key_encrypted)
    }

    fn update(
        &self,
        id: &str,
        mutate: impl FnOnce(&mut SttServer),
    ) -> Result<(), SttServerError> {
        {
            let mut map = self
                .entries
                .write()
                .map_err(|_| SttServerError::Crypto("stt server lock poisoned".into()))?;
            let server = map
                .get_mut(id)
                .ok_or_else(|| SttServerError::NotFound(id.to_string()))?;
            mutate(server);
        }
        self.save()
    }

    fn encrypt(&self, plaintext: &str) -> Result<String, SttServerError> {
        match &self.persist {
            Some((_, cipher)) => cipher
                .encrypt_str(plaintext)
                .map_err(|e| SttServerError::Crypto(e.to_string())),
            // Kalıcılık yoksa (dev/test) da aynı at-rest ilkesini korumak için
            // süreç ömürlü sabit bir anahtar kullanılır — RELAY/SECRETS
            // store'larının `in_memory()` varyantlarıyla aynı taviz.
            None => in_memory_cipher()
                .encrypt_str(plaintext)
                .map_err(|e| SttServerError::Crypto(e.to_string())),
        }
    }

    fn decrypt(&self, encoded: &str) -> Result<String, SttServerError> {
        match &self.persist {
            Some((_, cipher)) => cipher
                .decrypt_str(encoded)
                .map_err(|e| SttServerError::Crypto(e.to_string())),
            None => in_memory_cipher()
                .decrypt_str(encoded)
                .map_err(|e| SttServerError::Crypto(e.to_string())),
        }
    }

    fn save(&self) -> Result<(), SttServerError> {
        let Some((path, cipher)) = &self.persist else {
            return Ok(());
        };
        let snapshot: HashMap<String, SttServer> = self
            .entries
            .read()
            .map_err(|_| SttServerError::Crypto("stt server lock poisoned".into()))?
            .clone();
        let json = serde_json::to_string(&SerializedEntries(&snapshot))?;
        let sealed = cipher
            .encrypt_str(&json)
            .map_err(|e| SttServerError::Crypto(e.to_string()))?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, sealed)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

/// `save()`/`load_entries()` düz alanları (private key/token dahil) taşıyan
/// dahili tam-veri gösterimi — `SttServer`'ın `Serialize`'ı panel yanıtları
/// için şifreli alanları atlar, ama diskteki tek blob zaten AES-256-GCM ile
/// şifreli olduğundan burada tam veri saklanmalı (aksi halde restart'ta
/// private key/token kaybolur). Bu yüzden dosyaya yazarken/okurken ayrı,
/// hiçbir alanı atlamayan bir görünüm kullanılır.
struct SerializedEntries<'a>(&'a HashMap<String, SttServer>);

impl<'a> Serialize for SerializedEntries<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (k, v) in self.0.iter() {
            map.serialize_entry(k, &FullSttServer(v))?;
        }
        map.end()
    }
}

/// `SttServer`'ın hiçbir alanı atlamayan disk-kalıcılığı görünümü.
struct FullSttServer<'a>(&'a SttServer);

impl<'a> Serialize for FullSttServer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct Full<'b> {
            id: &'b str,
            name: &'b str,
            hostname: &'b str,
            ssh_port: u16,
            ssh_user: &'b str,
            ssh_public_key: &'b str,
            ssh_private_key_encrypted: &'b str,
            host_fingerprint: &'b Option<String>,
            public_endpoint_override: &'b Option<String>,
            status: SttServerStatus,
            status_detail: &'b Option<String>,
            endpoint_url: &'b Option<String>,
            api_token_encrypted: &'b Option<String>,
            gpu_name: &'b Option<String>,
            vram_mb: Option<u64>,
            last_health_check_at: Option<i64>,
            created_at: i64,
        }
        let s = self.0;
        Full {
            id: &s.id,
            name: &s.name,
            hostname: &s.hostname,
            ssh_port: s.ssh_port,
            ssh_user: &s.ssh_user,
            ssh_public_key: &s.ssh_public_key,
            ssh_private_key_encrypted: &s.ssh_private_key_encrypted,
            host_fingerprint: &s.host_fingerprint,
            public_endpoint_override: &s.public_endpoint_override,
            status: s.status,
            status_detail: &s.status_detail,
            endpoint_url: &s.endpoint_url,
            api_token_encrypted: &s.api_token_encrypted,
            gpu_name: &s.gpu_name,
            vram_mb: s.vram_mb,
            last_health_check_at: s.last_health_check_at,
            created_at: s.created_at,
        }
        .serialize(serializer)
    }
}

fn load_entries(path: &FsPath, cipher: &Cipher) -> Result<HashMap<String, SttServer>, SttServerError> {
    let sealed = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(e) => return Err(SttServerError::Io(e)),
    };
    let json = cipher
        .decrypt_str(sealed.trim())
        .map_err(|e| SttServerError::Crypto(format!("stt server file unreadable: {e}")))?;
    // `SttServer::deserialize` şifreli alanları da (skip_serializing sadece
    // yazmayı etkiler, okumayı değil) doğru şekilde okur.
    Ok(serde_json::from_str(&json)?)
}

/// `persist` yokken (dev/test) kullanılan, süreç ömürlü sabit anahtarlı cipher.
/// Kalıcılık zaten yok — amaç bellek içi tutulan değerlerin de kazayla düz
/// metin olarak log/panic çıktısına karışmasını aynı ilkelle engellemek.
fn in_memory_cipher() -> Cipher {
    let key = crate::crypto::derive_key(b"stt-servers-in-memory", "streamdubber.stt_servers.v1");
    Cipher::new(&key).expect("32 byte key")
}

// --- Admin HTTP handlers ------------------------------------------------
//
// `POST/GET /admin/stt-servers`, `GET/DELETE /admin/stt-servers/{id}`,
// `POST /admin/stt-servers/{id}/provision` — `put_secret`/`delete_secret`
// (app.rs) ile aynı desen: `AdminUser` extractor, `Logger` audit kaydı,
// gizli alanlar (`SttServer`'ın `Serialize`'ı zaten bunları atlar) hiçbir
// yanıtta dönmez.

fn stt_server_status_code(err: &SttServerError) -> StatusCode {
    match err {
        SttServerError::NotFound(_) => StatusCode::NOT_FOUND,
        SttServerError::Invalid(_) => StatusCode::UNPROCESSABLE_ENTITY,
        SttServerError::Io(_) | SttServerError::Crypto(_) | SttServerError::Serde(_) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

fn default_ssh_port() -> u16 {
    22
}

fn default_ssh_user() -> String {
    "root".to_string()
}

/// `POST /admin/stt-servers` gövdesi.
#[derive(Debug, Deserialize)]
pub struct CreateServerRequest {
    pub name: String,
    pub hostname: String,
    #[serde(default = "default_ssh_port")]
    pub ssh_port: u16,
    #[serde(default = "default_ssh_user")]
    pub ssh_user: String,
    /// NAT'lı port eşleme kullanan sağlayıcılar için (ör. Vast.ai) dışarıdan
    /// erişilebilir gerçek uç nokta — boşsa `http://{hostname}:{port}` kullanılır.
    #[serde(default)]
    pub public_endpoint_override: Option<String>,
}

/// Yeni bir self-hosted STT sunucu kaydı ekler: SSH keypair üretir, `Pending`
/// durumunda döner. Admin, yanıttaki `ssh_public_key`'i sunucusuna ekleyip
/// ardından `POST .../{id}/provision` ile kurulumu başlatır.
pub async fn create_server(
    admin: AdminUser,
    State(store): State<Arc<SttServerStore>>,
    State(logger): State<Arc<Logger>>,
    Json(req): Json<CreateServerRequest>,
) -> Result<Json<SttServer>, (StatusCode, String)> {
    let server = store
        .add(
            &req.name,
            &req.hostname,
            req.ssh_port,
            &req.ssh_user,
            req.public_endpoint_override,
        )
        .map_err(|err| (stt_server_status_code(&err), err.to_string()))?;
    logger.log(
        LogLevel::Info,
        "stt_servers",
        &format!("stt server added: {} ({})", server.name, server.hostname),
        Some(&admin.claims.sub),
        None,
    );
    Ok(Json(server))
}

/// Tüm self-hosted STT sunucu kayıtlarını listeler (sadece admin).
pub async fn list_servers(
    _admin: AdminUser,
    State(store): State<Arc<SttServerStore>>,
) -> Json<Vec<SttServer>> {
    Json(store.list())
}

/// Tek bir sunucu kaydının güncel durumunu döner (panel polling için).
pub async fn get_server(
    _admin: AdminUser,
    Path(id): Path<String>,
    State(store): State<Arc<SttServerStore>>,
) -> Result<Json<SttServer>, (StatusCode, String)> {
    store
        .get(&id)
        .map(Json)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("stt server not found: {id}")))
}

/// Bir sunucu kaydını siler.
pub async fn delete_server(
    admin: AdminUser,
    Path(id): Path<String>,
    State(store): State<Arc<SttServerStore>>,
    State(logger): State<Arc<Logger>>,
) -> Result<StatusCode, (StatusCode, String)> {
    store
        .remove(&id)
        .map_err(|err| (stt_server_status_code(&err), err.to_string()))?;
    logger.log(
        LogLevel::Info,
        "stt_servers",
        &format!("stt server removed: {id}"),
        Some(&admin.claims.sub),
        None,
    );
    Ok(StatusCode::NO_CONTENT)
}

/// Kurulumu tetikler: durumu `Provisioning`'e çevirir, `Provisioner`'ı arka
/// planda çalıştırır (`tokio::spawn`) ve hemen `202 Accepted` döner — panel
/// `GET /admin/stt-servers/{id}` ile durumu pollar. `AppState.stt_provisioner`
/// prod'da `SshProvisioner`, testlerde `FakeProvisioner` olabilir.
pub async fn provision_server(
    admin: AdminUser,
    Path(id): Path<String>,
    State(store): State<Arc<SttServerStore>>,
    State(provisioner): State<Arc<dyn Provisioner>>,
    State(logger): State<Arc<Logger>>,
) -> Result<StatusCode, (StatusCode, String)> {
    let server = store
        .get(&id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("stt server not found: {id}")))?;
    let private_key = store
        .decrypt_private_key(&server)
        .map_err(|err| (stt_server_status_code(&err), err.to_string()))?;
    store
        .set_status(&id, SttServerStatus::Provisioning, None)
        .map_err(|err| (stt_server_status_code(&err), err.to_string()))?;
    logger.log(
        LogLevel::Info,
        "stt_servers",
        &format!("stt server provisioning started: {} ({})", server.name, server.hostname),
        Some(&admin.claims.sub),
        None,
    );

    tokio::spawn(async move {
        match provisioner.provision(&server, &private_key).await {
            Ok(outcome) => {
                if let Some(fingerprint) = &outcome.host_fingerprint {
                    if let Err(err) = store.pin_fingerprint(&server.id, fingerprint) {
                        tracing::warn!(server_id = %server.id, %err, "failed to persist host fingerprint");
                    }
                }
                if let Err(err) = store.mark_ready(
                    &server.id,
                    &outcome.endpoint_url,
                    &outcome.api_token,
                    outcome.gpu_name,
                    outcome.vram_mb,
                ) {
                    tracing::warn!(server_id = %server.id, %err, "failed to persist provisioning success");
                }
            }
            Err(err) => {
                let _ = store.set_status(&server.id, SttServerStatus::Error, Some(err.to_string()));
            }
        }
    });

    Ok(StatusCode::ACCEPTED)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_generates_keypair_and_starts_pending() {
        let store = SttServerStore::in_memory();
        let server = store.add("gpu-1", "1.2.3.4", 22, "root", None).expect("add");
        assert_eq!(server.status, SttServerStatus::Pending);
        assert!(server.ssh_public_key.starts_with("ssh-ed25519 "));
        assert!(!server.ssh_private_key_encrypted.is_empty());
        assert_ne!(server.ssh_private_key_encrypted, server.ssh_public_key);
    }

    #[test]
    fn rejects_empty_fields() {
        let store = SttServerStore::in_memory();
        assert!(matches!(
            store.add("", "1.2.3.4", 22, "root", None),
            Err(SttServerError::Invalid(_))
        ));
        assert!(matches!(
            store.add("gpu-1", "", 22, "root", None),
            Err(SttServerError::Invalid(_))
        ));
    }

    #[test]
    fn public_endpoint_override_is_stored_and_visible_in_json() {
        // Vast.ai gibi NAT'lı port eşleme kullanan sağlayıcılar için — bu
        // alan gizli değil (private key/token'ın aksine), panelde görünür.
        let store = SttServerStore::in_memory();
        let server = store
            .add(
                "gpu-1",
                "1.2.3.4",
                22,
                "root",
                Some("https://142.214.185.187:41234".into()),
            )
            .expect("add");
        assert_eq!(
            server.public_endpoint_override.as_deref(),
            Some("https://142.214.185.187:41234")
        );

        let json = serde_json::to_string(&server).expect("serialize");
        assert!(json.contains("142.214.185.187:41234"));
    }

    #[test]
    fn public_endpoint_override_blank_is_treated_as_none() {
        let store = SttServerStore::in_memory();
        let server = store
            .add("gpu-1", "1.2.3.4", 22, "root", Some("   ".into()))
            .expect("add");
        assert_eq!(server.public_endpoint_override, None);
    }

    #[test]
    fn private_key_never_appears_in_json_serialization() {
        let store = SttServerStore::in_memory();
        let server = store.add("gpu-1", "1.2.3.4", 22, "root", None).expect("add");
        store
            .mark_ready(&server.id, "https://gpu-1:8443", "super-secret-token", None, None)
            .expect("mark_ready");
        let fetched = store.get(&server.id).expect("get");

        let json = serde_json::to_string(&fetched).expect("serialize");
        assert!(!json.contains("PRIVATE KEY"));
        assert!(!json.contains("super-secret-token"));
        // Public alanlar hâlâ görünür.
        assert!(json.contains("ssh-ed25519"));
        assert!(json.contains("https://gpu-1:8443"));
    }

    #[test]
    fn list_and_remove() {
        let store = SttServerStore::in_memory();
        let a = store.add("a", "1.1.1.1", 22, "root", None).unwrap();
        let b = store.add("b", "2.2.2.2", 22, "root", None).unwrap();
        assert_eq!(store.list().len(), 2);

        store.remove(&a.id).unwrap();
        let remaining = store.list();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, b.id);

        assert!(matches!(store.remove(&a.id), Err(SttServerError::NotFound(_))));
    }

    #[test]
    fn set_status_updates_detail() {
        let store = SttServerStore::in_memory();
        let server = store.add("a", "1.1.1.1", 22, "root", None).unwrap();
        store
            .set_status(&server.id, SttServerStatus::Provisioning, None)
            .unwrap();
        assert_eq!(store.get(&server.id).unwrap().status, SttServerStatus::Provisioning);

        store
            .set_status(&server.id, SttServerStatus::Error, Some("ssh timeout".into()))
            .unwrap();
        let fetched = store.get(&server.id).unwrap();
        assert_eq!(fetched.status, SttServerStatus::Error);
        assert_eq!(fetched.status_detail.as_deref(), Some("ssh timeout"));
    }

    #[test]
    fn active_ready_returns_most_recent_and_decrypts_token() {
        let store = SttServerStore::in_memory();
        let a = store.add("a", "1.1.1.1", 22, "root", None).unwrap();
        let b = store.add("b", "2.2.2.2", 22, "root", None).unwrap();

        // Henüz hiç Ready yok.
        assert!(store.active_ready().is_none());

        store
            .mark_ready(&a.id, "https://a:8443", "token-a", Some("RTX 3070".into()), Some(8192))
            .unwrap();
        let (active, token) = store.active_ready().expect("a is ready");
        assert_eq!(active.id, a.id);
        assert_eq!(token, "token-a");

        // b daha sonra Ready olursa aktif olan o olmalı.
        std::thread::sleep(std::time::Duration::from_secs(1));
        store
            .mark_ready(&b.id, "https://b:8443", "token-b", None, None)
            .unwrap();
        let (active, token) = store.active_ready().expect("b is ready");
        assert_eq!(active.id, b.id);
        assert_eq!(token, "token-b");
    }

    #[test]
    fn pin_fingerprint_persists() {
        let store = SttServerStore::in_memory();
        let server = store.add("a", "1.1.1.1", 22, "root", None).unwrap();
        store.pin_fingerprint(&server.id, "SHA256:abc123").unwrap();
        assert_eq!(
            store.get(&server.id).unwrap().host_fingerprint.as_deref(),
            Some("SHA256:abc123")
        );
    }

    #[test]
    fn persists_encrypted_and_reloads_without_plaintext_private_key() {
        let path = std::env::temp_dir().join(format!("sd-stt-servers-{}.enc", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let key = crate::crypto::derive_key(b"jwt", "streamdubber.stt_servers.v1");

        let created_id = {
            let store = SttServerStore::with_file(&path, &key).unwrap();
            let server = store.add("gpu-1", "1.2.3.4", 22, "root", None).unwrap();
            server.id
        };

        // Diskte düz metin private key yok.
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("PRIVATE KEY"));

        let reloaded = SttServerStore::with_file(&path, &key).unwrap();
        let server = reloaded.get(&created_id).expect("reloaded");
        assert!(!server.ssh_private_key_encrypted.is_empty());

        let wrong = crate::crypto::derive_key(b"other", "streamdubber.stt_servers.v1");
        assert!(SttServerStore::with_file(&path, &wrong).is_err());

        let _ = std::fs::remove_file(&path);
    }
}
