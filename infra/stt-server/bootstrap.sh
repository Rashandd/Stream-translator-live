#!/usr/bin/env bash
#
# infra/stt-server/bootstrap.sh — roadmap 2.6 self-hosted GPU STT kurulumu.
#
# SshProvisioner (backend/src/stt_servers/provision.rs, adım 6) bu script'i
# `ssh user@host 'bash -s' < (preamble + bootstrap.sh)` ile admin'in eklediği
# GPU'lu VPS/VDS'te (sağlayıcıdan bağımsız — Hetzner/OVH/Vast.ai/kendi
# donanımı, fark etmez) çalıştırır. Idempotent: tekrar çalıştırmak kurulu
# Docker/toolkit'i atlar, mevcut API token'ı korur, sadece servisi günceller.
#
# Vast.ai notu (2026-07-08): Vast.ai'nin varsayılan "instance" tipi kendisi
# zaten bir Docker container'ıdır (host üzerinde) — bu container'ın İÇİNDE
# ayrıca nested bir Docker daemon çalıştırmak (docker-in-docker) genelde
# mümkün değil (systemd yok, privileged mode garanti değil). Bu yüzden script
# önce Docker'ın gerçekten kullanılabilir olup olmadığını TESPİT eder;
# kullanılamıyorsa (tipik Vast.ai container instance) Docker'a hiç
# dokunmadan STT servisini doğrudan bir Python venv içinde çalıştırır ("bare
# mode" — bkz. adım 3b). Vast.ai'nin "Ubuntu VM" instance tipi (gerçek VM,
# systemd var) seçilirse normal Docker yolu (adım 2-3) sorunsuz çalışır.
#
# Varsayım: Ubuntu/Debian (apt tabanlı) — ilk sürüm kapsamı bilinçli olarak
# tek dağıtım ailesiyle sınırlı (Faz 1 dışı özellik sormadan implement etme).
#
# Girdi (env ile override edilebilir; SshProvisioner set eder):
#   STT_IMAGE — Docker modunda pull edilecek image (varsayılan: ghcr.io/streamdubber/streamdubber-stt-server:latest)
#   STT_PORT  — dinlenecek port (varsayılan: 8443) — Vast.ai'de bu, host içi
#               port'tur; dışarıdan NAT'lı farklı bir port'tan erişiliyorsa
#               admin panelde "public endpoint override" ile ayrıca belirtilir.
#   STT_MODEL — faster-whisper model adı (varsayılan: large-v3-turbo)
#
# Girdi (dosya — SshProvisioner bu script'ten ÖNCE $INSTALL_DIR'a yazar):
#   $INSTALL_DIR/main.py, $INSTALL_DIR/requirements.txt — bare mode'da
#   çalıştırılacak uygulama kaynağı (provision.rs'teki APP_INSTALL_DIR ile
#   birebir aynı literal olmalı — ikisi senkron tutulur).
#
# Çıktı: başarıyla biterse SON SATIRDA TEK bir JSON satırı (Provisioner bunu
# parse eder), başka hiçbir çıktı stdout'a karışmaz (log satırları stderr'e
# gider):
#   {"status":"ready","gpu_name":"...","vram_mb":8192,"api_token":"...","port":8443}
# Herhangi bir adım başarısız olursa script hata koduyla çıkar, JSON basmaz.

set -euo pipefail

STT_IMAGE="${STT_IMAGE:-ghcr.io/streamdubber/streamdubber-stt-server:latest}"
STT_PORT="${STT_PORT:-8443}"
STT_MODEL="${STT_MODEL:-large-v3-turbo}"
INSTALL_DIR="/opt/streamdubber-stt"

log() { echo "[bootstrap] $*" >&2; }

if [ "$(id -u)" -ne 0 ]; then
    log "ERROR: must run as root (admin sunucuyu root ssh_user ile ekler)."
    exit 1
fi

# 0. Temel araçlar (idempotent — apt zaten kuruluysa hızlıca no-op geçer).
# Vast.ai container instance'larında da apt/root normal çalışır (kısıtlı olan
# nested Docker/systemd, apt değil).
log "ensuring base tools (curl, gnupg, python3, python3-venv)..."
apt-get update -qq
apt-get install -y -qq curl gnupg python3 python3-venv python3-pip ca-certificates

