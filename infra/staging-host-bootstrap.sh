#!/usr/bin/env bash
set -euo pipefail

# One-time, idempotent host hardening. This script intentionally never prints
# generated secret values and refuses to touch SSH until a key-backed sudo
# operator has been verified.
if [ "$(id -u)" -ne 0 ]; then
  echo "Run with sudo: sudo $0" >&2
  exit 1
fi

operator=${SUDO_USER:-}
if [ -z "$operator" ] || [ "$operator" = root ]; then
  echo "Invoke this script through sudo from the non-root operator account." >&2
  exit 1
fi

operator_entry=$(getent passwd "$operator")
operator_home=$(printf '%s\n' "$operator_entry" | cut -d: -f6)
if [ -z "$operator_entry" ] || [ -z "$operator_home" ]; then
  echo "Could not resolve the sudo operator account." >&2
  exit 1
fi
if ! id -nG "$operator" | tr ' ' '\n' | grep -qx sudo; then
  echo "The operator account is not in the sudo group." >&2
  exit 1
fi
if ! sudo -u "$operator" sudo -n true; then
  echo "The operator account cannot complete a non-interactive sudo check." >&2
  exit 1
fi
if [ ! -s "$operator_home/.ssh/authorized_keys" ]; then
  echo "The operator account has no authorized SSH key; refusing SSH hardening." >&2
  exit 1
fi

# OpenClaw must remain loopback-only. Abort before changing anything if its
# known gateway port is bound to a public/wildcard address.
if ss -H -lntp | awk '$4 ~ /:18789$/ {print $4}' | grep -Evq '^(127\.0\.0\.1|\[::1\]|::1):18789$'; then
  echo "OpenClaw port 18789 is not loopback-only; refusing deployment." >&2
  exit 1
fi

for command in docker openssl ufw iptables sshd systemctl curl git tar; do
  command -v "$command" >/dev/null || {
    echo "Required host command is missing: $command" >&2
    exit 1
  }
done

id streamdubber >/dev/null 2>&1 || \
  useradd --system --create-home --home-dir /srv/streamdubber --shell /usr/sbin/nologin streamdubber

chown streamdubber:streamdubber /srv/streamdubber
chmod 0750 /srv/streamdubber
install -d -o root -g streamdubber -m 0750 /etc/streamdubber
install -d -o streamdubber -g streamdubber -m 0750 /srv/streamdubber/releases
install -d -o streamdubber -g streamdubber -m 0750 /srv/streamdubber/backups

env_file=/etc/streamdubber/staging.env
if [ ! -f "$env_file" ]; then
  env_tmp=$(mktemp /etc/streamdubber/.staging.env.XXXXXX)
  chmod 0600 "$env_tmp"
  postgres_password=$(openssl rand -hex 32)
  redis_password=$(openssl rand -hex 32)
  jwt_secret=$(openssl rand -base64 48 | tr -d '\n')
  secrets_enc_key=$(openssl rand -hex 32)
  {
    printf '%s\n' 'POSTGRES_DB=streamdubber_staging'
    printf '%s\n' 'POSTGRES_USER=streamdubber'
    printf 'POSTGRES_PASSWORD=%s\n' "$postgres_password"
    printf 'REDIS_PASSWORD=%s\n' "$redis_password"
    printf 'JWT_SECRET=%s\n' "$jwt_secret"
    printf 'SECRETS_ENC_KEY=%s\n' "$secrets_enc_key"
    printf '%s\n' 'ADMIN_EMAIL='
    printf '%s\n' 'ADMIN_PASSWORD_HASH='
  } > "$env_tmp"
  unset postgres_password redis_password jwt_secret secrets_enc_key
  chown root:streamdubber "$env_tmp"
  mv -f "$env_tmp" "$env_file"
fi
chown root:streamdubber "$env_file"
chmod 0600 "$env_file"

for key in POSTGRES_PASSWORD REDIS_PASSWORD JWT_SECRET SECRETS_ENC_KEY; do
  if ! grep -Eq "^${key}=.+" "$env_file"; then
    echo "$env_file is missing a non-empty $key value." >&2
    exit 1
  fi
done
if ! grep -Eq '^SECRETS_ENC_KEY=[[:xdigit:]]{64}$' "$env_file"; then
  echo "$env_file has an invalid SECRETS_ENC_KEY format." >&2
  exit 1
