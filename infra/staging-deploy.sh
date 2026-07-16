#!/usr/bin/env bash
set -euo pipefail

if [ "$(id -u)" -ne 0 ]; then
  exec sudo -- "$(readlink -f "$0")" "$@"
fi

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
source_root=$(cd "$script_dir/.." && pwd)
# shellcheck source=infra/staging-common.sh
source "$script_dir/staging-common.sh"

staging_require_env
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

old_release=$(readlink -f "$STAGING_BASE/current" 2>/dev/null || true)
nginx_was_active=0
if systemctl is-active --quiet nginx; then
  nginx_was_active=1
fi

deployment_succeeded=0
recover() {
  if [ "$deployment_succeeded" -eq 1 ]; then
    return
  fi
  echo "Deployment failed; restoring the prior edge/application state." >&2
  staging_compose "$release" stop caddy >/dev/null 2>&1 || true
  if [ -n "$old_release" ] && [ -f "$old_release/$STAGING_COMPOSE_FILE" ]; then
    staging_compose "$old_release" up -d --build --wait --wait-timeout 240 \
      postgres redis backend frontend caddy >/dev/null 2>&1 || true
  elif [ "$nginx_was_active" -eq 1 ]; then
    systemctl enable nginx >/dev/null 2>&1 || true
    systemctl start nginx || true
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

staging_compose "$release" run --rm --no-deps caddy \
  caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile
systemctl is-active --quiet rashand-api-bridge.socket

if [ "$nginx_was_active" -eq 1 ]; then
  systemctl stop nginx
fi
staging_compose "$release" up -d --force-recreate --wait --wait-timeout 240 caddy
"$release/infra/staging-health.sh" --retry

if [ -n "$old_release" ] && [ "$old_release" != "$release" ]; then
  ln -sfn "$old_release" "$STAGING_BASE/previous"
fi
ln -sfn "$release" "$STAGING_BASE/current"

# Caddy now owns 80/443. Prevent a reboot race for those ports; recovery above
# re-enables Nginx if the first cutover fails.
systemctl disable nginx >/dev/null
systemctl stop nginx >/dev/null
if systemctl is-enabled --quiet nginx || systemctl is-active --quiet nginx; then
  echo "Nginx still competes for the Caddy edge ports." >&2
  false
fi

deployment_succeeded=1
trap - ERR INT TERM
staging_compose "$release" ps
echo "Activated immutable staging release $commit."
