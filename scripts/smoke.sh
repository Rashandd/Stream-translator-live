#!/usr/bin/env bash
# Lokal canlı smoke testi — gerçek backend binary'sini geçici bir ortamla
# ayağa kaldırır ve MVP kritik akışlarını CANLI doğrular:
#   health → admin login → panelden secret girme (maskeli okuma) →
#   config toggle → pairing (kod → SDK token) → relay hedefleri →
#   audit logları → RESTART → kalıcılık (secret/relay/usage dosyaları).
#
# Kullanım: bash scripts/smoke.sh   (repo kökünde; cargo gerektirir)

set -euo pipefail
cd "$(dirname "$0")/.."

WORK="$(mktemp -d /tmp/sd-smoke.XXXXXX)"
PORT=$((20000 + RANDOM % 20000))
BASE="http://127.0.0.1:${PORT}"
BACKEND_PID=""

cleanup() {
    [[ -n "$BACKEND_PID" ]] && kill "$BACKEND_PID" 2>/dev/null || true
    rm -rf "$WORK"
}
trap cleanup EXIT

log() { printf '\033[1;34m[smoke]\033[0m %s\n' "$*"; }
fail() { printf '\033[1;31m[smoke FAIL]\033[0m %s\n' "$*" >&2; exit 1; }

start_backend() {
    BIND_ADDR="127.0.0.1:${PORT}" \
    JWT_SECRET="smoke-test-jwt-secret-at-least-32-bytes!" \
    ADMIN_EMAIL="ops@smoke.test" \
    ADMIN_PASSWORD="smoke-pass" \
    SECRETS_FILE="$WORK/secrets.enc" \
    TOKENS_FILE="$WORK/tokens.enc" \
    RELAY_FILE="$WORK/relay.enc" \
    USAGE_FILE="$WORK/usage.jsonl" \
    LOG_FILE="$WORK/logs.jsonl" \
    RUST_LOG=warn \
    cargo run --quiet --bin streamdubber-backend &
    BACKEND_PID=$!

    for _ in $(seq 1 60); do
        if curl -sf "$BASE/health" >/dev/null 2>&1; then
            return 0
        fi
        sleep 0.5
    done
    fail "backend did not become healthy"
}

stop_backend() {
    kill "$BACKEND_PID" 2>/dev/null || true
    wait "$BACKEND_PID" 2>/dev/null || true
    BACKEND_PID=""
}

json() { python3 -c "import json,sys; d=json.load(sys.stdin); print(d$1)"; }

log "building + starting backend (temp env: $WORK, port $PORT)"
start_backend
log "1. /health OK"

# --- admin login ---
TOKEN=$(curl -sf -X POST "$BASE/api/auth/login" \
    -H 'Content-Type: application/json' \
    -d '{"email":"ops@smoke.test","password":"smoke-pass"}' | json "['token']")
[[ -n "$TOKEN" ]] || fail "login returned no token"
BAD=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/api/auth/login" \
    -H 'Content-Type: application/json' -d '{"email":"ops@smoke.test","password":"wrong"}')
[[ "$BAD" == "401" ]] || fail "wrong password expected 401, got $BAD"
log "2. admin login OK (yanlis sifre 401)"

AUTH="Authorization: Bearer $TOKEN"

# --- panelden secret girme + maskeli okuma ---
RESP=$(curl -sf -X PUT "$BASE/admin/secrets/DEEPGRAM_KEY" -H "$AUTH" \
    -H 'Content-Type: application/json' -d '{"value":"dg_live_smoketest_1234"}')
echo "$RESP" | grep -q 'dg_live_smoketest_1234' && fail "full secret leaked in response"
echo "$RESP" | grep -q 'dg_l…1234' || fail "masked value missing in response"
ANON=$(curl -s -o /dev/null -w '%{http_code}' "$BASE/admin/secrets")
[[ "$ANON" == "401" ]] || fail "anonymous /admin/secrets expected 401, got $ANON"
log "3. secret girildi, sadece maskeli okunuyor (anonim 401)"

