#!/usr/bin/env bash
set -euo pipefail

attempts=1
if [ "${1:-}" = "--retry" ]; then
  attempts=60
fi

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
    --resolve "$host:443:127.0.0.1" "https://$host$path" >/dev/null
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
    --resolve "$host:443:127.0.0.1" "https://$host/health" || \
    curl --fail --silent --show-error --output /dev/null --dump-header "$headers" \
      --connect-timeout 5 --max-time 20 \
      --resolve "$host:443:127.0.0.1" "https://$host/"
  grep -Eiq '^strict-transport-security:.*max-age=' "$headers"
  grep -Eiq '^x-robots-tag:[[:space:]]*noindex, nofollow' "$headers"
}

check_http_redirect() {
  local host=$1
  local headers="$tmp_dir/${host}.http.headers"
  curl --silent --show-error --output /dev/null --dump-header "$headers" \
    --connect-timeout 5 --max-time 10 \
    --resolve "$host:80:127.0.0.1" "http://$host/"
  grep -Eq '^HTTP/[0-9.]+ (301|308)' "$headers"
  grep -Eiq "^location:[[:space:]]*https://${host}(/|\\r?$)" "$headers"
}

check_websocket_upgrade() {
  local headers="$tmp_dir/websocket.headers"
  curl --http1.1 --silent --show-error --output /dev/null --dump-header "$headers" \
    --connect-timeout 5 --max-time 2 \
    --resolve 'dev.api.strovak.com:443:127.0.0.1' \
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
    --resolve 'dev.api.strovak.com:443:127.0.0.1' \
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
    --resolve 'ext-staging.strovak.com:443:127.0.0.1' \
    'https://ext-staging.strovak.com/api/plans')
  [ "$code" = 404 ]
}

check_existing_api_bridge() {
  local code
  code=$(curl --silent --show-error --output /dev/null --write-out '%{http_code}' \
    --connect-timeout 5 --max-time 15 \
    --resolve 'rashand-tech.com:443:127.0.0.1' \
    'https://rashand-tech.com/api/health')
  [ "$code" -ge 200 ] && [ "$code" -lt 500 ]
}

for host_path in \
  'dev.api.strovak.com /health' \
  'dev.strovak.com /' \
  'ext-staging.strovak.com /health' \
  'rashand-tech.com /' \
  'shopping.rashand-tech.com /'; do
  read -r host path <<<"$host_path"
  retry origin_get "$host" "$path"
done

retry check_existing_api_bridge
retry check_staging_headers dev.api.strovak.com
retry check_staging_headers dev.strovak.com
retry check_staging_headers ext-staging.strovak.com
retry check_http_redirect dev.strovak.com
retry check_http_redirect dev.api.strovak.com
retry check_http_redirect ext-staging.strovak.com
retry check_websocket_upgrade
retry check_dashboard_cors
retry check_extension_surface_restricted

for host_path in \
  'dev.api.strovak.com /health' \
  'dev.strovak.com /' \
  'ext-staging.strovak.com /health'; do
  read -r host path <<<"$host_path"
  retry public_get "$host" "$path"
done

echo "HTTPS, headers, redirects, CORS, restricted EBS, public routing, and WSS checks passed."