fi
if ! grep -qx 'ADMIN_PASSWORD_HASH=' "$env_file" && \
  ! grep -Eq "^ADMIN_PASSWORD_HASH='[$]argon2id[$][^']+'$" "$env_file"; then
  echo "ADMIN_PASSWORD_HASH must be empty or a single-quoted argon2id PHC value." >&2
  exit 1
fi

# Preserve the existing rashand-tech.com API while Caddy takes the public edge.
# The proxy listens only on Docker's private bridge and can reach only the
# existing loopback FastAPI port; no StreamDubber container gets host networking.
docker_gateway=$(docker network inspect bridge --format '{{(index .IPAM.Config 0).Gateway}}')
if [ -z "$docker_gateway" ]; then
  echo "Could not determine Docker's private bridge gateway." >&2
  exit 1
fi
if ! ss -H -lnt | awk '$4 ~ /127\.0\.0\.1:8000$/ {found=1} END {exit !found}'; then
  echo "Existing rashand API is not listening on 127.0.0.1:8000." >&2
  exit 1
fi
test -x /usr/lib/systemd/systemd-socket-proxyd

cat > /etc/systemd/system/rashand-api-bridge.socket <<EOF
[Unit]
Description=Private Docker bridge socket for the existing Rashand API
After=docker.service
Requires=docker.service

[Socket]
ListenStream=${docker_gateway}:18000
Accept=no
FreeBind=yes

[Install]
WantedBy=sockets.target
EOF

cat > /etc/systemd/system/rashand-api-bridge.service <<'EOF'
[Unit]
Description=Forward the private Docker bridge socket to the existing loopback Rashand API
Requires=rashand-api-bridge.socket

[Service]
ExecStart=/usr/lib/systemd/systemd-socket-proxyd 127.0.0.1:8000
NoNewPrivileges=yes
PrivateDevices=yes
PrivateTmp=yes
ProtectControlGroups=yes
ProtectHome=yes
ProtectKernelLogs=yes
ProtectKernelModules=yes
ProtectKernelTunables=yes
ProtectSystem=strict
RestrictAddressFamilies=AF_INET AF_INET6
RestrictSUIDSGID=yes
EOF

systemctl daemon-reload
systemctl enable --now rashand-api-bridge.socket
curl --fail-with-body --silent --output /dev/null --max-time 5 \
  "http://${docker_gateway}:18000/" || {
    # A 4xx response still proves the bridge is connected; only curl transport
    # failures are fatal. Retry without --fail for that distinction.
    curl --silent --output /dev/null --max-time 5 "http://${docker_gateway}:18000/"
  }

# The operator/key check above is deliberately completed before root SSH is
# disabled. Validate both syntax and effective values before reloading sshd.
rm -f /etc/ssh/sshd_config.d/99-streamdubber-hardening.conf
cat > /etc/ssh/sshd_config.d/00-streamdubber-hardening.conf <<'EOF'
PasswordAuthentication no
KbdInteractiveAuthentication no
PermitRootLogin no
PubkeyAuthentication yes
EOF
sshd -t
effective_sshd=$(sshd -T)
for expected in \
  'passwordauthentication no' \
  'kbdinteractiveauthentication no' \
  'permitrootlogin no' \
  'pubkeyauthentication yes'; do
  grep -qx "$expected" <<<"$effective_sshd" || {
    echo "Effective sshd configuration did not apply: $expected" >&2
    exit 1
  }
done
systemctl reload ssh

mapfile -t ssh_ports < <(awk '$1 == "port" {print $2}' <<<"$effective_sshd" | sort -nu)
if [ "${#ssh_ports[@]}" -eq 0 ]; then
  echo "Could not determine the effective SSH port." >&2
  exit 1
fi

# Host input: SSH and HTTP(S) only. Existing mail containers/data are not
# removed, but their public exceptions are closed as required for this staging
# host. Docker forwarding is handled separately below.
for port in 25 143 465 587 993; do
  ufw --force delete allow "$port/tcp" >/dev/null 2>&1 || true
done
ufw default deny incoming
ufw default allow outgoing
ufw default deny routed
for port in "${ssh_ports[@]}"; do
  ufw allow "$port/tcp"
