#!/usr/bin/env bash

STAGING_BASE=/srv/streamdubber
STAGING_ENV_FILE=/etc/streamdubber/staging.env
STAGING_COMPOSE_FILE=infra/compose.staging.yml
STAGING_OPERATION_LOCK_FILE=/etc/streamdubber/staging-operation.lock
STAGING_MIN_BUILD_FREE_GIB=12
STAGING_MIN_BUILD_FREE_KIB=$((STAGING_MIN_BUILD_FREE_GIB * 1024 * 1024))
# shellcheck disable=SC2034 # Used by scripts that source this shared file.
STAGING_ALPINE_IMAGE='alpine:3.22@sha256:14358309a308569c32bdc37e2e0e9694be33a9d99e68afb0f5ff33cc1f695dce'

staging_set_symlink() {
  local link=$1
  local target=$2
  local temporary_link

  if [ -e "$link" ] && [ ! -L "$link" ]; then
    echo "Refusing to replace non-symlink release path: $link" >&2
    return 1
  fi
  if [ -z "$target" ]; then
    rm -f -- "$link"
    return 0
  fi

  temporary_link="${link}.new.$$"
  if [ -e "$temporary_link" ] || [ -L "$temporary_link" ]; then
    echo "Refusing to overwrite temporary release link: $temporary_link" >&2
    return 1
  fi
  ln -s -- "$target" "$temporary_link"
  if ! mv -Tf -- "$temporary_link" "$link"; then
    rm -f -- "$temporary_link"
    return 1
  fi
}

staging_operation_lock_is_held() {
  case "${STAGING_OPERATION_LOCK_FD:-}" in
    '' | *[!0-9]*) return 1 ;;
  esac
  [ "${STAGING_OPERATION_LOCK_HELD:-}" = 1 ] && \
    [ -e "/proc/$$/fd/$STAGING_OPERATION_LOCK_FD" ]
}

staging_acquire_operation_lock() {
  local lock_fd

  if staging_operation_lock_is_held; then
    return 0
  fi
  if ! command -v flock >/dev/null; then
    echo "Required host command is missing: flock" >&2
    return 1
  fi
  if [ -e "$STAGING_OPERATION_LOCK_FILE" ] && \
    { [ ! -f "$STAGING_OPERATION_LOCK_FILE" ] || [ -L "$STAGING_OPERATION_LOCK_FILE" ]; }; then
    echo "Staging operation lock is not a regular file: $STAGING_OPERATION_LOCK_FILE" >&2
    return 1
  fi
  if [ ! -e "$STAGING_OPERATION_LOCK_FILE" ]; then
    install -o root -g root -m 0600 /dev/null "$STAGING_OPERATION_LOCK_FILE"
  fi

  exec {lock_fd}>"$STAGING_OPERATION_LOCK_FILE"
  if ! flock -n "$lock_fd"; then
    eval "exec ${lock_fd}>&-"
    echo "Another staging deploy, backup, restore, or rollback is already running; retry after it completes." >&2
    return 1
  fi
  STAGING_OPERATION_LOCK_FD=$lock_fd
  STAGING_OPERATION_LOCK_HELD=1
  export STAGING_OPERATION_LOCK_FD STAGING_OPERATION_LOCK_HELD
}

staging_require_build_disk_space() {
  local docker_root
  local candidate
  local report
  local filesystem
  local available_kib
  local mount_point
  local available_gib

  docker_root=$(docker info --format '{{.DockerRootDir}}' 2>/dev/null || true)
  if [ -z "$docker_root" ] || [ ! -d "$docker_root" ]; then
    echo "Could not determine Docker's data root for the staging build disk-space preflight." >&2
    return 1
  fi

  for candidate in "$STAGING_BASE" "$docker_root" "$@"; do
    if [ ! -d "$candidate" ]; then
      echo "Build disk-space preflight path does not exist: $candidate" >&2
      return 1
    fi
    report=$(df -Pk "$candidate" | awk 'NR == 2 {print $1 "\t" $4 "\t" $6; exit}')
    IFS=$'\t' read -r filesystem available_kib mount_point <<<"$report"
    if ! [[ "$available_kib" =~ ^[0-9]+$ ]]; then
      echo "Could not read available disk space for $candidate." >&2
      return 1
    fi
    if [ "$available_kib" -lt "$STAGING_MIN_BUILD_FREE_KIB" ]; then
      available_gib=$(awk -v kib="$available_kib" 'BEGIN { printf "%.1f", kib / 1048576 }')
      echo "Insufficient free disk space for staging Docker builds on $mount_point: ${available_gib} GiB available; at least ${STAGING_MIN_BUILD_FREE_GIB} GiB is required." >&2
      echo "Review 'docker system df', free only unused build cache/images if safe, or add storage before retrying." >&2
      return 1
    fi
  done
}

