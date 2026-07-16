# StreamDubber staging operations

This deployment runs the application in production mode while keeping every
database, token, volume, network, and credential isolated from production.

## Endpoints

| Surface | URL |
| --- | --- |
| Frontend | `https://dev.strovak.com` |
| API and WebSocket | `https://dev.api.strovak.com` / `wss://dev.api.strovak.com` |
| Twitch Extension EBS | `https://ext-staging.strovak.com` |

Caddy is the only staging container with published host ports. PostgreSQL and
Redis are attached only to `streamdubber_staging_internal`; backend and
frontend are reachable only through `streamdubber_staging_edge`. OpenClaw stays
on host loopback and is not routed by Caddy.

The host already had two unrelated websites on Nginx. The Caddy cutover keeps
their static data mounted read-only and proxies only their existing app ports.
The loopback-only Rashand API is exposed to Caddy by a narrow systemd socket on
Docker's private bridge. Nginx is disabled only after all old and new sites
pass direct-origin health checks.

## First deployment

From the isolated staging worktree, with all changes committed:

```bash
sudo infra/staging-host-bootstrap.sh
sudo infra/staging-deploy.sh
sudo /srv/streamdubber/current/infra/staging-status.sh
```

The bootstrap creates the `streamdubber` system user, private directories,
firewall units, backup timer, and `/etc/streamdubber/staging.env`. Generated
values are never printed. The env file is root-owned, mode `0600`, and must not
be copied into the repository.

Before admin login can work, an operator must add these two values to that
file:

```dotenv
ADMIN_EMAIL=operator-staging-address
ADMIN_PASSWORD_HASH='$argon2id$single-quoted-phc-value'
```

Generate the hash without putting the password on a command line:

```bash
cargo run --bin hash_password
```

Keep the hash inside literal single quotes in `staging.env`; otherwise Compose
interprets the PHC dollar signs as variable substitutions. The deploy script
rejects an unquoted hash instead of silently starting with a broken login.

After editing the env file, apply it with `sudo infra/staging-deploy.sh` (same
commit deploys idempotently and force-recreates the backend when needed).

## OAuth console configuration

Create staging-only applications. Never copy a production client secret,
refresh token, extension secret, or user database into this environment.

| Purpose | Managed secret names | Exact callback URL |
| --- | --- | --- |
| Google login/link | `GOOGLE_LOGIN_CLIENT_ID`, `GOOGLE_LOGIN_CLIENT_SECRET` | `https://dev.api.strovak.com/api/auth/oauth/google/callback` |
| Twitch login/link | `TWITCH_CLIENT_ID`, `TWITCH_CLIENT_SECRET` | `https://dev.api.strovak.com/api/auth/oauth/twitch/callback` |
| YouTube captions connect | `YOUTUBE_CLIENT_ID`, `YOUTUBE_CLIENT_SECRET` | `https://dev.api.strovak.com/api/integrations/youtube/callback` |
| Streamlabs connect | `STREAMLABS_CLIENT_ID`, `STREAMLABS_CLIENT_SECRET` | `https://dev.api.strovak.com/api/integrations/streamlabs/callback` |
| Twitch channel connect | `TWITCH_CONNECT_CLIENT_ID`, `TWITCH_CONNECT_CLIENT_SECRET` | `https://dev.api.strovak.com/api/integrations/twitch/callback` |

Enter those credentials through the staging Admin > Secrets page. They are
stored in `/data/secrets.enc` inside `staging_backend_data`, encrypted with the
staging-only `SECRETS_ENC_KEY`. OAuth tokens are stored separately in
`/data/tokens.enc` in the same isolated volume.

For Twitch Extension Hosted Test, configure the EBS/allowlist host as
`https://ext-staging.strovak.com`, upload the generated staging extension ZIP,
and enter the console's base64 shared secret as `TWITCH_EXT_SECRET`. Do not use
the production extension client or `TWITCH_EXT_SECRET`.

## Routine operations

```bash
# Deploy/update the committed revision
sudo infra/staging-deploy.sh

# Container, TLS, WSS, firewall, and public checks
sudo /srv/streamdubber/current/infra/staging-status.sh

# Follow all logs, or name one or more services
sudo /srv/streamdubber/current/infra/staging-logs.sh backend caddy

# Consistent PostgreSQL + encrypted file store + Redis backup
sudo /srv/streamdubber/current/infra/staging-backup.sh

# Validation-only restore (default)
sudo /srv/streamdubber/current/infra/staging-restore.sh \
  /srv/streamdubber/backups/YYYYMMDDTHHMMSSZ

# Destructive restore after the dry-run succeeds
CONFIRM_RESTORE=staging sudo --preserve-env=CONFIRM_RESTORE \
  /srv/streamdubber/current/infra/staging-restore.sh \
  /srv/streamdubber/backups/YYYYMMDDTHHMMSSZ

# Swap to the prior immutable release; failed health auto-reverts
sudo /srv/streamdubber/current/infra/staging-rollback.sh
```

The daily backup timer is `streamdubber-staging-backup.timer`. Backups are
root-readable under `/srv/streamdubber/backups` and retained for 14 days. The
runtime encryption keys in `/etc/streamdubber/staging.env` are deliberately not
included; escrow that file through a separate secret-management channel.

## Host security invariants

- SSH password and direct root login are disabled only after a non-root sudo
  account with an authorized key passes validation.
- UFW admits only the effective SSH port plus TCP 80/443 and UDP 443.
- Two source-restricted UFW rules let only Caddy's fixed private address
  (`172.30.10.2`) reach the preserved host apps on 18000/3000; neither port is
  public.
- A dedicated `DOCKER-USER` sub-chain uses original destination ports so
  Docker cannot bypass that policy with a remapped published port.
- Existing mail container data is not removed, but its public port exceptions
  are closed to satisfy the staging host's three-port ingress policy.
- Ports 3000, 8080, 5432, 6379, and 8200 are never published by this compose
  project. Existing host applications may bind loopback or a wildcard address;
  UFW still blocks non-allowlisted public ingress.
