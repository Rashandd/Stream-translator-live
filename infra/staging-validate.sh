#!/usr/bin/env bash
set -euo pipefail

root=$(git rev-parse --show-toplevel)
compose_file="$root/infra/compose.staging.yml"
tmp_dir=$(mktemp -d)
trap 'rm -rf "$tmp_dir"' EXIT

scripts=(
  staging-common.sh
  staging-host-bootstrap.sh
  staging-deploy.sh
  staging-health.sh
  staging-status.sh
  staging-logs.sh
  staging-backup.sh
  staging-restore.sh
  staging-rollback.sh
  staging-script-tests.sh
)

for script in "${scripts[@]}"; do
  bash -n "$root/infra/$script"
done

"$root/infra/staging-script-tests.sh"

cat > "$tmp_dir/staging.env" <<'EOF'
POSTGRES_DB=streamdubber_staging
POSTGRES_USER=streamdubber
POSTGRES_PASSWORD=test-postgres-password
REDIS_PASSWORD=test-redis-password
JWT_SECRET=test-jwt-secret-with-at-least-thirty-two-bytes
SECRETS_ENC_KEY=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
STAGING_PUBLIC_IPV4=203.0.113.10
ADMIN_EMAIL=
ADMIN_PASSWORD_HASH=
EOF

docker compose \
  --project-directory "$root" \
  -f "$compose_file" \
  --env-file "$tmp_dir/staging.env" \
  config --format json > "$tmp_dir/compose.json"

python3 - "$root" "$tmp_dir/compose.json" <<'PY'
import json
import pathlib
import sys

root = pathlib.Path(sys.argv[1]).resolve()
cfg = json.loads(pathlib.Path(sys.argv[2]).read_text())
services = cfg["services"]

required = {"frontend", "backend", "postgres", "redis", "caddy"}
assert required <= services.keys(), f"missing services: {required - services.keys()}"

for name, service in services.items():
    assert service.get("restart") == "unless-stopped", f"{name}: restart policy"
    assert "healthcheck" in service, f"{name}: healthcheck missing"
    log = service.get("logging", {})
    assert log.get("driver") == "json-file", f"{name}: log driver"
    assert log.get("options", {}).get("max-size") == "10m", f"{name}: log size"
    assert service.get("pids_limit", 0) > 0, f"{name}: pids_limit missing"

for name in ("postgres", "redis", "backend", "frontend"):
    assert not services[name].get("ports"), f"{name}: must not publish host ports"

published = {(p.get("host_ip"), p.get("published"), p.get("protocol")) for p in services["caddy"]["ports"]}
assert published == {
    ("203.0.113.10", "80", "tcp"),
    ("203.0.113.10", "443", "tcp"),
    ("203.0.113.10", "443", "udp"),
}, published

assert cfg["networks"]["staging_internal"].get("internal") is True
assert cfg["networks"]["staging_edge"]["ipam"]["config"][0]["subnet"] == "172.30.10.0/24"
assert cfg["networks"]["staging_internal"]["ipam"]["config"][0]["subnet"] == "172.30.11.0/24"
for name in ("postgres", "redis"):
    assert set(services[name]["networks"]) == {"staging_internal"}

expected_volumes = {"staging_postgres", "staging_backend_data", "staging_redis"}
assert expected_volumes <= cfg["volumes"].keys()

backend = services["backend"]
backend_env = backend["environment"]
assert backend_env["APP_ENV"] == "production"
assert backend_env["BIND_ADDR"] == "0.0.0.0:8080"
assert backend_env["CORS_ALLOWED_ORIGINS"] == "https://dev.strovak.com"
assert backend_env["RUST_LOG"] == "info"
assert backend["build"]["context"] == str(root), backend["build"]["context"]
assert backend["build"]["dockerfile"] == "backend/Dockerfile"

frontend = services["frontend"]
assert frontend["environment"]["NODE_ENV"] == "production"
assert frontend["build"]["context"] == str(root / "frontend")
args = frontend["build"]["args"]
assert args["NEXT_PUBLIC_API_URL"] == "https://dev.api.strovak.com"
assert args["NEXT_PUBLIC_OVERLAY_WS_URL"] == "wss://dev.api.strovak.com"

