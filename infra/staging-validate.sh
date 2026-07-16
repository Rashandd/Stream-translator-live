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
)

for script in "${scripts[@]}"; do
  bash -n "$root/infra/$script"
done

cat > "$tmp_dir/staging.env" <<'EOF'
POSTGRES_DB=streamdubber_staging
POSTGRES_USER=streamdubber
POSTGRES_PASSWORD=test-postgres-password
REDIS_PASSWORD=test-redis-password
JWT_SECRET=test-jwt-secret-with-at-least-thirty-two-bytes
SECRETS_ENC_KEY=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
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

published = {(p.get("published"), p.get("protocol")) for p in services["caddy"]["ports"]}
assert published == {("80", "tcp"), ("443", "tcp"), ("443", "udp")}, published

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
assert services["caddy"]["networks"]["staging_edge"]["ipv4_address"] == "172.30.10.2"

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
PY

echo "Staging deployment validation passed."
