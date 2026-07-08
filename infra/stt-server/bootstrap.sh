#!/usr/bin/env bash
#
# infra/stt-server/bootstrap.sh — roadmap 2.6 self-hosted GPU STT kurulumu.
#
# SshProvisioner (backend/src/stt_servers/provision.rs, adım 6) bu script'i
# `ssh user@host 'bash -s' < bootstrap.sh` ile admin'in eklediği GPU'lu
# VPS/VDS'te (sağlayıcıdan bağımsız — Hetzner/OVH/Vast.ai/kendi donanımı,
# fark etmez) çalıştırır. Idempotent: tekrar çalıştırmak kurulu Docker/
# toolkit'i atlar, mevcut API token'ı korur, sadece servisi günceller.
#
# Varsayım: Ubuntu/Debian (apt tabanlı) — ilk sürüm kapsamı bilinçli olarak
# tek dağıtım ailesiyle sınırlı (Faz 1 dışı özellik sormadan implement etme).
#
# Girdi (env ile override edilebilir; SshProvisioner set eder):
#   STT_IMAGE — pull edilecek image (varsayılan: ghcr.io/streamdubber/streamdubber-stt-server:latest)
#   STT_PORT  — dinlenecek port (varsayılan: 8443)
#   STT_MODEL — faster-whisper model adı (varsayılan: large-v3-turbo)
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
log "ensuring base tools (curl, gnupg, python3)..."
apt-get update -qq
apt-get install -y -qq curl gnupg python3 ca-certificates

# 1. NVIDIA GPU şartı — yoksa net hatayla çık (roadmap 2.6 §2: ilk sürüm
# NVIDIA-only, bkz. docs/local-stt-plan.md §2'deki aynı karar).
if ! command -v nvidia-smi >/dev/null 2>&1; then
    log "ERROR: nvidia-smi not found — this host has no NVIDIA driver installed."
    exit 1
fi
if ! nvidia-smi >/dev/null 2>&1; then
    log "ERROR: nvidia-smi present but failed to run (driver/GPU misconfigured)."
    exit 1
fi

# 2. Docker (idempotent).
if ! command -v docker >/dev/null 2>&1; then
    log "installing Docker..."
    curl -fsSL https://get.docker.com | sh
fi

# 3. NVIDIA Container Toolkit (idempotent — Docker zaten nvidia runtime'ı
# görüyorsa atla).
if ! docker info 2>/dev/null | grep -qi nvidia; then
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
fi

# 4. Firewall — yalnızca STT portu açık kalsın (genel ufw kuralı, sağlayıcıya
# özel bir şey değil; asıl yetkilendirme sınırı yine de bearer token'dır).
if command -v ufw >/dev/null 2>&1; then
    ufw allow "${STT_PORT}/tcp" >/dev/null 2>&1 || true
fi

mkdir -p "$INSTALL_DIR"
cd "$INSTALL_DIR"

# 5. API token — ilk kurulumda üretilir; yeniden provision etmek mevcut
# token'ı korur (backend'in elindeki token geçersizleşmez, sadece servis
# yeniden başlar/güncellenir).
if [ -f .env ] && grep -q '^API_TOKEN=' .env; then
    API_TOKEN=$(grep '^API_TOKEN=' .env | cut -d= -f2-)
else
    API_TOKEN=$(head -c 32 /dev/urandom | base64 | tr -dc 'a-zA-Z0-9' | head -c 43)
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

# 6. /health hazır olana kadar poll (model yükleme + ilk CUDA warm-up dahil
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
    docker compose logs --tail=50 >&2 || true
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