for name, service in services.items():
    image = service.get("image", "")
    assert not image.endswith(":latest"), f"{name}: latest image forbidden"

caddy_mounts = services["caddy"]["volumes"]
sources = {v["source"] for v in caddy_mounts if v.get("type") == "bind"}
assert str(root / "infra/Caddyfile.staging") in sources, sources
assert not services["caddy"].get("extra_hosts"), services["caddy"].get("extra_hosts")
assert all("Rashand" not in str(mount) and "Rashtech" not in str(mount) for mount in caddy_mounts)
assert set(services["caddy"]["networks"]) == {"staging_edge"}

caddyfile = (root / "infra/Caddyfile.staging").read_text()
for host in ("dev.strovak.com", "dev.api.strovak.com", "ext-staging.strovak.com"):
    assert host in caddyfile
for header in ("Strict-Transport-Security", "X-Robots-Tag"):
    assert header in caddyfile

security_files = [
    root / "infra/compose.staging.yml",
    root / "infra/Caddyfile.staging",
    *(path for path in sorted((root / "infra").glob("staging-*.sh")) if path.name != "staging-validate.sh"),
]
all_text = "\n".join(path.read_text() for path in security_files)
assert "dev-root-token" not in all_text
assert "server -dev" not in all_text

common = (root / "infra/staging-common.sh").read_text()
deploy = (root / "infra/staging-deploy.sh").read_text()
backup = (root / "infra/staging-backup.sh").read_text()
restore = (root / "infra/staging-restore.sh").read_text()
rollback = (root / "infra/staging-rollback.sh").read_text()
bootstrap = (root / "infra/staging-host-bootstrap.sh").read_text()
health = (root / "infra/staging-health.sh").read_text()
status = (root / "infra/staging-status.sh").read_text()
docs = (root / "infra/staging.md").read_text()

assert "flock -n" in common
for operation in (deploy, backup, restore, rollback):
    assert "staging_acquire_operation_lock" in operation
assert "staging_validate_caddy_config" in common
assert "staging_validate_caddy_config \"$release\"" in deploy
assert "run --rm --no-deps caddy" not in deploy
assert "staging_set_symlink" in common
assert "old_current_link" in deploy and "old_previous_link" in deploy
assert "nginx_was_enabled" in deploy
assert "delete allow OpenSSH" in bootstrap
assert bootstrap.index('ufw allow "$port/tcp"') < bootstrap.index('delete allow OpenSSH')
assert "staging_non_loopback_forbidden_listeners" in common
assert "staging_non_loopback_forbidden_listeners" in status
assert "[ \"$code\" -ge 200 ]" not in health
assert "curl --fail" in health
assert "dev.api.strovak.com /health" in health
assert 'source "$script_dir/staging-common.sh"' in health
assert 'STAGING_PUBLIC_IPV4=([0-9]{1,3}\\.){3}[0-9]{1,3}' in common
assert "staging_require_build_disk_space" in common
assert "staging_require_build_disk_space \"$source_root\"" in deploy
assert "12 GiB" in docs
assert "--print-path" in backup
assert "mv -T \"$partial_dir\" \"$final_dir\"" in backup
assert ".predeploy-backup" in deploy
assert "STAGING_BACKUP_SKIP_BACKEND_RESTART" in backup
assert "STAGING_BACKUP_SKIP_BACKEND_RESTART=1" in deploy
assert "Active staging backend is unhealthy" in deploy
assert "CONFIRM_ROLLBACK_DATA" in rollback
assert "STAGING_RESTORE_SKIP_AUTOMATIC_BACKUP" in restore and "STAGING_RESTORE_START_RELEASE" in restore
assert "CONFIRM_ROLLBACK_DATA" in docs

retired_text = "\n".join(
    (
        (root / "infra/Caddyfile.staging").read_text(),
        (root / "infra/compose.staging.yml").read_text(),
        bootstrap,
        deploy,
        health,
        status,
        docs,
    )
).lower()
assert "rashand" not in retired_text
assert "rashtech" not in retired_text
PY

echo "Staging deployment validation passed."
