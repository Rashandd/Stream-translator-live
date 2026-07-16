#!/usr/bin/env bash
set -euo pipefail

if [ "$(id -u)" -ne 0 ]; then
  exec sudo -- "$(readlink -f "$0")" "$@"
fi

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=infra/staging-common.sh
source "$script_dir/staging-common.sh"
staging_require_env

current=$(staging_current_release)
target=$(readlink -f "$STAGING_BASE/previous" 2>/dev/null || true)
if [ -z "$target" ] || [ "$target" = "$current" ] || [ ! -f "$target/$STAGING_COMPOSE_FILE" ]; then
  echo "No distinct previous staging release is available." >&2
  exit 1
fi

staging_compose "$target" config --quiet
rollback_succeeded=0
recover() {
  if [ "$rollback_succeeded" -eq 1 ]; then
    return
  fi
  echo "Rollback target failed health checks; restoring the current release." >&2
  staging_compose "$current" up -d --build --wait --wait-timeout 240 \
    postgres redis backend frontend caddy >/dev/null 2>&1 || true
}
trap recover ERR
trap 'recover; exit 130' INT
trap 'recover; exit 143' TERM

export COMPOSE_PARALLEL_LIMIT=1
staging_compose "$target" up -d --build --remove-orphans --wait --wait-timeout 240
"$target/infra/staging-health.sh" --retry

ln -sfn "$target" "$STAGING_BASE/current"
ln -sfn "$current" "$STAGING_BASE/previous"
rollback_succeeded=1
trap - ERR INT TERM
staging_compose "$target" ps
echo "Rolled back to release $(basename "$target"); the replaced release is now the roll-forward target."
