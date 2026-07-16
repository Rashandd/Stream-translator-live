#!/usr/bin/env bash
set -euo pipefail

if [ "$(id -u)" -ne 0 ]; then
  exec sudo --preserve-env=CONFIRM_RESTORE -- "$(readlink -f "$0")" "$@"
fi
if [ "$#" -ne 1 ]; then
  echo "Usage: $0 /srv/streamdubber/backups/YYYYMMDDTHHMMSSZ" >&2
  exit 2
fi

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=infra/staging-common.sh
source "$script_dir/staging-common.sh"
staging_require_env
release=$(staging_current_release)

backup_dir=$(readlink -f "$1")
case "$backup_dir" in
  "$STAGING_BASE"/backups/*) ;;
  *)
    echo "Restore input must be inside $STAGING_BASE/backups." >&2
    exit 2
    ;;
esac

for file in postgres.dump backend-data.tgz redis-data.tgz SHA256SUMS RELEASE; do
  test -r "$backup_dir/$file"
done
(
  cd "$backup_dir"
  sha256sum --check --quiet SHA256SUMS
)
staging_compose "$release" exec -T postgres pg_restore --list \
  < "$backup_dir/postgres.dump" >/dev/null

python3 - "$backup_dir/backend-data.tgz" "$backup_dir/redis-data.tgz" <<'PY'
import pathlib
import sys
import tarfile

for raw in sys.argv[1:]:
    path = pathlib.Path(raw)
    with tarfile.open(path, "r:gz") as archive:
        for member in archive.getmembers():
            candidate = pathlib.PurePosixPath(member.name)
            if candidate.is_absolute() or ".." in candidate.parts:
                raise SystemExit(f"unsafe archive member in {path.name}")
            if member.issym() or member.islnk() or member.isdev() or member.isfifo():
                raise SystemExit(f"unsafe archive entry type in {path.name}")
PY

if [ "${CONFIRM_RESTORE:-}" != staging ]; then
  echo "Restore dry-run passed. Re-run with CONFIRM_RESTORE=staging to replace staging data."
  exit 0
fi

# Capture a fresh recovery point before the destructive operation.
"$release/infra/staging-backup.sh"

services_stopped=0
recover_services() {
  status=$?
  if [ "$services_stopped" -eq 1 ]; then
    staging_wait_up "$release" postgres redis backend frontend caddy >/dev/null 2>&1 || true
  fi
  exit "$status"
}
trap recover_services EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

staging_compose "$release" stop -t 30 caddy frontend backend redis
services_stopped=1

# --single-transaction makes a failed SQL restore leave the old database
# intact. The encrypted file stores have the automatic pre-restore backup above.
# shellcheck disable=SC2016 # Expanded inside the PostgreSQL container.
staging_compose "$release" exec -T postgres sh -ec \
  'exec pg_restore --clean --if-exists --no-owner --exit-on-error --single-transaction -U "$POSTGRES_USER" -d "$POSTGRES_DB"' \
  < "$backup_dir/postgres.dump"

docker run --rm \
  -v staging_backend_data:/data \
  -v "$backup_dir:/backup:ro" \
  "$STAGING_ALPINE_IMAGE" \
  sh -ec 'find /data -mindepth 1 -delete; tar -C /data -xzf /backup/backend-data.tgz'
docker run --rm \
  -v staging_redis:/data \
  -v "$backup_dir:/backup:ro" \
  "$STAGING_ALPINE_IMAGE" \
  sh -ec 'find /data -mindepth 1 -delete; tar -C /data -xzf /backup/redis-data.tgz'

staging_wait_up "$release" postgres redis backend frontend caddy
services_stopped=0
"$release/infra/staging-health.sh" --retry

trap - EXIT INT TERM
echo "Restore completed from $backup_dir."
