#!/usr/bin/env bash
set -euo pipefail

if [ "$(id -u)" -ne 0 ]; then
  exec sudo --preserve-env=CONFIRM_ROLLBACK_DATA -- "$(readlink -f "$0")" "$@"
fi

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=infra/staging-common.sh
source "$script_dir/staging-common.sh"
staging_require_env
staging_acquire_operation_lock

current=$(staging_current_release)
target=$(readlink -f "$STAGING_BASE/previous" 2>/dev/null || true)
if [ -z "$target" ] || [ "$target" = "$current" ] || [ ! -f "$target/$STAGING_COMPOSE_FILE" ]; then
  echo "No distinct previous staging release is available." >&2
  exit 1
fi

if [ ! -r "$current/.predeploy-backup" ]; then
  echo "Current release has no pre-deploy snapshot metadata; refusing an unsafe code-only rollback." >&2
  exit 1
fi
predeploy_backup=$(<"$current/.predeploy-backup")
case "$predeploy_backup" in
  "$STAGING_BASE"/backups/*) ;;
  *)
    echo "Current release has no valid pre-deploy snapshot; refusing an unsafe code-only rollback." >&2
    exit 1
    ;;
esac
if [ ! -d "$predeploy_backup" ]; then
  echo "Pre-deploy snapshot is missing: $predeploy_backup" >&2
  exit 1
fi
if [ "${CONFIRM_ROLLBACK_DATA:-}" != staging ]; then
  echo "Code-only rollback is disabled because migrations and encrypted stores may be incompatible." >&2
  echo "First validate $predeploy_backup with: sudo $current/infra/staging-restore.sh $predeploy_backup" >&2
  echo "Then run: CONFIRM_ROLLBACK_DATA=staging sudo --preserve-env=CONFIRM_ROLLBACK_DATA $0" >&2
  exit 1
fi

staging_compose "$target" config --quiet
rollback_succeeded=0
rollback_data_restore_started=0
rollback_recovery_backup=
recover() {
  if [ "$rollback_succeeded" -eq 1 ]; then
    return
  fi
  echo "Rollback target failed; restoring the current release and its pre-rollback data." >&2
  trap - ERR
  if [ "$rollback_data_restore_started" -eq 1 ] && [ -n "$rollback_recovery_backup" ]; then
    staging_compose "$current" build --pull backend frontend >/dev/null 2>&1 || true
    CONFIRM_RESTORE=staging \
      STAGING_RESTORE_SKIP_AUTOMATIC_BACKUP=1 \
      STAGING_RESTORE_START_RELEASE="$current" \
      "$current/infra/staging-restore.sh" "$rollback_recovery_backup" >/dev/null 2>&1 || true
  fi
  staging_compose "$current" up -d --build --wait --wait-timeout 240 \
    postgres redis backend frontend caddy >/dev/null 2>&1 || true
}
trap recover ERR
trap 'recover; exit 130' INT
trap 'recover; exit 143' TERM

export COMPOSE_PARALLEL_LIMIT=1
# Keep a recovery point for the data currently used by $current. The nested
# scripts reuse this process's flock, so they cannot race another operation.
rollback_recovery_backup=$("$current/infra/staging-backup.sh" --print-path)
case "$rollback_recovery_backup" in
  "$STAGING_BASE"/backups/*) ;;
  *)
    echo "Pre-rollback backup did not return a safe staging backup path." >&2
    exit 1
    ;;
esac
test -d "$rollback_recovery_backup"

# Build the target before any data is restored. compose otherwise shares image
# tags between immutable releases and could start the current binary against
# the target's pre-deploy database state.
staging_compose "$target" build --pull backend frontend
rollback_data_restore_started=1
CONFIRM_RESTORE=staging \
  STAGING_RESTORE_SKIP_AUTOMATIC_BACKUP=1 \
  STAGING_RESTORE_START_RELEASE="$target" \
  "$current/infra/staging-restore.sh" "$predeploy_backup"

staging_compose "$target" up -d --remove-orphans --wait --wait-timeout 240
"$target/infra/staging-health.sh" --retry

# The target is becoming the active release again. Record the snapshot taken
# just before this rollback so an intentional roll-forward has the matching
# data state rather than the target's stale historical metadata.
printf '%s\n' "$rollback_recovery_backup" > "$target/.predeploy-backup"
chown root:root "$target/.predeploy-backup"
chmod 0600 "$target/.predeploy-backup"
staging_set_symlink "$STAGING_BASE/current" "$target"
staging_set_symlink "$STAGING_BASE/previous" "$current"
rollback_succeeded=1
trap - ERR INT TERM
staging_compose "$target" ps
echo "Restored pre-deploy data and rolled back to release $(basename "$target"); the replaced release is now the roll-forward target."
