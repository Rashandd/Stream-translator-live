//! SSH tabanlı kurulum soyutlaması (roadmap 2.6 adım 2/6).
//!
//! `SshProvisioner` admin'in eklediği herhangi bir GPU'lu VPS/VDS'e
//! (sağlayıcıdan bağımsız — Hetzner, OVH, Vast.ai, kendi donanımı, fark
//! etmez) SSH ile bağlanıp `infra/stt-server/bootstrap.sh`'i çalıştırır.
//! Yeni bir Rust SSH crate'i eklemek yerine (bu projenin `gpu_probe.rs`'de
//! `nvidia-smi`'yi shell-out ettiği tercihle tutarlı) sistemde zaten var olan
//! openssh-client paketini (`ssh`, `ssh-keyscan`, `ssh-keygen`) kullanır.
//!
//! `FakeProvisioner` gerçek SSH yapmadan test/iskelet implementasyonudur —
//! admin route'ları ve durum makinesi bunun üzerinden test edilir
//! (`SttProvider`/`SecretStore` trait'lerinde zaten kullanılan aynı desen).

use std::path::{Path, PathBuf};
use std::process::Stdio;

use async_trait::async_trait;
use serde::Deserialize;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use super::SttServer;

/// Bir kurulumun başarılı sonucu — `SttServerStore::mark_ready` +
/// (varsa) `pin_fingerprint`'e geçirilir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisionOutcome {
    pub endpoint_url: String,
    pub api_token: String,
    pub gpu_name: Option<String>,
    pub vram_mb: Option<u64>,
    /// İlk bağlantıda `ssh-keyscan` ile alınan host fingerprint (TOFU) —
    /// `None` sadece `FakeProvisioner`'da (gerçek SSH hiç yapılmadığında).
    pub host_fingerprint: Option<String>,
}

/// Kurulum hataları.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum ProvisionError {
    #[error("ssh connection failed: {0}")]
    Connection(String),
    #[error("host fingerprint mismatch — possible reprovisioned/MITM host: {0}")]
    FingerprintMismatch(String),
    #[error("bootstrap script failed: {0}")]
    Bootstrap(String),
    #[error("unexpected bootstrap output: {0}")]
    Malformed(String),
}

/// Bir sunucuyu SSH ile kurup hazır hale getiren soyutlama.
#[async_trait]
pub trait Provisioner: Send + Sync {
    /// `ssh_private_key`: `SttServerStore`'dan çözülmüş (düz metin) OpenSSH
    /// private key — trait bilinçli olarak store'a bağımlı değil, çağıran
    /// (admin route handler'ı) key'i çözüp geçirir.
    async fn provision(
        &self,
        server: &SttServer,
        ssh_private_key: &str,
    ) -> Result<ProvisionOutcome, ProvisionError>;
}

/// Test/iskelet implementasyonu: sabit bir sonuç döner, gerçek SSH yapmaz.
pub struct FakeProvisioner {
    result: Result<ProvisionOutcome, ProvisionError>,
}

impl FakeProvisioner {
    /// Verilen sonucu her çağrıda aynen döner.
    pub fn always(result: Result<ProvisionOutcome, ProvisionError>) -> Self {
        FakeProvisioner { result }
    }
}

#[async_trait]
impl Provisioner for FakeProvisioner {
    async fn provision(
        &self,
        _server: &SttServer,
        _ssh_private_key: &str,
    ) -> Result<ProvisionOutcome, ProvisionError> {
        self.result.clone()
    }
}

/// `bootstrap.sh`'in bare-mode'da (Docker kullanılamadığında — bkz. Vast.ai
/// notu, `infra/stt-server/bootstrap.sh`) uygulama kaynağını yazacağı yol.
/// Bootstrap script'teki `INSTALL_DIR` literal'iyle BİREBİR aynı kalmalı —
/// ikisi ayrı dosyalarda olduğu için elle senkronize tutulur.
const APP_INSTALL_DIR: &str = "/opt/streamdubber-stt";