# --- config toggle (restart'siz) ---
curl -sf -X PUT "$BASE/admin/config" -H "$AUTH" -H 'Content-Type: application/json' \
    -d '{"key":"providers.mock_fallback","value":"false"}' >/dev/null
curl -sf "$BASE/config" | grep -q '"providers.mock_fallback":"false"' \
    || fail "config toggle did not apply"
log "4. config toggle aninda uygulandi (providers.mock_fallback=false)"

# --- pairing: kod → SDK token → relay hedefleri ---
CODE=$(curl -sf -X POST "$BASE/api/pairing/code" -H "$AUTH" \
    -H 'Content-Type: application/json' -d '{}' | json "['code']")
[[ ${#CODE} == 6 ]] || fail "pairing code not 6 digits: $CODE"
SDK_TOKEN=$(curl -sf -X POST "$BASE/api/pairing/claim" \
    -H 'Content-Type: application/json' \
    -d "{\"pairing_code\":\"$CODE\",\"device_name\":\"Smoke PC\",\"sdk_version\":\"0.0.0\"}" \
    | json "['token']")
[[ -n "$SDK_TOKEN" ]] || fail "pairing claim returned no token"

curl -sf -X PUT "$BASE/api/relay/targets" -H "$AUTH" -H 'Content-Type: application/json' \
    -d '{"targets":[{"platform":"youtube","rtmp_url":"rtmp://a.rtmp.youtube.com/live2","stream_key":"smoke-yt-key"}]}' >/dev/null
GOT=$(curl -sf "$BASE/api/relay/targets" -H "Authorization: Bearer $SDK_TOKEN" | json "[0]['stream_key']")
[[ "$GOT" == "smoke-yt-key" ]] || fail "sdk could not read relay targets"
log "5. pairing + relay hedef senkronu OK (SDK token'i ile okundu)"

# --- audit loglari ---
LOGS=$(curl -sf "$BASE/admin/logs?limit=100" -H "$AUTH")
echo "$LOGS" | grep -q 'secret updated: DEEPGRAM_KEY' || fail "secret audit log missing"
echo "$LOGS" | grep -q 'sdk paired' || fail "pairing audit log missing"
echo "$LOGS" | grep -q 'smoke-yt-key' && fail "stream key leaked into logs"
log "6. audit loglari OK (secret/pairing izli, stream key sizintisi yok)"

# --- RESTART: kaliciligi dogrula ---
log "7. backend yeniden baslatiliyor (kalicilik testi)..."
stop_backend
start_backend

TOKEN=$(curl -sf -X POST "$BASE/api/auth/login" \
    -H 'Content-Type: application/json' \
    -d '{"email":"ops@smoke.test","password":"smoke-pass"}' | json "['token']")
AUTH="Authorization: Bearer $TOKEN"

SRC=$(curl -sf "$BASE/admin/secrets" -H "$AUTH" \
    | python3 -c "import json,sys; d=json.load(sys.stdin); print(next(i['source']+':'+(i['masked'] or '') for i in d if i['name']=='DEEPGRAM_KEY'))")
[[ "$SRC" == "panel:dg_l…1234" ]] || fail "secret did not survive restart: $SRC"

GOT=$(curl -sf "$BASE/api/relay/targets" -H "Authorization: Bearer $SDK_TOKEN" | json "[0]['stream_key']")
[[ "$GOT" == "smoke-yt-key" ]] || fail "relay targets did not survive restart"

grep -q 'dg_live_smoketest_1234' "$WORK/secrets.enc" && fail "secrets file contains plaintext"
grep -q 'smoke-yt-key' "$WORK/relay.enc" && fail "relay file contains plaintext"
log "8. restart sonrasi kalicilik OK (secret=panel/maskeli, relay hedefi durdu, dosyalar sifreli)"

stop_backend
printf '\033[1;32m[smoke PASS]\033[0m tum canli dogrulamalar gecti\n'
