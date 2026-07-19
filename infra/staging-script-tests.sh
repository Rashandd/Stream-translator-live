#!/usr/bin/env bash
set -euo pipefail

root=$(git rev-parse --show-toplevel)
tmp_dir=$(mktemp -d)
trap 'rm -rf "$tmp_dir"' EXIT

# shellcheck source=infra/staging-common.sh
source "$root/infra/staging-common.sh"

fail() {
  echo "staging script regression test failed: $*" >&2
  exit 1
}

assert_contains() {
  local needle=$1
  local haystack=$2
  [[ "$haystack" == *"$needle"* ]] || fail "expected output to contain $needle"
}

# Promotion/recovery must be able to atomically replace a release link and to
# restore an absent link without treating the symlink's target as a directory.
mkdir -p "$tmp_dir/release-one" "$tmp_dir/release-two"
staging_set_symlink "$tmp_dir/current" "$tmp_dir/release-one"
test "$(readlink "$tmp_dir/current")" = "$tmp_dir/release-one" || fail "initial symlink target"
staging_set_symlink "$tmp_dir/current" "$tmp_dir/release-two"
test "$(readlink "$tmp_dir/current")" = "$tmp_dir/release-two" || fail "replaced symlink target"
staging_set_symlink "$tmp_dir/previous" ""
test ! -e "$tmp_dir/previous" || fail "absent symlink restore"

# A second mutating operation must fail rather than race the first one. The
# child keeps the lock until the FIFO is released below.
lock_file="$tmp_dir/operations.lock"
: > "$lock_file"
chmod 0600 "$lock_file"
mkfifo "$tmp_dir/release-lock"
(
  source "$root/infra/staging-common.sh"
  STAGING_OPERATION_LOCK_FILE="$lock_file"
  staging_acquire_operation_lock
  : > "$tmp_dir/lock-held"
  read -r _ < "$tmp_dir/release-lock"
) &
lock_owner=$!
for _ in $(seq 1 100); do
  [ -e "$tmp_dir/lock-held" ] && break
  sleep 0.05
done
[ -e "$tmp_dir/lock-held" ] || fail "lock owner did not start"
STAGING_OPERATION_LOCK_FILE="$lock_file"
if staging_acquire_operation_lock 2>/dev/null; then
  fail "second operation acquired a held lock"
fi
printf 'release\n' > "$tmp_dir/release-lock"
wait "$lock_owner"
staging_acquire_operation_lock
# A nested backup/restore child must inherit the held FD and succeed without
# taking a second flock (the deploy pre-snapshot follows this path).
bash -c '
  source "$1"
  STAGING_OPERATION_LOCK_FILE="$2"
  staging_acquire_operation_lock
' _ "$root/infra/staging-common.sh" "$lock_file"

# Staging-sensitive ports may never listen beyond loopback, including TCP/3000.
ss() {
  cat <<'EOF'
tcp LISTEN 0 4096 127.0.0.1:3000 0.0.0.0:*
tcp LISTEN 0 4096 [::1]:8080 [::]:*
tcp LISTEN 0 4096 0.0.0.0:3000 0.0.0.0:*
tcp LISTEN 0 4096 0.0.0.0:5432 0.0.0.0:*
udp UNCONN 0 0 [::]:6379 [::]:*
tcp LISTEN 0 4096 172.30.10.1:8200 0.0.0.0:*
tcp LISTEN 0 4096 0.0.0.0:80 0.0.0.0:*
EOF
}
unexpected_listeners=$(staging_non_loopback_forbidden_listeners)
assert_contains '0.0.0.0:3000' "$unexpected_listeners"
assert_contains '0.0.0.0:5432' "$unexpected_listeners"
assert_contains '[::]:6379' "$unexpected_listeners"
assert_contains '172.30.10.1:8200' "$unexpected_listeners"
[[ "$unexpected_listeners" != *'127.0.0.1:3000'* ]] || fail "loopback IPv4 listener was rejected"
[[ "$unexpected_listeners" != *'[::1]:8080'* ]] || fail "loopback IPv6 listener was rejected"
unset -f ss

# The deploy preflight must refuse an undersized Docker data filesystem before
# a multi-stage build begins and explain the documented threshold.
STAGING_BASE="$tmp_dir/base"
mkdir -p "$STAGING_BASE" "$tmp_dir/docker-root" "$tmp_dir/source"
docker() {
  if [ "$1" = info ]; then
    printf '%s\n' "$tmp_dir/docker-root"
    return 0
  fi
  return 1
}
df() {
  printf '%s\n' 'Filesystem 1024-blocks Used Available Capacity Mounted on'
  printf 'mockfs 20000000 16000000 %s 80%% %s\n' "$STAGING_TEST_FREE_KIB" "$2"
}
STAGING_TEST_FREE_KIB=4500000
if staging_require_build_disk_space "$tmp_dir/source" 2>"$tmp_dir/disk-error"; then
  fail "low disk preflight unexpectedly passed"
fi
assert_contains 'at least 12 GiB' "$(<"$tmp_dir/disk-error")"
STAGING_TEST_FREE_KIB=$((12 * 1024 * 1024))
staging_require_build_disk_space "$tmp_dir/source"
unset -f docker df

# The Caddy image does not provide `validate` as an entrypoint. Keep the
# executable explicit so an isolated validation container cannot fail before
# it reads the staging Caddyfile.
staging_caddy_image() {
  printf '%s\n' 'caddy:2.11.4-alpine@sha256:test'
}
docker() {
  printf '%s\n' "$*" > "$tmp_dir/caddy-command"
}
staging_validate_caddy_config "$tmp_dir/source"
assert_contains ' caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile' "$(<"$tmp_dir/caddy-command")"
unset -f staging_caddy_image docker

# A failed current backend must not be rebuilt by the pre-deploy snapshot's
# cleanup path. The deploy passes this internal switch only after confirming
# that the current backend is not healthy.
backup_script=$(<"$root/infra/staging-backup.sh")
deploy_script=$(<"$root/infra/staging-deploy.sh")
assert_contains 'STAGING_BACKUP_SKIP_BACKEND_RESTART' "$backup_script"
assert_contains 'STAGING_BACKUP_SKIP_BACKEND_RESTART=1' "$deploy_script"
assert_contains "Active staging backend is unhealthy" "$deploy_script"

# A pending DNS/CDN edge-certificate change must not block installing and
# fully checking the origin. --origin-only skips only the three public
# DNS lookups; every other health check still runs.
health_script=$(<"$root/infra/staging-health.sh")
assert_contains '--origin-only' "$deploy_script"
assert_contains '--origin-only' "$health_script"
assert_contains 'check_public' "$health_script"

echo "Staging script regression tests passed."
