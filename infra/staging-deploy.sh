#!/usr/bin/env bash
set -euo pipefail

if [ "$(id -u)" -ne 0 ]; then
  exec sudo -- "$(readlink -f "$0")" "$@"
fi

origin_only=0
if [ "${1:-}" = "--origin-only" ]; then
  origin_only=1
  shift
fi
if [ "$#" -ne 0 ]; then
  echo "Usage: $0 [--origin-only]" >&2
  exit 2
fi

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
source_root=$(cd "$script_dir/.." && pwd)
# shellcheck source=infra/staging-common.sh
source "$script_dir/staging-common.sh"

staging_require_env
staging_acquire_operation_lock
git_safe=(git -c "safe.directory=$source_root" -C "$source_root")
commit=$("${git_safe[@]}" rev-parse --verify HEAD)

if ! "${git_safe[@]}" diff-index --quiet HEAD --; then
  echo "Tracked changes are not committed; refusing an ambiguous release." >&2
  exit 1
fi
if [ -n "$("${git_safe[@]}" ls-files --others --exclude-standard)" ]; then
  echo "Untracked files are present; commit or remove them before deployment." >&2
  exit 1
fi

# Multi-stage Rust and Next.js images need room for both BuildKit cache and
# release artifacts. Check Docker's data root and both release/source paths
# before creating a release archive or pulling/building any image.
staging_require_build_disk_space "$source_root"

release="$STAGING_BASE/releases/$commit"
marker="$release/.streamdubber-release"
if [ -e "$release" ] && { [ ! -f "$marker" ] || [ "$(cat "$marker")" != "$commit" ]; }; then
  echo "Incomplete or mismatched release directory exists: $release" >&2
  exit 1
fi

partial=
cleanup_partial() {
  if [ -n "$partial" ] && [ -d "$partial" ]; then
    rm -rf "$partial"
  fi
}
trap cleanup_partial EXIT

if [ ! -d "$release" ]; then
  partial="$STAGING_BASE/releases/.${commit}.partial.$$"
  install -d -o streamdubber -g streamdubber -m 0750 "$partial"
  "${git_safe[@]}" archive --format=tar "$commit" | tar -xf - -C "$partial"
  test -f "$partial/$STAGING_COMPOSE_FILE"
  printf '%s\n' "$commit" > "$partial/.streamdubber-release"
  chown -R streamdubber:streamdubber "$partial"
  chmod -R a+rX "$partial"
  mv "$partial" "$release"
  partial=
fi

staging_compose "$release" config --quiet

old_current_link=$(readlink "$STAGING_BASE/current" 2>/dev/null || true)
old_previous_link=$(readlink "$STAGING_BASE/previous" 2>/dev/null || true)
old_release=$(readlink -f "$STAGING_BASE/current" 2>/dev/null || true)
nginx_was_active=0
if systemctl is-active --quiet nginx; then
  nginx_was_active=1
fi
nginx_was_enabled=0
if systemctl is-enabled --quiet nginx; then
  nginx_was_enabled=1
fi