done
ufw allow 80/tcp
ufw allow 443/tcp
ufw allow 443/udp
# Only Caddy's fixed private edge subnet may reach the two preserved host
# applications. These are not public ingress exceptions.
ufw allow from 172.30.10.2 to "$docker_gateway" port 18000 proto tcp
ufw allow from 172.30.10.2 to "$docker_gateway" port 3000 proto tcp
ufw --force enable

unexpected_ufw=$(ufw status | awk \
  '/ALLOW IN/ && $0 ~ /172\.30\.10\.2/ && ($0 ~ /18000/ || $0 ~ /3000/) {next}
   /ALLOW IN/ {print $1}' | \
  grep -Ev "^($(IFS='|'; echo "${ssh_ports[*]}")|80|443)(/tcp|/udp)?$" || true)
if [ -n "$unexpected_ufw" ]; then
  echo "Unexpected UFW allow rules remain:" >&2
  printf '%s\n' "$unexpected_ufw" >&2
  exit 1
fi

# Docker publishes before UFW sees a packet. Own one narrow sub-chain and use
# conntrack's ORIGINAL destination port so a future 8080:80 mapping cannot
# accidentally pass the allowlist. ESTABLISHED/RELATED must come first or
# container DNS/ACME/OAuth response traffic is broken.
cat > /usr/local/sbin/streamdubber-docker-firewall <<'EOF'
#!/bin/sh
set -eu

interface=$(ip route show default | awk '/default/ {print $5; exit}')
[ -n "$interface" ]

apply_rules() {
  binary=$1
  chain=STREAMDUBBER-STAGING
  if ! "$binary" -nL DOCKER-USER >/dev/null 2>&1; then
    return 0
  fi
  "$binary" -N "$chain" 2>/dev/null || true
  "$binary" -F "$chain"
  "$binary" -A "$chain" -m conntrack --ctstate RELATED,ESTABLISHED -j RETURN
  "$binary" -A "$chain" -p tcp -m conntrack --ctstate NEW --ctorigdstport 80 -j RETURN
  "$binary" -A "$chain" -p tcp -m conntrack --ctstate NEW --ctorigdstport 443 -j RETURN
  "$binary" -A "$chain" -p udp -m conntrack --ctstate NEW --ctorigdstport 443 -j RETURN
  "$binary" -A "$chain" -j DROP
  "$binary" -C DOCKER-USER -i "$interface" -j "$chain" 2>/dev/null || \
    "$binary" -I DOCKER-USER 1 -i "$interface" -j "$chain"
}

apply_rules iptables
command -v ip6tables >/dev/null 2>&1 && apply_rules ip6tables
EOF
chmod 0750 /usr/local/sbin/streamdubber-docker-firewall

cat > /etc/systemd/system/streamdubber-docker-firewall.service <<'EOF'
[Unit]
Description=Restrict Docker-published ingress to HTTP(S)
After=docker.service
Requires=docker.service
PartOf=docker.service

[Service]
Type=oneshot
ExecStart=/usr/local/sbin/streamdubber-docker-firewall
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
EOF

install -d -m 0755 /etc/systemd/system/docker.service.d
cat > /etc/systemd/system/docker.service.d/streamdubber-firewall.conf <<'EOF'
[Unit]
Wants=streamdubber-docker-firewall.service
EOF

# Daily backups use the currently activated immutable release. The timer is
# harmless before the first deploy because the service has a path condition.
cat > /etc/systemd/system/streamdubber-staging-backup.service <<'EOF'
[Unit]
Description=Back up StreamDubber staging data
ConditionPathExists=/srv/streamdubber/current/infra/staging-backup.sh

[Service]
Type=oneshot
ExecStart=/srv/streamdubber/current/infra/staging-backup.sh
Nice=10
IOSchedulingClass=best-effort
IOSchedulingPriority=7
EOF

cat > /etc/systemd/system/streamdubber-staging-backup.timer <<'EOF'
[Unit]
Description=Daily StreamDubber staging backup

[Timer]
OnCalendar=*-*-* 02:15:00 UTC
Persistent=true
RandomizedDelaySec=15m

[Install]
WantedBy=timers.target
EOF

systemctl daemon-reload
systemctl enable --now streamdubber-docker-firewall.service
systemctl enable --now streamdubber-staging-backup.timer

echo "Staging host baseline is ready; generated secrets remain only in $env_file."
