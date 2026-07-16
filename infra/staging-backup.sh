#!/usr/bin/env bash
set -euo pipefail

if [ "$(id -u)" -ne 0 ]; then
  exec sudo -- "$(readlink -f "$0")" "$@"
fi

umask 077
script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=infra/staging-common.sh
source "$script_dir/staging-common.sh"
staging_require_env
release=$(staging_current_release)

backup_root="$STAGING_BASE/backups"
stamp=$(date -u +%Y%m%dT%H%M%SZ)
final_dir="$backup_root/$stamp"
partial_dir=$(mktemp -d "$backup_root/.${stamp}.partial.XXXXXX")

backend_stopped=0
redis_stopped=0
cleanup() {
  status=$?
  if [ "$redis_stopped" -eq 1 ]; then
    staging_wait_up "$release" redis >/dev/null 2>&1 || true
  fi
  if [ "$backend_stopped" -eq 1 ]; then
    staging_wait_up "$release" backend >/dev/null 2>&1 || true
  fi
  if [ "$status" -ne 0 ]; then
    if [ -n "$partial_dir" ]; then
      rm -rf "$partial_dir"
    fi
  fi
  exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

# Stop the writer while PostgreSQL and encrypted file stores are captured so
# they describe one application point in time. Frontend/Caddy stay up, but API
# requests fail closed for this short maintenance window.
staging_compose "$release" stop -t 30 backend
backend_stopped=1

# shellcheck disable=SC2016 # Expanded inside the PostgreSQL container.
staging_compose "$release" exec -T postgres sh -ec \
  'exec pg_dump -U "$POSTGRES_USER" -d "$POSTGRES_DB" --format=custom' \
  > "$partial_dir/postgres.dump"

# shellcheck disable=SC2016 # Expanded inside the Redis container.
staging_compose "$release" exec -T redis sh -ec \
  'redis-cli -a "$REDIS_PASSWORD" --no-auth-warning SAVE' >/dev/null
staging_compose "$release" stop -t 30 redis
redis_stopped=1

docker run --rm \
  -v staging_backend_data:/data:ro \
  -v "$partial_dir:/backup" \
  "$STAGING_ALPINE_IMAGE" \
  tar -C /data -czf /backup/backend-data.tgz .
docker run --rm \
  -v staging_redis:/data:ro \
  -v "$partial_dir:/backup" \
  "$STAGING_ALPINE_IMAGE" \
  tar -C /data -czf /backup/redis-data.tgz .

(
  cd "$partial_dir"
  sha256sum postgres.dump backend-data.tgz redis-data.tgz > SHA256SUMS
)
printf '%s\n' "$(basename "$release")" > "$partial_dir/RELEASE"
chmod 0600 "$partial_dir"/*
chown -R streamdubber:streamdubber "$partial_dir"
mv "$partial_dir" "$final_dir"
partial_dir=

staging_wait_up "$release" redis >/dev/null
redis_stopped=0
staging_wait_up "$release" backend >/dev/null
backend_stopped=0

# Retain only this deployment's dated backup directories for 14 days.
find "$backup_root" -mindepth 1 -maxdepth 1 -type d \
  -name '20??????T??????Z' -mtime +14 -exec rm -rf -- {} +

trap - EXIT INT TERM
echo "Backup created at $final_dir (runtime encryption keys remain separate in $STAGING_ENV_FILE)."
