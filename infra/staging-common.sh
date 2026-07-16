#!/usr/bin/env bash

STAGING_BASE=/srv/streamdubber
STAGING_ENV_FILE=/etc/streamdubber/staging.env
STAGING_COMPOSE_FILE=infra/compose.staging.yml
# shellcheck disable=SC2034 # Used by scripts that source this shared file.
STAGING_ALPINE_IMAGE='alpine:3.22@sha256:14358309a308569c32bdc37e2e0e9694be33a9d99e68afb0f5ff33cc1f695dce'

staging_require_env() {
  if [ ! -r "$STAGING_ENV_FILE" ]; then
    echo "Missing or unreadable $STAGING_ENV_FILE; run staging-host-bootstrap.sh first." >&2
    return 1
  fi
  local key
  for key in POSTGRES_DB POSTGRES_USER POSTGRES_PASSWORD REDIS_PASSWORD JWT_SECRET SECRETS_ENC_KEY; do
    if ! grep -Eq "^${key}=.+" "$STAGING_ENV_FILE"; then
      echo "$STAGING_ENV_FILE is missing a non-empty $key value." >&2
      return 1
    fi
  done
  if ! grep -qx 'ADMIN_PASSWORD_HASH=' "$STAGING_ENV_FILE" && \
    ! grep -Eq "^ADMIN_PASSWORD_HASH='[$]argon2id[$][^']+'$" "$STAGING_ENV_FILE"; then
    echo "ADMIN_PASSWORD_HASH must be empty or a single-quoted argon2id PHC value." >&2
    return 1
  fi
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