/// Gerçek SSH implementasyonu — `ssh`/`ssh-keyscan`/`ssh-keygen`'e shell-out
/// eder. Akış: (1) private key'i 0600 izinli geçici dosyaya yaz, (2)
/// `ssh-keyscan` ile host'un güncel anahtarlarını al, (3) fingerprint'i
/// (sadece ilk satır — `gpu_probe.rs`'in "yalnız ilk GPU" sadeleştirmesiyle
/// aynı ilke) `server.host_fingerprint` ile karşılaştır (TOFU: yoksa kabul
/// edilir, varsa uyuşmalı), (4) `bootstrap.sh`'i (önüne `main.py`/
/// `requirements.txt`'i `$INSTALL_DIR`'a yazan bir preamble eklenmiş halde)
/// `ssh ... 'bash -s'` ile stdin'den çalıştırır — gerçek `ssh` istemcisi
/// ayrıca tüm keyscan satırlarını içeren geçici bir `known_hosts` dosyasına
/// karşı `StrictHostKeyChecking=yes` ile doğrular (bizim string
/// karşılaştırmamızla aynı ana bağlı değil — TOCTOU'ya karşı asıl
/// kriptografik doğrulama budur).
///
/// `main.py`/`requirements.txt` her zaman gönderilir (Docker modunda
/// kullanılmasa da diskte durmaları zararsızdır) — böylece Rust tarafı,
/// uzak makinenin Docker kullanıp kullanamayacağına (Vast.ai container
/// instance'larında genelde kullanamaz) bakmadan tek bir SSH çağrısıyla her
/// iki moda da hazır bir script gönderir; asıl mod seçimi bootstrap.sh
/// içinde, uzak makinede tespit edilir.
pub struct SshProvisioner {
    bootstrap_script: String,
    main_py: String,
    requirements_txt: String,
}

impl SshProvisioner {
    /// İçerikleri doğrudan verir (test için).
    pub fn new(
        bootstrap_script: impl Into<String>,
        main_py: impl Into<String>,
        requirements_txt: impl Into<String>,
    ) -> Self {
        SshProvisioner {
            bootstrap_script: bootstrap_script.into(),
            main_py: main_py.into(),
            requirements_txt: requirements_txt.into(),
        }
    }

    /// `infra/stt-server/bootstrap.sh` + `stt-server/main.py` +
    /// `stt-server/requirements.txt`'i dosyadan okuyarak kurar.
    pub fn from_paths(
        bootstrap_path: impl AsRef<Path>,
        main_py_path: impl AsRef<Path>,
        requirements_path: impl AsRef<Path>,
    ) -> std::io::Result<Self> {
        Ok(Self::new(
            std::fs::read_to_string(bootstrap_path)?,
            std::fs::read_to_string(main_py_path)?,
            std::fs::read_to_string(requirements_path)?,
        ))
    }
}

/// `main.py`/`requirements.txt`'i uzak makinede `$APP_INSTALL_DIR`'a yazan
/// shell preamble'ı üretir. Base64 kullanılır — rastgele Python kaynak
/// kodunun içinde heredoc delimiter'ıyla çakışma riski olmadan (kod, tırnak,
/// `$()' vb. içerebilir) ikili-güvenli aktarım sağlar.
fn build_write_app_files_preamble(main_py: &str, requirements_txt: &str) -> String {
    use base64::engine::general_purpose::STANDARD as B64;
    use base64::Engine;
    format!(
        "mkdir -p {dir}\n\
         echo '{main_b64}' | base64 -d > {dir}/main.py\n\
         echo '{req_b64}' | base64 -d > {dir}/requirements.txt\n",
        dir = APP_INSTALL_DIR,
        main_b64 = B64.encode(main_py.as_bytes()),
        req_b64 = B64.encode(requirements_txt.as_bytes()),
    )
}