predeploy_backup=
if [ -n "$old_release" ] && [ -f "$old_release/$STAGING_COMPOSE_FILE" ]; then
  # Capture the old release's database and encrypted stores before a new
  # backend can run migrations. The new release's script is used deliberately:
  # it shares this deployment lock even when the current release predates it.
  old_backend_healthy=0
  old_backend=$(staging_compose "$old_release" ps -q backend 2>/dev/null || true)
  if [ -n "$old_backend" ]; then
    old_backend_state=$(docker inspect --format '{{.State.Status}}/{{if .State.Health}}{{.State.Health.Status}}{{else}}missing{{end}}' "$old_backend" 2>/dev/null || true)
    if [ "$old_backend_state" = 'running/healthy' ]; then
      old_backend_healthy=1
    fi
  fi
  if [ "$old_backend_healthy" -eq 1 ]; then
    predeploy_backup=$("$release/infra/staging-backup.sh" --print-path)
  else
    # A broken current release must not be rebuilt by the backup cleanup path.
    # The snapshot is still consistent because the backend is already stopped.
    echo "Active staging backend is unhealthy; keeping it stopped after the pre-deploy backup." >&2
    predeploy_backup=$(STAGING_BACKUP_SKIP_BACKEND_RESTART=1 "$release/infra/staging-backup.sh" --print-path)
  fi
  case "$predeploy_backup" in
    "$STAGING_BASE"/backups/*) ;;
    *)
      echo "Pre-deploy backup did not return a safe staging backup path." >&2
      exit 1
      ;;
  esac
  if [ ! -d "$predeploy_backup" ]; then
    echo "Pre-deploy backup directory is missing: $predeploy_backup" >&2
    exit 1
  fi
  printf '%s\n' "$predeploy_backup" > "$release/.predeploy-backup"
  chown root:root "$release/.predeploy-backup"
  chmod 0600 "$release/.predeploy-backup"
fi

deployment_succeeded=0
deployment_data_may_have_changed=0
restore_release_links() {
  staging_set_symlink "$STAGING_BASE/current" "$old_current_link" || true
  staging_set_symlink "$STAGING_BASE/previous" "$old_previous_link" || true
}

restore_nginx_state() {
  if [ "$nginx_was_enabled" -eq 1 ]; then
    systemctl enable nginx >/dev/null 2>&1 || true
  else
    systemctl disable nginx >/dev/null 2>&1 || true
  fi
  if [ "$nginx_was_active" -eq 1 ]; then
    systemctl start nginx >/dev/null 2>&1 || true
  else
    systemctl stop nginx >/dev/null 2>&1 || true
  fi
}

recover() {
  if [ "$deployment_succeeded" -eq 1 ]; then
    return
  fi
  echo "Deployment failed; restoring the prior edge/application state." >&2
  trap - ERR
  staging_compose "$release" stop caddy >/dev/null 2>&1 || true
  restore_release_links
  data_recovery_succeeded=1
  if [ "$deployment_data_may_have_changed" -eq 1 ]; then
    if [ -z "$predeploy_backup" ] || [ -z "$old_release" ]; then
      echo "No pre-deploy snapshot is available; refusing to start an old release against possibly migrated data." >&2
      data_recovery_succeeded=0
    else
      echo "Restoring the pre-deploy snapshot before restarting the old release." >&2
      if ! staging_compose "$old_release" build --pull backend frontend; then
        echo "Could not rebuild the old release before data recovery." >&2
        data_recovery_succeeded=0
      elif ! CONFIRM_RESTORE=staging \
        STAGING_RESTORE_START_RELEASE="$old_release" \
        "$release/infra/staging-restore.sh" "$predeploy_backup"; then
        echo "Pre-deploy recovery restore failed; refusing to start the old release against possibly migrated data." >&2
        data_recovery_succeeded=0
      fi
    fi
  fi
  if [ "$nginx_was_active" -eq 1 ] && [ -n "$old_release" ]; then
    staging_compose "$old_release" stop caddy >/dev/null 2>&1 || true
  fi
  if [ "$nginx_was_active" -eq 1 ]; then
    # Nginx owned the original edge, so restore it before rebuilding any
    # application containers and never race it with an old Caddy instance.
    restore_nginx_state
  fi
  if [ "$data_recovery_succeeded" -eq 1 ] && [ -n "$old_release" ] && [ -f "$old_release/$STAGING_COMPOSE_FILE" ]; then
    if [ "$nginx_was_active" -eq 1 ]; then
      staging_compose "$old_release" up -d --build --wait --wait-timeout 240 \
        postgres redis backend frontend >/dev/null 2>&1 || true
    else
      staging_compose "$old_release" up -d --build --wait --wait-timeout 240 \
        postgres redis backend frontend caddy >/dev/null 2>&1 || true
    fi
  fi
  if [ "$nginx_was_active" -eq 0 ]; then
    restore_nginx_state
  fi
}
trap recover ERR
trap 'recover; exit 130' INT
trap 'recover; exit 143' TERM

export COMPOSE_PARALLEL_LIMIT=1
staging_compose "$release" pull postgres redis caddy
staging_compose "$release" build --pull backend
staging_compose "$release" build --pull frontend
staging_wait_up "$release" postgres redis
deployment_data_may_have_changed=1
staging_wait_up "$release" backend

# Migrations are embedded and complete before backend health turns green. Seed
# only documented keys from the immutable release, then recreate the backend so
# the persisted values are loaded before any staging traffic is accepted.
# shellcheck disable=SC2016 # Expanded inside the PostgreSQL container.
staging_compose "$release" exec -T postgres sh -ec \
  'exec psql -v ON_ERROR_STOP=1 -U "$POSTGRES_USER" -d "$POSTGRES_DB"' \
  < "$release/infra/staging-config.sql"
staging_compose "$release" up -d --no-deps --force-recreate --wait --wait-timeout 240 backend
staging_wait_up "$release" frontend

staging_validate_caddy_config "$release"

if [ "$nginx_was_active" -eq 1 ]; then
  systemctl stop nginx
fi
staging_compose "$release" up -d --force-recreate --wait --wait-timeout 240 caddy
health_args=(--retry)
if [ "$origin_only" -eq 1 ]; then
  health_args+=(--origin-only)
fi
"$release/infra/staging-health.sh" "${health_args[@]}"

# Caddy now owns 80/443. Prevent a reboot race for those ports before changing
# release links; recover() restores both the captured links and Nginx state if
# any promotion step fails.
systemctl disable nginx >/dev/null
systemctl stop nginx >/dev/null
if systemctl is-enabled --quiet nginx || systemctl is-active --quiet nginx; then
  echo "Nginx still competes for the Caddy edge ports." >&2
  false
fi

if [ -n "$old_release" ] && [ "$old_release" != "$release" ]; then
  staging_set_symlink "$STAGING_BASE/previous" "$old_release"
elif [ -z "$old_release" ]; then
  staging_set_symlink "$STAGING_BASE/previous" ""
fi
staging_set_symlink "$STAGING_BASE/current" "$release"

deployment_succeeded=1
trap - ERR INT TERM
staging_compose "$release" ps
if [ "$origin_only" -eq 1 ]; then
  echo "WARNING: public DNS/TLS checks were skipped; run staging-status.sh after the external DNS/edge change." >&2
fi
echo "Activated immutable staging release $commit."
