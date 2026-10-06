#!/usr/bin/env bash
set -euo pipefail

attempts=1
check_public=1

usage() {
  echo "Usage: $0 [--retry] [--origin-only]" >&2
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --retry)
      attempts=60
      ;;
    --origin-only)
      check_public=0
      ;;
    *)
      usage
      exit 2
      ;;
  esac
  shift
done

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=infra/staging-common.sh
source "$script_dir/staging-common.sh"
staging_require_env
staging_public_ipv4=$(sed -n 's/^STAGING_PUBLIC_IPV4=//p' "$STAGING_ENV_FILE")
release_root=$(cd "$script_dir/.." && pwd)

tmp_dir=$(mktemp -d)
trap 'rm -rf "$tmp_dir"' EXIT

retry() {
  local count
  for count in $(seq 1 "$attempts"); do
    if "$@" >/dev/null 2>&1; then
      return 0
    fi
    if [ "$count" -eq "$attempts" ]; then
      break
    fi
    sleep 5
  done
  "$@"
}

origin_get() {
  local host=$1
  local path=$2
  curl --fail --silent --show-error --location \
    --connect-timeout 5 --max-time 20 \
    --resolve "$host:443:$staging_public_ipv4" "https://$host$path" >/dev/null
}

public_get() {
  local host=$1
  local path=$2
  curl --fail --silent --show-error --location \
    --connect-timeout 5 --max-time 20 "https://$host$path" >/dev/null
}

check_staging_headers() {
  local host=$1
  local headers="$tmp_dir/${host}.headers"
  curl --fail --silent --show-error --output /dev/null --dump-header "$headers" \
    --connect-timeout 5 --max-time 20 \
    --resolve "$host:443:$staging_public_ipv4" "https://$host/health" || \
    curl --fail --silent --show-error --output /dev/null --dump-header "$headers" \
      --connect-timeout 5 --max-time 20 \
      --resolve "$host:443:$staging_public_ipv4" "https://$host/"
  grep -Eiq '^strict-transport-security:.*max-age=' "$headers"
  grep -Eiq '^x-robots-tag:[[:space:]]*noindex, nofollow' "$headers"
}

# dev.strovak.com answers every unauthenticated request with a Basic Auth
# challenge, and this script holds only the bcrypt hash. The 401 challenge is
# therefore the healthy answer: it proves the edge, its certificate, and the
# gate. Any other status fails; a 2xx would mean the gate is off. Extra
# arguments are passed to curl (the origin check pins the address).
check_frontend_gate() {
  local headers="$tmp_dir/frontend-gate.headers"
  local code
  code=$(curl --silent --show-error --output /dev/null --dump-header "$headers" \
    --write-out '%{http_code}' --connect-timeout 5 --max-time 20 "$@" \
    'https://dev.strovak.com/') &&
    [ "$code" = 401 ] &&
    grep -Eiq '^www-authenticate:[[:space:]]*basic' "$headers"
}

check_frontend_gate_headers() {
  local headers="$tmp_dir/frontend-gate.origin.headers"
  curl --silent --show-error --output /dev/null --dump-header "$headers" \
    --connect-timeout 5 --max-time 20 \
    --resolve "dev.strovak.com:443:$staging_public_ipv4" 'https://dev.strovak.com/' &&
    grep -Eiq '^strict-transport-security:.*max-age=' "$headers" &&
    grep -Eiq '^x-robots-tag:[[:space:]]*noindex, nofollow' "$headers"
}

# The gate hides the frontend from this script, so reach it from inside the
# Caddy container: the same network path and upstream that reverse_proxy uses.
check_frontend_upstream() {
  local caddy
  caddy=$(staging_compose "$release_root" ps -q caddy) &&
    [ -n "$caddy" ] &&
    docker exec "$caddy" wget -q -T 10 -O /dev/null http://frontend:3000/
}

check_http_redirect() {
  local host=$1
  local headers="$tmp_dir/${host}.http.headers"
  curl --silent --show-error --output /dev/null --dump-header "$headers" \
    --connect-timeout 5 --max-time 10 \
    --resolve "$host:80:$staging_public_ipv4" "http://$host/"
  grep -Eq '^HTTP/[0-9.]+ (301|308)' "$headers"
  grep -Eiq "^location:[[:space:]]*https://${host}(/|\\r?$)" "$headers"
}

check_websocket_upgrade() {
  local headers="$tmp_dir/websocket.headers"
  curl --http1.1 --silent --show-error --output /dev/null --dump-header "$headers" \
    --connect-timeout 5 --max-time 2 \
    --resolve "dev.api.strovak.com:443:$staging_public_ipv4" \
    -H 'Connection: Upgrade' \
    -H 'Upgrade: websocket' \
    -H 'Sec-WebSocket-Version: 13' \
    -H 'Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==' \
    'https://dev.api.strovak.com/ws/stream' || true
  grep -Eq '^HTTP/1\.1 101' "$headers"
}

check_dashboard_cors() {
  local headers="$tmp_dir/cors.headers"
  curl --silent --show-error --output /dev/null --dump-header "$headers" \
    --request OPTIONS --connect-timeout 5 --max-time 10 \
    --resolve "dev.api.strovak.com:443:$staging_public_ipv4" \
    -H 'Origin: https://dev.strovak.com' \
    -H 'Access-Control-Request-Method: GET' \
    'https://dev.api.strovak.com/api/auth/session'
  grep -Eiq '^access-control-allow-origin:[[:space:]]*https://dev\.strovak\.com' "$headers"
  grep -Eiq '^access-control-allow-credentials:[[:space:]]*true' "$headers"
}

check_extension_surface_restricted() {
  local code
  code=$(curl --silent --show-error --output /dev/null --write-out '%{http_code}' \
    --connect-timeout 5 --max-time 10 \
    --resolve "ext-staging.strovak.com:443:$staging_public_ipv4" \
    'https://ext-staging.strovak.com/api/plans')
  [ "$code" = 404 ]
}

for host_path in \
  'dev.api.strovak.com /health' \
  'ext-staging.strovak.com /health'; do
  read -r host path <<<"$host_path"
  retry origin_get "$host" "$path"
done
retry check_frontend_gate --resolve "dev.strovak.com:443:$staging_public_ipv4"
retry check_frontend_upstream

retry check_staging_headers dev.api.strovak.com
retry check_frontend_gate_headers
retry check_staging_headers ext-staging.strovak.com
retry check_http_redirect dev.strovak.com
retry check_http_redirect dev.api.strovak.com
retry check_http_redirect ext-staging.strovak.com
retry check_websocket_upgrade
retry check_dashboard_cors
retry check_extension_surface_restricted

if [ "$check_public" -eq 1 ]; then
  for host_path in \
    'dev.api.strovak.com /health' \
    'ext-staging.strovak.com /health'; do
    read -r host path <<<"$host_path"
    retry public_get "$host" "$path"
  done
  retry check_frontend_gate
fi

if [ "$check_public" -eq 1 ]; then
  echo "HTTPS, headers, redirects, CORS, frontend gate, restricted EBS, public routing, and WSS checks passed."
else
  echo "Origin HTTPS, headers, redirects, CORS, frontend gate, restricted EBS, and WSS checks passed; public DNS/TLS was intentionally not checked."
fi