# 1. NVIDIA GPU şartı — yoksa net hatayla çık (roadmap 2.6 §2: ilk sürüm
# NVIDIA-only, bkz. docs/local-stt-plan.md §2'deki aynı karar). Vast.ai
# instance'larında GPU passthrough host tarafından zaten kurulur, bu kontrol
# değişmeden geçerli.
if ! command -v nvidia-smi >/dev/null 2>&1; then
    log "ERROR: nvidia-smi not found — this host has no NVIDIA driver installed."
    exit 1
fi
if ! nvidia-smi >/dev/null 2>&1; then
    log "ERROR: nvidia-smi present but failed to run (driver/GPU misconfigured)."
    exit 1
fi

mkdir -p "$INSTALL_DIR"
cd "$INSTALL_DIR"

# 2. API token — ilk kurulumda üretilir; yeniden provision etmek mevcut
# token'ı korur (backend'in elindeki token geçersizleşmez, sadece servis
# yeniden başlar/güncellenir).
if [ -f .env ] && grep -q '^API_TOKEN=' .env; then
    API_TOKEN=$(grep '^API_TOKEN=' .env | cut -d= -f2-)
else
    API_TOKEN=$(head -c 32 /dev/urandom | base64 | tr -dc 'a-zA-Z0-9' | head -c 43)
fi

# 3. Docker gerçekten kullanılabilir mi? — sadece CLI'nin varlığı yetmez,
# daemon'a erişilebilmesi gerekir. `docker info` daemon'a bağlanamazsa/nested
# Docker izinli değilse (tipik Vast.ai container instance) hata döner.
DOCKER_USABLE=0
HAS_SYSTEMD=0
if [ -d /run/systemd/system ] && command -v systemctl >/dev/null 2>&1; then
    HAS_SYSTEMD=1
fi
if command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
    DOCKER_USABLE=1
elif [ "$HAS_SYSTEMD" -eq 1 ]; then
    # Docker henüz kurulu değil ama systemd var — gerçek bir VM (Hetzner/OVH/
    # kendi donanımı/Vast.ai "Ubuntu VM" tipi), Docker güvenle kurulabilir.
    DOCKER_USABLE=1
fi

if [ "$DOCKER_USABLE" -eq 1 ]; then
    log "Docker mode (real VM or already-usable Docker daemon detected)."

    if ! command -v docker >/dev/null 2>&1; then
        log "installing Docker..."
        curl -fsSL https://get.docker.com | sh
    fi

    # NVIDIA Container Toolkit (idempotent — Docker zaten nvidia runtime'ı
    # görüyorsa atla). systemd yoksa `systemctl restart docker` çalışmaz —
    # bu durumda (Docker usable ama systemd yok) toolkit kurulumu atlanır;
    # böyle bir kutuda GPU runtime'ı zaten şablon/host tarafından
    # yapılandırılmış olması beklenir (ör. bazı Vast.ai Docker şablonları).
    if ! docker info 2>/dev/null | grep -qi nvidia; then
        if [ "$HAS_SYSTEMD" -eq 1 ]; then
            log "installing NVIDIA Container Toolkit..."
            curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey \
                | gpg --dearmor -o /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg
            curl -s -L https://nvidia.github.io/libnvidia-container/stable/deb/nvidia-container-toolkit.list \
                | sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#g' \
                > /etc/apt/sources.list.d/nvidia-container-toolkit.list
            apt-get update -qq
            apt-get install -y -qq nvidia-container-toolkit
            nvidia-ctk runtime configure --runtime=docker
            systemctl restart docker
        else
            log "WARNING: docker has no nvidia runtime registered and systemd is unavailable to reconfigure it — assuming the host/template already wired GPU passthrough into Docker."
        fi
    fi

    # Firewall — yalnızca STT portu açık kalsın (genel ufw kuralı, sağlayıcıya
    # özel bir şey değil; asıl yetkilendirme sınırı yine de bearer token'dır).
    if command -v ufw >/dev/null 2>&1; then
        ufw allow "${STT_PORT}/tcp" >/dev/null 2>&1 || true
    fi

    cat > .env <<EOF