#[async_trait]
impl Provisioner for SshProvisioner {
    async fn provision(
        &self,
        server: &SttServer,
        ssh_private_key: &str,
    ) -> Result<ProvisionOutcome, ProvisionError> {
        let workdir = TempWorkdir::new()?;
        let key_path = workdir.write_private_key(ssh_private_key)?;

        let keyscan_output = run_ssh_keyscan(&server.hostname, server.ssh_port).await?;
        let current_fingerprint = fingerprint_of_first_line(&keyscan_output).await?;
        if let Some(expected) = &server.host_fingerprint {
            if expected != &current_fingerprint {
                return Err(ProvisionError::FingerprintMismatch(format!(
                    "expected {expected}, got {current_fingerprint}"
                )));
            }
        }
        let known_hosts_path = workdir.write_known_hosts(&keyscan_output)?;

        let script = format!(
            "{}\n{}",
            build_write_app_files_preamble(&self.main_py, &self.requirements_txt),
            self.bootstrap_script
        );
        let output = run_bootstrap_over_ssh(server, &key_path, &known_hosts_path, &script).await?;

        if !output.status.success() {
            return Err(ProvisionError::Bootstrap(format!(
                "exit status {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut outcome = parse_bootstrap_result(&stdout, &server.hostname)?;
        outcome.host_fingerprint = Some(current_fingerprint);
        // Vast.ai gibi NAT'lı port eşleme kullanan sağlayıcılarda dışarıdan
        // erişilebilir port/host, servisin kendi bildirdiğinden farklı
        // olabilir — admin bunu sunucu eklerken ayrıca belirtmişse (bkz.
        // `SttServer::public_endpoint_override`) o değer esas alınır.
        if let Some(override_endpoint) = &server.public_endpoint_override {
            outcome.endpoint_url = override_endpoint.clone();
        }
        Ok(outcome)
    }
}

/// Geçici dosyaların (private key, known_hosts) yaşam döngüsünü yönetir —
/// `Drop` ile temizlenir, hata yollarında da diskte anahtar kalmaz.
struct TempWorkdir {
    dir: PathBuf,
}

impl TempWorkdir {
    fn new() -> Result<Self, ProvisionError> {
        let dir = std::env::temp_dir().join(format!(
            "sd-ssh-provision-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        std::fs::create_dir_all(&dir)
            .map_err(|e| ProvisionError::Connection(format!("temp workdir: {e}")))?;
        Ok(TempWorkdir { dir })
    }

    fn write_private_key(&self, key: &str) -> Result<PathBuf, ProvisionError> {
        let path = self.dir.join("id_ed25519");
        std::fs::write(&path, key)
            .map_err(|e| ProvisionError::Connection(format!("write private key: {e}")))?;
        set_owner_only_permissions(&path)
            .map_err(|e| ProvisionError::Connection(format!("chmod private key: {e}")))?;
        Ok(path)
    }

    fn write_known_hosts(&self, keyscan_output: &str) -> Result<PathBuf, ProvisionError> {
        let path = self.dir.join("known_hosts");
        std::fs::write(&path, keyscan_output)
            .map_err(|e| ProvisionError::Connection(format!("write known_hosts: {e}")))?;
        Ok(path)
    }
}

impl Drop for TempWorkdir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[cfg(unix)]
fn set_owner_only_permissions(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_owner_only_permissions(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

fn unique_suffix() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
        .to_string()
}

/// `ssh-keyscan -p <port> <host>` — host'un mevcut anahtarlarını (tüm
/// türler) ham `known_hosts` satırları olarak döner.
async fn run_ssh_keyscan(hostname: &str, port: u16) -> Result<String, ProvisionError> {
    let output = Command::new("ssh-keyscan")
        .arg("-p")
        .arg(port.to_string())
        .arg(hostname)
        .output()
        .await
        .map_err(|e| ProvisionError::Connection(format!("ssh-keyscan spawn failed: {e}")))?;

    if !output.status.success() || output.stdout.is_empty() {
        return Err(ProvisionError::Connection(format!(
            "ssh-keyscan returned no host keys for {hostname}:{port}"
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// `ssh-keyscan` çıktısının İLK satırından (yalnızca bir anahtar türü —
/// `gpu_probe.rs`'in "yalnız ilk GPU" sadeleştirmesiyle aynı ilke)
/// `ssh-keygen -lf -` ile SHA256 fingerprint çıkarır.
async fn fingerprint_of_first_line(keyscan_output: &str) -> Result<String, ProvisionError> {
    let first_line = keyscan_output
        .lines()
        .find(|l| !l.trim().is_empty())
        .ok_or_else(|| ProvisionError::Malformed("empty ssh-keyscan output".into()))?;

    let mut child = Command::new("ssh-keygen")
        .arg("-lf")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ProvisionError::Connection(format!("ssh-keygen spawn failed: {e}")))?;

    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| ProvisionError::Connection("ssh-keygen stdin unavailable".into()))?;
        stdin
            .write_all(first_line.as_bytes())
            .await
            .map_err(|e| ProvisionError::Connection(format!("ssh-keygen write failed: {e}")))?;
    }

    let output = child
        .wait_with_output()
        .await
        .map_err(|e| ProvisionError::Connection(format!("ssh-keygen wait failed: {e}")))?;
    if !output.status.success() {
        return Err(ProvisionError::Malformed(format!(
            "ssh-keygen -lf failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    text.split_whitespace()
        .nth(1)
        .map(str::to_string)
        .ok_or_else(|| ProvisionError::Malformed("could not parse ssh-keygen fingerprint".into()))
}

/// `bootstrap.sh`'i stdin üzerinden `ssh ... 'bash -s'` ile uzak makinede
/// çalıştırır. `StrictHostKeyChecking=yes` + pinlenmiş `known_hosts` —
/// host anahtarı uyuşmazsa `ssh` kendisi bağlantıyı reddeder (yukarıdaki
/// fingerprint karşılaştırmamız TOCTOU'ya karşı ek bir erken-uyarı, asıl
/// kriptografik doğrulama budur).
async fn run_bootstrap_over_ssh(
    server: &SttServer,
    key_path: &Path,
    known_hosts_path: &Path,
    bootstrap_script: &str,
) -> Result<std::process::Output, ProvisionError> {
    let mut child = Command::new("ssh")
        .arg("-i")
        .arg(key_path)
        .arg("-p")
        .arg(server.ssh_port.to_string())
        .arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg("ConnectTimeout=15")
        .arg("-o")
        .arg("StrictHostKeyChecking=yes")
        .arg("-o")
        .arg(format!("UserKnownHostsFile={}", known_hosts_path.display()))
        .arg(format!("{}@{}", server.ssh_user, server.hostname))
        .arg("bash -s")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ProvisionError::Connection(format!("ssh spawn failed: {e}")))?;

    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| ProvisionError::Connection("ssh stdin unavailable".into()))?;
        stdin
            .write_all(bootstrap_script.as_bytes())
            .await
            .map_err(|e| ProvisionError::Connection(format!("ssh write failed: {e}")))?;
    }

    child
        .wait_with_output()
        .await
        .map_err(|e| ProvisionError::Connection(format!("ssh wait failed: {e}")))
}

/// `bootstrap.sh`'in son (boş olmayan) stdout satırındaki JSON sonucu.
#[derive(Debug, Deserialize)]
struct BootstrapResult {
    status: String,
    gpu_name: Option<String>,
    vram_mb: Option<u64>,
    api_token: String,
    port: u16,
}

fn parse_bootstrap_result(stdout: &str, hostname: &str) -> Result<ProvisionOutcome, ProvisionError> {
    let last_line = stdout
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .ok_or_else(|| ProvisionError::Malformed("bootstrap produced no output".into()))?;

    let result: BootstrapResult = serde_json::from_str(last_line)
        .map_err(|e| ProvisionError::Malformed(format!("could not parse bootstrap output: {e}")))?;

    if result.status != "ready" {
        return Err(ProvisionError::Bootstrap(format!(
            "unexpected bootstrap status: {}",
            result.status
        )));
    }

    Ok(ProvisionOutcome {
        endpoint_url: format!("http://{hostname}:{}", result.port),
        api_token: result.api_token,
        gpu_name: result.gpu_name,
        vram_mb: result.vram_mb,
        host_fingerprint: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server() -> SttServer {
        super::super::SttServerStore::in_memory()
            .add("gpu-1", "1.2.3.4", 22, "root", None)
            .expect("add")
    }

    // --- build_write_app_files_preamble (bare-mode/Vast.ai desteği) ---

    #[test]
    fn write_app_files_preamble_roundtrips_via_base64() {
        use base64::engine::general_purpose::STANDARD as B64;
        use base64::Engine;

        let main_py = "print('hello')\nimport os\n# quotes ' and \" and $(cmd) are fine\n";
        let requirements = "fastapi==0.115.6\n";
        let preamble = build_write_app_files_preamble(main_py, requirements);

        assert!(preamble.contains(&format!("mkdir -p {APP_INSTALL_DIR}")));
        assert!(preamble.contains(&format!("> {APP_INSTALL_DIR}/main.py")));
        assert!(preamble.contains(&format!("> {APP_INSTALL_DIR}/requirements.txt")));

        // Base64 blokları düz metni bozmadan taşımalı (tırnak/$()/newline dahil).
        let main_b64 = B64.encode(main_py.as_bytes());
        let req_b64 = B64.encode(requirements.as_bytes());
        assert!(preamble.contains(&main_b64));
        assert!(preamble.contains(&req_b64));
        assert_eq!(
            String::from_utf8(B64.decode(main_b64).unwrap()).unwrap(),
            main_py
        );
    }

    fn outcome() -> ProvisionOutcome {
        ProvisionOutcome {
            endpoint_url: "http://gpu-1:8443".into(),
            api_token: "tok".into(),
            gpu_name: Some("RTX 3070".into()),
            vram_mb: Some(8192),
            host_fingerprint: None,
        }
    }

    #[tokio::test]
    async fn fake_provisioner_returns_configured_success() {
        let provisioner = FakeProvisioner::always(Ok(outcome()));
        assert_eq!(provisioner.provision(&server(), "unused-key").await, Ok(outcome()));
    }

    #[tokio::test]
    async fn fake_provisioner_returns_configured_error() {
        let provisioner = FakeProvisioner::always(Err(ProvisionError::Connection("timeout".into())));
        assert!(matches!(
            provisioner.provision(&server(), "unused-key").await,
            Err(ProvisionError::Connection(_))
        ));
    }

    // --- parse_bootstrap_result ---

    #[test]
    fn parses_ready_result_from_last_line() {
        let stdout = "[bootstrap] some log noise\n[bootstrap] more noise\n\
            {\"status\":\"ready\",\"gpu_name\":\"RTX 3070\",\"vram_mb\":8192,\"api_token\":\"tok-123\",\"port\":8443}\n";
        let outcome = parse_bootstrap_result(stdout, "1.2.3.4").expect("parsed");
        assert_eq!(outcome.endpoint_url, "http://1.2.3.4:8443");
        assert_eq!(outcome.api_token, "tok-123");
        assert_eq!(outcome.gpu_name.as_deref(), Some("RTX 3070"));
        assert_eq!(outcome.vram_mb, Some(8192));
        assert_eq!(outcome.host_fingerprint, None);
    }

    #[test]
    fn parses_result_with_null_gpu_fields() {
        let stdout = r#"{"status":"ready","gpu_name":null,"vram_mb":null,"api_token":"tok","port":8443}"#;
        let outcome = parse_bootstrap_result(stdout, "host").expect("parsed");
        assert_eq!(outcome.gpu_name, None);
        assert_eq!(outcome.vram_mb, None);
    }

    #[test]
    fn rejects_non_ready_status() {
        let stdout = r#"{"status":"error","gpu_name":null,"vram_mb":null,"api_token":"tok","port":8443}"#;
        assert!(matches!(
            parse_bootstrap_result(stdout, "host"),
            Err(ProvisionError::Bootstrap(_))
        ));
    }

    #[test]
    fn rejects_garbage_output() {
        assert!(matches!(
            parse_bootstrap_result("not json at all", "host"),
            Err(ProvisionError::Malformed(_))
        ));
        assert!(matches!(
            parse_bootstrap_result("", "host"),
            Err(ProvisionError::Malformed(_))
        ));
    }

    // --- fingerprint extraction (gerçek ssh-keygen binary'sine karşı,
    // ağ/gerçek SSH gerektirmez — sabit bir known_hosts satırı besleriz) ---

    #[tokio::test]
    async fn fingerprint_of_first_line_uses_real_ssh_keygen() {
        // Gerçek bir ed25519 host key satırı (rastgele üretilmiş, gerçek bir
        // sunucuya ait değil) — format `host key-type base64key`.
        let (_priv, pubkey) = generate_test_keypair();
        let known_hosts_line = format!("test-host {pubkey}");

        let fp = fingerprint_of_first_line(&known_hosts_line)
            .await
            .expect("fingerprint");
        assert!(fp.starts_with("SHA256:"), "fp: {fp}");

        // Aynı satır her zaman aynı fingerprint'i vermeli (deterministik).
        let fp2 = fingerprint_of_first_line(&known_hosts_line).await.expect("fingerprint 2");
        assert_eq!(fp, fp2);
    }

    #[tokio::test]
    async fn fingerprint_of_first_line_rejects_garbage() {
        assert!(fingerprint_of_first_line("").await.is_err());
        assert!(fingerprint_of_first_line("not a valid known_hosts line").await.is_err());
    }

    /// `ssh-keygen -t ed25519`'e shell-out ederek test için gerçek bir
    /// (host key olarak kullanılmayan, sadece bu testte tek seferlik) keypair
    /// üretir — `SttServerStore::add`'ın kullandığı aynı yardımcı mantık.
    fn generate_test_keypair() -> (String, String) {
        let dir = std::env::temp_dir().join(format!("sd-test-hostkey-{}", unique_suffix()));
        std::fs::create_dir_all(&dir).unwrap();
        let priv_path = dir.join("id_ed25519");
        let status = std::process::Command::new("ssh-keygen")
            .args(["-t", "ed25519", "-N", "", "-C", "test", "-f"])
            .arg(&priv_path)
            .status()
            .expect("run ssh-keygen");
        assert!(status.success());
        let private = std::fs::read_to_string(&priv_path).unwrap();
        let public = std::fs::read_to_string(priv_path.with_extension("pub")).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        // Public key satırı "ssh-ed25519 AAAA... comment" — known_hosts
        // formatı için "host ssh-ed25519 AAAA..." istiyoruz, comment'i atarız.
        let pubkey_no_comment = public
            .split_whitespace()
            .take(2)
            .collect::<Vec<_>>()
            .join(" ");
        (private, pubkey_no_comment)
    }
}
