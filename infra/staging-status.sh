#!/usr/bin/env bash
set -euo pipefail

if [ "$(id -u)" -ne 0 ]; then
  exec sudo -- "$(readlink -f "$0")" "$@"
fi

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=infra/staging-common.sh
source "$script_dir/staging-common.sh"
staging_require_env
release=$(staging_current_release)

staging_compose "$release" ps
for service in postgres redis backend frontend caddy; do
  container=$(staging_compose "$release" ps -q "$service")
  test -n "$container"
  state=$(docker inspect --format '{{.State.Status}}' "$container")
  health=$(docker inspect --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}missing{{end}}' "$container")
  if [ "$state" != running ] || [ "$health" != healthy ]; then
    echo "$service is not healthy (state=$state health=$health)." >&2
    exit 1
  fi
done

env_mode=$(stat -c '%a:%U' "$STAGING_ENV_FILE")
if [ "$env_mode" != '600:root' ]; then
  echo "$STAGING_ENV_FILE must be mode 600 and root-owned (got $env_mode)." >&2
  exit 1
fi
if [ "$(docker network inspect streamdubber_staging_internal --format '{{.Internal}}')" != true ]; then
  echo "The PostgreSQL/Redis network is not Docker-internal." >&2
  exit 1
fi

"$release/infra/staging-health.sh"

# No staging service except Caddy may publish a host port.
for service in postgres redis backend frontend; do
  container=$(staging_compose "$release" ps -q "$service")
  bindings=$(docker inspect --format '{{json .HostConfig.PortBindings}}' "$container")
  if [ "$bindings" != '{}' ] && [ "$bindings" != 'null' ]; then
    echo "$service unexpectedly publishes a host port." >&2
    exit 1
  fi
done

ss -H -lntup | awk '$5 ~ /:(3000|5432|6379|8080|8200)$/ {print}' || true
ufw status verbose
iptables -S STREAMDUBBER-STAGING
systemctl is-active --quiet streamdubber-docker-firewall.service
systemctl is-active --quiet rashand-api-bridge.socket
if systemctl is-enabled --quiet nginx || systemctl is-active --quiet nginx; then
  echo "Nginx is enabled/active and may race Caddy for 80/443." >&2
  exit 1
fi

caddy_container=$(staging_compose "$release" ps -q caddy)
if docker exec "$caddy_container" wget -q -T 2 -O /dev/null \
  http://host.docker.internal:18789/ 2>/dev/null; then
  echo "Caddy can unexpectedly reach the OpenClaw loopback gateway." >&2
  exit 1
fi

backend_container=$(staging_compose "$release" ps -q backend)
frontend_container=$(staging_compose "$release" ps -q frontend)
test "$(docker inspect --format '{{range .Config.Env}}{{if eq . "APP_ENV=production"}}production{{end}}{{end}}' "$backend_container")" = production
test "$(docker inspect --format '{{range .Config.Env}}{{if eq . "NODE_ENV=production"}}production{{end}}{{end}}' "$frontend_container")" = production

if ss -H -lntp | awk '$4 ~ /:18789$/ {print $4}' | grep -Evq '^(127\.0\.0\.1|\[::1\]|::1):18789$'; then
  echo "OpenClaw is no longer loopback-only." >&2
  exit 1
fi

echo "Staging service, edge, firewall, and isolation checks passed."