STT_MODEL=${STT_MODEL}
API_TOKEN=${API_TOKEN}
EOF

    cat > docker-compose.yml <<EOF
services:
  stt-server:
    image: ${STT_IMAGE}
    restart: unless-stopped
    env_file: .env
    ports:
      - "${STT_PORT}:8443"
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: all
              capabilities: [gpu]
    healthcheck:
      test: ["CMD", "python3", "-c", "import urllib.request,sys; sys.exit(0 if urllib.request.urlopen('http://127.0.0.1:8443/health', timeout=3).status == 200 else 1)"]
      interval: 30s
      timeout: 5s
      retries: 3
      start_period: 60s
EOF

    log "pulling ${STT_IMAGE}..."
    docker compose pull
    log "starting service..."
    docker compose up -d
else
    # 3b. Bare mode — Docker kullanılamıyor (tipik Vast.ai container
    # instance: nested Docker/systemd yok). STT servisini doğrudan bir
    # Python venv içinde çalıştırır, systemd'siz basit bir kendi-kendini-
    # yeniden-başlatan supervisor loop'uyla (nohup + while).
    log "Bare mode (no usable Docker/systemd — likely a Vast.ai container instance): running stt-server directly via Python venv, no nested Docker."

    if [ ! -f main.py ] || [ ! -f requirements.txt ]; then
        log "ERROR: main.py/requirements.txt not found in $INSTALL_DIR — SshProvisioner should have written them before running this script."
        exit 1
    fi

    if [ ! -d venv ]; then
        python3 -m venv venv
    fi
    venv/bin/pip install --quiet --upgrade pip
    venv/bin/pip install --quiet -r requirements.txt

    cat > .env <<EOF
STT_MODEL=${STT_MODEL}
API_TOKEN=${API_TOKEN}
EOF

    cat > run.sh <<RUNEOF
#!/usr/bin/env bash
cd "$INSTALL_DIR"
set -a
. ./.env
set +a
while true; do
    venv/bin/uvicorn main:app --host 0.0.0.0 --port ${STT_PORT} >> stt-server.log 2>&1
    echo "\$(date -u +%FT%TZ) stt-server exited, restarting in 2s" >> stt-server.log
    sleep 2
done
RUNEOF
    chmod +x run.sh

    # Idempotent restart: önceki süreç(ler)i durdur (best-effort — systemd
    # olmadan tam bir process-group garantisi yok, ama tekrar tekrar
    # provision etmek her seferinde eski uvicorn'u öldürüp yenisini başlatır).
    pkill -f "$INSTALL_DIR/venv/bin/uvicorn main:app" 2>/dev/null || true
    pkill -f "$INSTALL_DIR/run.sh" 2>/dev/null || true
    sleep 1

    nohup ./run.sh >/dev/null 2>&1 &
    disown
fi

# 4. /health hazır olana kadar poll (model yükleme + ilk CUDA warm-up dahil
# — bu yüzden geniş bir zaman aşımı: 60 x 5s = 5 dakika).
log "waiting for /health..."
HEALTH=""
for _ in $(seq 1 60); do
    if HEALTH=$(curl -fsS "http://127.0.0.1:${STT_PORT}/health" 2>/dev/null); then
        break
    fi
    HEALTH=""
    sleep 5
done

if [ -z "$HEALTH" ]; then
    log "ERROR: service did not become healthy in time"
    if [ "$DOCKER_USABLE" -eq 1 ]; then
        docker compose logs --tail=50 >&2 || true
    else
        tail -n 50 stt-server.log >&2 || true
    fi
    exit 1
fi

python3 - "$HEALTH" "$API_TOKEN" "$STT_PORT" <<'PYEOF'
import json
import sys

health_json, api_token, port = sys.argv[1:4]
health = json.loads(health_json)
print(json.dumps({
    "status": "ready",
    "gpu_name": health.get("gpu_name"),
    "vram_mb": health.get("vram_mb"),
    "api_token": api_token,
    "port": int(port),
}))
PYEOF
