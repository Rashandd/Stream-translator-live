#!/usr/bin/env bash
set -euo pipefail

if [ "$(id -u)" -ne 0 ]; then
  exec sudo -- "$(readlink -f "$0")" "$@"
fi

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=infra/staging-common.sh
source "$script_dir/staging-common.sh"
staging_require_env
release=$(staging_current_release)
exec docker compose \
  --project-directory "$release" \
  -f "$release/$STAGING_COMPOSE_FILE" \
  --env-file "$STAGING_ENV_FILE" \
  logs --tail=200 -f "$@"