staging_require_env() {
  if [ ! -r "$STAGING_ENV_FILE" ]; then
    echo "Missing or unreadable $STAGING_ENV_FILE; run staging-host-bootstrap.sh first." >&2
    return 1
  fi
  local key
  for key in POSTGRES_DB POSTGRES_USER POSTGRES_PASSWORD REDIS_PASSWORD JWT_SECRET SECRETS_ENC_KEY STAGING_PUBLIC_IPV4; do
    if ! grep -Eq "^${key}=.+" "$STAGING_ENV_FILE"; then
      echo "$STAGING_ENV_FILE is missing a non-empty $key value." >&2
      return 1
    fi
  done
  if ! grep -Eq '^STAGING_PUBLIC_IPV4=([0-9]{1,3}\.){3}[0-9]{1,3}$' "$STAGING_ENV_FILE"; then
    echo "$STAGING_ENV_FILE has an invalid STAGING_PUBLIC_IPV4 value." >&2
    return 1
  fi
  if ! grep -qx 'ADMIN_PASSWORD_HASH=' "$STAGING_ENV_FILE" && \
    ! grep -Eq "^ADMIN_PASSWORD_HASH='[$]argon2id[$][^']+'$" "$STAGING_ENV_FILE"; then
    echo "ADMIN_PASSWORD_HASH must be empty or a single-quoted argon2id PHC value." >&2
    return 1
  fi
}

# dev.strovak.com is gated by Caddy Basic Auth. With either value missing Caddy
# still starts, but answers 401 to every request, so the frontend is unusable.
# Catch that before a long image build rather than after it.
staging_require_edge_gate() {
  if ! grep -Eq '^STAGING_BASIC_AUTH_USER=[^[:space:]]+$' "$STAGING_ENV_FILE"; then
    echo "$STAGING_ENV_FILE is missing a non-empty STAGING_BASIC_AUTH_USER value; see infra/staging.md (Edge Basic Auth)." >&2
    return 1
  fi
  if ! grep -Eq "^STAGING_BASIC_AUTH_HASH='[$]2[aby][$][0-9]{2}[$][./A-Za-z0-9]{53}'$" "$STAGING_ENV_FILE"; then
    echo "STAGING_BASIC_AUTH_HASH must be a single-quoted bcrypt value from 'caddy hash-password'; see infra/staging.md (Edge Basic Auth)." >&2
    return 1
  fi
}

# A host that never had Nginx installed has no unit to disable or stop, and
# systemctl reports a missing unit as a failure.
staging_nginx_unit_exists() {
  systemctl cat nginx.service >/dev/null 2>&1
}

staging_compose() {
  local release_root=$1
  shift
  docker compose \
    --project-directory "$release_root" \
    -f "$release_root/$STAGING_COMPOSE_FILE" \
    --env-file "$STAGING_ENV_FILE" \
    "$@"
}

staging_current_release() {
  local current
  current=$(readlink -f "$STAGING_BASE/current" 2>/dev/null || true)
  if [ -z "$current" ] || [ ! -f "$current/$STAGING_COMPOSE_FILE" ]; then
    echo "No active staging release is installed." >&2
    return 1
  fi
  printf '%s\n' "$current"
}

staging_wait_up() {
  local release_root=$1
  shift
  staging_compose "$release_root" up -d --wait --wait-timeout 240 "$@"
}

staging_caddy_image() {
  local release_root=$1
  local image

  image=$(staging_compose "$release_root" config --format json | python3 -c \
    'import json, sys; print(json.load(sys.stdin)["services"]["caddy"]["image"])')
  case "$image" in
    *@sha256:*) printf '%s\n' "$image" ;;
    *)
      echo "Caddy image must be pinned by digest before validation." >&2
      return 1
      ;;
  esac
}

staging_validate_caddy_config() {
  local release_root=$1
  local image

  image=$(staging_caddy_image "$release_root")
  docker run --rm --network none --read-only \
    --tmpfs /config:rw,noexec,nosuid,size=16m \
    --tmpfs /data:rw,noexec,nosuid,size=16m \
    --tmpfs /tmp:rw,noexec,nosuid,size=16m \
    -v "$release_root/infra/Caddyfile.staging:/etc/caddy/Caddyfile:ro" \
    "$image" caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile
}

staging_non_loopback_forbidden_listeners() {
  ss -H -lntu | awk '
    $5 ~ /:(3000|5432|6379|8080|8200)$/ &&
      $5 !~ /^(127\.0\.0\.1|\[::1\]|::1):(3000|5432|6379|8080|8200)$/ {
        print
      }
  '
}
