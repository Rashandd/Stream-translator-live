#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
failures=0

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  failures=$((failures + 1))
}

require_ignore_rule() {
  local dockerignore=$1
  local rule=$2

  grep -Fqx -- "$rule" "$dockerignore" \
    || fail "${dockerignore#"$repo_root"/} must contain '$rule'"
}

check_pinned_froms() {
  local dockerfile=$1
  local from_line image

  while IFS= read -r from_line; do
    image=$(awk '{ for (i = 2; i <= NF; i++) if ($i !~ /^--/) { print $i; exit } }' <<<"$from_line")
    [[ $image =~ @sha256:[0-9a-f]{64}$ ]] \
      || fail "${dockerfile#"$repo_root"/} has an unpinned base image: $image"
  done < <(grep -E '^FROM[[:space:]]' "$dockerfile")
}

for dockerignore in "$repo_root/.dockerignore" "$repo_root/frontend/.dockerignore"; do
  if [[ ! -f $dockerignore ]]; then
    fail "${dockerignore#"$repo_root"/} is missing"
    continue
  fi

  for rule in \
    '.git/' \
    '.env' \
    '.env.*' \
    'node_modules/' \
    '**/node_modules/' \
    'target/' \
    '**/target/' \
    'codex.md' \
    'claude.md' \
    '*.pem' \
    '*.key' \
    '*.enc' \
    'google-client-secret.json'; do
    require_ignore_rule "$dockerignore" "$rule"
  done
done

for required_source in \
  Cargo.toml \
  Cargo.lock \
  backend/Cargo.toml \
  sdk/src-tauri/Cargo.toml \
  infra/stt-server/bootstrap.sh \
  stt-server/main.py \
  stt-server/requirements.txt \
  frontend/package.json \
  frontend/package-lock.json \
  frontend/next.config.mjs; do
  [[ -f "$repo_root/$required_source" ]] || fail "required build source is missing: $required_source"
done

grep -Eq '^COPY[[:space:]].*sdk/src-tauri/Cargo\.toml' "$repo_root/backend/Dockerfile" \
  || fail 'backend/Dockerfile must copy the SDK workspace manifest'
grep -Eq '^RUN[[:space:]]+cargo build .*--locked' "$repo_root/backend/Dockerfile" \
  || fail 'backend/Dockerfile must build with Cargo.lock enforcement'
grep -Eq '^FROM[[:space:]]+rust:1\.88-bookworm@sha256:[0-9a-f]{64}[[:space:]]+AS[[:space:]]+builder$' \
  "$repo_root/backend/Dockerfile" \
  || fail 'backend/Dockerfile must use a pinned Rust 1.88 builder for the locked dependencies'

check_pinned_froms "$repo_root/backend/Dockerfile"
check_pinned_froms "$repo_root/frontend/Dockerfile"

if ((failures > 0)); then
  printf '%d Docker build contract check(s) failed\n' "$failures" >&2
  exit 1
fi

printf 'Docker build contract checks passed\n'
