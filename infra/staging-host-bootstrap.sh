#!/usr/bin/env bash
set -euo pipefail

# One-time, idempotent host hardening. This script intentionally never prints
# generated secret values and refuses to touch SSH until a key-backed sudo
# operator has been verified.
if [ "$(id -u)" -ne 0 ]; then
  echo "Run with sudo: sudo $0" >&2
  exit 1
fi

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=infra/staging-common.sh
source "$script_dir/staging-common.sh"

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

for command in docker openssl ufw iptables sshd systemctl curl git tar flock python3 df ip; do
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
if [ -e "$STAGING_OPERATION_LOCK_FILE" ] && \
  { [ ! -f "$STAGING_OPERATION_LOCK_FILE" ] || [ -L "$STAGING_OPERATION_LOCK_FILE" ]; }; then
  echo "Staging operation lock is not a regular file: $STAGING_OPERATION_LOCK_FILE" >&2
  exit 1
fi
if [ ! -e "$STAGING_OPERATION_LOCK_FILE" ]; then
  install -o root -g root -m 0600 /dev/null "$STAGING_OPERATION_LOCK_FILE"
fi
chown root:root "$STAGING_OPERATION_LOCK_FILE"
chmod 0600 "$STAGING_OPERATION_LOCK_FILE"

env_file=/etc/streamdubber/staging.env
if [ ! -f "$env_file" ]; then
  env_tmp=$(mktemp /etc/streamdubber/.staging.env.XXXXXX)
  chmod 0600 "$env_tmp"
  postgres_password=$(openssl rand -hex 32)
  redis_password=$(openssl rand -hex 32)
  jwt_secret=$(openssl rand -base64 48 | tr -d '\n')
  secrets_enc_key=$(openssl rand -hex 32)
  staging_public_ipv4=$(ip -4 route get 1.1.1.1 2>/dev/null | awk '{for (i = 1; i <= NF; i++) if ($i == "src") {print $(i + 1); exit}}')
  if [ -z "$staging_public_ipv4" ]; then
    echo "Could not determine a public IPv4 address for STAGING_PUBLIC_IPV4." >&2
    rm -f "$env_tmp"
    exit 1
  fi
  {
    printf '%s\n' 'POSTGRES_DB=streamdubber_staging'
    printf '%s\n' 'POSTGRES_USER=streamdubber'
    printf 'POSTGRES_PASSWORD=%s\n' "$postgres_password"
    printf 'REDIS_PASSWORD=%s\n' "$redis_password"
    printf 'JWT_SECRET=%s\n' "$jwt_secret"
    printf 'SECRETS_ENC_KEY=%s\n' "$secrets_enc_key"
    printf 'STAGING_PUBLIC_IPV4=%s\n' "$staging_public_ipv4"
    printf '%s\n' 'ADMIN_EMAIL='
    printf '%s\n' 'ADMIN_PASSWORD_HASH='
    printf '%s\n' 'STAGING_BASIC_AUTH_USER='
    printf '%s\n' 'STAGING_BASIC_AUTH_HASH='
  } > "$env_tmp"
  unset postgres_password redis_password jwt_secret secrets_enc_key staging_public_ipv4
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
if ! grep -Eq '^STAGING_PUBLIC_IPV4=([0-9]{1,3}\.){3}[0-9]{1,3}$' "$env_file"; then
  echo "$env_file has an invalid STAGING_PUBLIC_IPV4 value." >&2
  exit 1
fi
if ! grep -qx 'ADMIN_PASSWORD_HASH=' "$env_file" && \
  ! grep -Eq "^ADMIN_PASSWORD_HASH='[$]argon2id[$][^']+'$" "$env_file"; then
  echo "ADMIN_PASSWORD_HASH must be empty or a single-quoted argon2id PHC value." >&2
  exit 1
fi

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

# Host input is SSH and HTTP(S) only. Remove any legacy mail ingress
# exceptions before applying the staging allowlist. Docker forwarding is
# handled separately below.
for port in 25 143 465 587 993; do
  ufw --force delete allow "$port/tcp" >/dev/null 2>&1 || true
done
# Preserve the effective SSH access path before removing the distro profile:
# on a host whose only SSH rule is `allow OpenSSH`, deletion must never create
# even a short lockout window.
for port in "${ssh_ports[@]}"; do
  ufw allow "$port/tcp"
done
# Ubuntu commonly installs `ufw allow OpenSSH` as an application-profile rule.
# Normalize it away before adding the exact effective SSH port(s): otherwise a
# profile can leave port 22 open after SSH moves, and the allowlist audit sees
# an unexpected `OpenSSH` rule on first bootstrap.
ufw --force delete allow OpenSSH >/dev/null 2>&1 || true
ufw default deny incoming
ufw default allow outgoing
ufw default deny routed
ufw allow 80/tcp
ufw allow 443/tcp
ufw allow 443/udp
# Older staging baselines created private Caddy-to-host allowances for retired
# services. Remove the exact rules when the Docker bridge is available.
docker_gateway=$(docker network inspect bridge --format '{{(index .IPAM.Config 0).Gateway}}' 2>/dev/null || true)
if [ -n "$docker_gateway" ]; then
  for port in 18000 3000; do
    ufw --force delete allow from 172.30.10.2 to "$docker_gateway" port "$port" proto tcp >/dev/null 2>&1 || true
  done
fi
ufw --force enable

unexpected_ufw=$(ufw status | awk '/ALLOW( IN)?/ {print $1}' | \
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
