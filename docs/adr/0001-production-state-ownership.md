# 0001 — Production State Ownership (BH-03)

- **Status:** Accepted
- **Date:** 2026-07-13
- **Deciders:** Backend hardening pass (BH-03 / BH-04)
- **Related:** `docs/backend-hardening-captions-plan.md` (BH-03/BH-04/BH-08/BH-09),
  `docs/mutable-tumbling-plan.md` (scoped first pass), `docs/schema.md`,
  `docs/security.md`

> This is the repository's first ADR, so there is no prior template to match. It
> establishes the plain **Context / Decision / Consequences** structure for the
> ADRs that follow.

## Context

StreamDubber persists domain state across **three different backends today**, and
which one is authoritative depends on the store:

1. **PostgreSQL** (`DATABASE_URL`) — optional. When unset the backend still runs
   fully on file/memory stores (`docs` calls this the "best-effort mirror" mode).
   Migrations `0001`–`0007` already define most tables.
2. **In-memory** (`RwLock`/`Mutex<HashMap>`) — the runtime authority for several
   stores; some are seeded from code defaults, some are caches over Postgres, some
   are purely ephemeral and lost on restart.
3. **Encrypted local files** (`*_FILE` env vars, AES-256-GCM via `crypto::Cipher`)
   — the durable source of truth for account/secret-bearing stores that have not
   yet cut over to Postgres.

This split is workable for a single instance but blocks two goals from
`docs/backend-hardening-captions-plan.md`:

- **BH-03** — make PostgreSQL the single durable source of truth for persistent
  *domain* state (accounts, usage, relay targets, settings, …). Today that state
  is scattered: some tables are authoritative (`browser_sessions`), some are
  write-only mirrors that are never read back (`users`, `usage`), some tables
  exist but are entirely unused (`platform_accounts`), and several domains have no
  table at all (relay targets, settings, founding apps).
- **BH-04** — move *ephemeral, shared* state to Redis so the backend can run as
  more than one instance. Today every rate limiter, CSRF/pairing/2FA code table,
  SDK command queue, and overlay fan-out channel lives in one process's memory, so
  a second instance would silently weaken rate limits and break overlay delivery.

Both BH-03 and BH-04 are far too large for one pass (BH-03 alone = this ADR + 8
new migrations + cutover of 11 domains + a legacy file→DB importer; BH-04 = 8
categories of ephemeral state). This ADR is the mandated **first step** of BH-03:
it records who owns each store today, who *should* own it, and the failure policy
for each — so the incremental cutovers that follow have a single reference to
check against. The only code landing alongside this ADR in the same pass is the
BH-03A fail-closed `DATABASE_URL` check and the BH-04 Redis-backed rate limiters
(the first BH-04 category); everything else in the tables below is **deferred** to
scoped follow-up tasks.

### Terminology used in the tables

- **Owner (veri sahibi)** — the *runtime authority* today, and (in Migration
  cutover) the intended durable owner. Three read/write shapes recur:
  - *Direct Postgres* — every read/write hits the DB; no in-memory copy in the
    hot path (`browser_sessions`).
  - *Read-cache over Postgres* — reads served from an in-memory cache seeded from
    code defaults and overlaid from the DB at boot; **writes are DB-first** (the
    DB write must succeed before the cache is updated), so the cache can never
    hold an un-persisted admin change (`service_config`, `plans`).
  - *Memory/file-authoritative + best-effort DB mirror* — memory (or an encrypted
    file) is authoritative; the DB is written best-effort and, for `users`/`usage`,
    never read back (`channel_bindings`, `users`, `usage`).
- **Durability** — survives a process restart?
- **Failure policy** — behaviour when the backing store errors. The two precedents
  this ADR standardises on are spelled out under Decision.

## Decision

### 1. Three ownership tiers

- **PostgreSQL owns durable domain state.** Every persistent domain entity's
  target owner is Postgres, authoritative when `DATABASE_URL` is configured. The
  in-memory read-caches for `service_config`/`plans` stay (they keep hot reads
  off the DB and provide code-default fallback), but they remain DB-first on
  write and must never be the *durable* owner.
- **Redis owns ephemeral, shared state.** Rate-limit counters, CSRF/pairing/2FA
  codes, the SDK command queue, and overlay presence/fan-out target Redis so
  multiple instances share one view. Keys use the `streamdubber:{env}:{parts…}`
  namespace (env segment from `Environment::as_label()`), per the parent plan's
  key format.
- **Encrypted local files stay local until each domain's scoped cutover.** Files
  remain the source of truth for the not-yet-migrated stores below, and one
  payload stays on disk permanently by design: the **raw voice sample**
  (`VOICE_SAMPLES_FILE`, AES-256-GCM) — only its metadata and a pointer move to
  Postgres. Secrets (`SECRETS_FILE`) and OAuth/stream-key blobs stay AES-256-GCM
  at rest; TLS 1.3 covers transit, so no second encryption layer is added
  (`docs/security.md`).

### 2. Fail-closed policies decided here

- **BH-03A — `DATABASE_URL` required in production.** When
  `Environment::from_env() == Production` and `DATABASE_URL` is absent, the
  process refuses to start rather than silently running file-only. Implemented in
  this same pass (`main.rs::enforce_production_database_requirement`), reusing the
  existing typed `Environment` enum (BH-02) — no parallel raw `APP_ENV` string
  reader is introduced.
- **`REDIS_URL` — optional this pass, present-but-unreachable is fatal.** Mirrors
  `DATABASE_URL`'s current status: absent ⇒ in-memory fallback (single-instance,
  a scaling limit, not a security regression — it is already today's behaviour
  everywhere). A configured-but-unreachable `REDIS_URL` is a hard startup failure
  (`cache::connect`'s `?`), identical to how `db::connect` already treats a
  misconfigured `DATABASE_URL`. Making `REDIS_URL` **mandatory in production** is
  deferred to **BH-08** (it needs further typed-environment modelling that BH-08
  owns).
- **Rate limiting is a security control → fail-closed at runtime.** Whenever Redis
  is configured, any error from a rate-limit check (timeout, refused, script
  error) returns **HTTP 503**, never a silent allow. This follows the precedent
  set by `SessionStore::rotate()` (a Postgres error during refresh → **401**, not
  a pass-through), **not** the permissive `ChannelBindingStore` precedent (memory
  is authoritative, DB write is best-effort/`warn`). A rate limiter is a security
  control, not a durability cache, so the stricter precedent applies.

### 3. State ownership table

Grouped by the tier each store belongs to (or is targeted at). "Migration
cutover" states what is already done vs. what is deferred and to which BH step.

#### 3.1 Already Postgres-owned (authoritative when `DATABASE_URL` is set)

| Store | Owner (veri sahibi) | Durability | TTL | Cache invalidation | Encryption | Migration cutover | Failure policy |
|---|---|---|---|---|---|---|---|
| `browser_sessions` (`auth/session.rs`) | **Direct Postgres** when configured; in-memory only for tests/local dev | Durable w/ DB (else lost on restart) | Refresh 30 d, access JWT 15 min; `expires_at` per row (no auto-reaper) | No cache — single authority; replaying a rotated token revokes the whole `family_id` | None needed — stores only SHA-256 `token_hash`; raw refresh token never persisted | **Done** (mig. `0005`) | **Fail-closed**: storage error → 401 (`refresh` clears cookies); no silent allow |
| `service_config` (`config.rs`) | Read-cache over Postgres; seeded from code `with_defaults()` | Durable overrides w/ DB; code defaults always present | None | **DB-first**: `PUT /admin/config` persists first, cache (`set_existing`, whitelisted) updated only on success; boot `load_from_db`→`apply_db_rows` overlays | None (non-secret provider toggles/URLs; `legal.*` HTML sanitised on read) | **Done** (mig. `0001` seed + persist/load) | Admin write fail-closed (DB error → 500, cache unchanged); boot-load error → warn + seed defaults (degrade); reads never touch DB |
| `plans` (`plans/mod.rs`) | Read-cache over Postgres; seeded from code (5 plans) | Durable overrides w/ DB; defaults always present | None | **DB-first**: `PUT /admin/plans` persists first, `set` only on success; boot `load_from_db`→`apply_db_rows` | None (pricing/limits are public) | **Done** (mig. `0001`) | Same DB-first rule as `service_config` (500 on DB error, no divergence) |
| `channel_bindings` (`bindings/mod.rs`) | **Memory-authoritative** (`RwLock<HashMap>`, read on pipeline hot path); Postgres is a durable mirror loaded at boot | Durable w/ DB (mirror reloaded at boot); memory-only otherwise | None (rows removed on unbind) | **Memory-first**: `bind`/`unbind` mutate memory; `persist_binding`/`delete_*` upsert best-effort; boot `load_bindings`→`store.load` | None (platform channel IDs are public) | **Done** (mig. `0002`) | **Best-effort**: DB write/load error → warn, memory stays authoritative, flow continues (the permissive precedent BH-04 does *not* reuse) |

#### 3.2 One-way mirror today → target Postgres (BH-03B; deferred)

Written to Postgres but **never read back**; restart recovery is from the file
(`users`) or file (`usage`), not the DB. Cutover = add a DB read-back/reconcile
path, then a legacy file→DB importer.

| Store | Owner (veri sahibi) | Durability | TTL | Cache invalidation | Encryption | Migration cutover | Failure policy |
|---|---|---|---|---|---|---|---|
| `users` (`users/mod.rs` → `users`, `user_oauth_links`, `user_consents`) | In-memory source-of-truth + `USERS_FILE` snapshot; Postgres = write-only mirror | Durable via encrypted file; DB mirror not authoritative | None (accounts persist) | Memory↔file on write; `upsert_user`/link/consent best-effort *after* file write; **no DB→memory load path yet** | `USERS_FILE` AES-256-GCM (whole snapshot); DB `totp_secret_encrypted` AES-256-GCM; password = argon2 hash | **Deferred (BH-03B)**: add read-back + reconcile, then file→DB importer | Best-effort mirror — DB error → warn; file authoritative (consent module: "Postgres yalnız best-effort ayna") |
| `usage` (`usage/mod.rs` → `usage_records`) | In-memory HashMap runtime authority + `USAGE_FILE` snapshot; Postgres = fire-and-forget mirror | Durable via file (reloaded + compacted at boot, last line per user+period wins) | Period-keyed `YYYY-MM` (UTC); counter self-resets on month change; no row expiry | Memory first; file append on real increments; DB upsert at each full-minute boundary via `tokio::spawn`; **no DB→memory load** | None (usage seconds are non-secret; plain JSONL) | **Deferred (BH-03B)**: add read-back/reconcile | Best-effort — file append warn; DB upsert warn (incl. FK violation when `users` row absent); never blocks the pipeline |

#### 3.3 File/memory only today → target Postgres (BH-03C domains 4–11; deferred)

None of these read or write Postgres today. Most need a **new migration** (no
table exists); `platform_accounts` is the exception — its table exists (mig.
`0001`) but is entirely unused.

| Store | Owner (veri sahibi) | Durability | TTL | Cache invalidation | Encryption | Migration cutover | Failure policy |
|---|---|---|---|---|---|---|---|
| SDK sessions / presence (`sdk_session/mod.rs`, `SDK_SESSIONS_FILE`) | In-memory + encrypted file; pairing also best-effort-writes the existing `sdk_sessions` table | Durable via encrypted file; `last_seen_at` presence recomputed after restart | No hard TTL; "connected" = `last_seen_at` within `CONNECTED_WINDOW_SECS`; device JWT 30 d | Signed heartbeat updates memory + file; DB row only via pairing (best-effort) | `SDK_SESSIONS_FILE` AES-256-GCM (32-byte `heartbeat_secret` stored encrypted) | **Deferred (BH-03C)**: make `sdk_sessions` authoritative | File/DB best-effort (warn); heartbeat replay-protected (HMAC + time tolerance) |
| Relay targets (`relay/mod.rs`, `RELAY_FILE`) | In-memory + encrypted file (single blob) | Durable via encrypted file | None | Memory ↔ encrypted file on write | AES-256-GCM (RTMP stream keys — plaintext never on disk) | **Deferred (BH-03C)**: needs a new migration | File best-effort |
| Platform / OAuth accounts (`integrations/token_store.rs` `TokenVault`, `TOKENS_FILE`) | In-memory + encrypted file; **`platform_accounts` table exists but is unused** | Durable via encrypted file | Per-token `expires_at` (from `expires_in`); caller checks; refreshable | Memory ↔ encrypted file on write | AES-256-GCM (access/refresh tokens) | **Deferred (BH-03C)**: cut over to `platform_accounts`, bundled with the `users` FK (module doc) | File best-effort |
| Stream / STT / TTS settings (`stream_settings/mod.rs`, `sdk_link/mod.rs` STT+TTS stores) | In-memory + plain JSON files (`STREAM_SETTINGS_FILE`, `STT_SETTINGS_FILE`, `TTS_SETTINGS_FILE`) | Durable via files | None | Memory ↔ file on write | None (language codes, mode prefs, benchmark results — non-secret) | **Deferred (BH-03C)**: new migrations | File best-effort |
| Founding applications (`founding/mod.rs`, `FOUNDING_APPS_FILE`) | In-memory + plain JSON file | Durable via file | None | Memory ↔ file on write; `is_approved` read live per StreamStart (`effective_plan`) | None (application metadata + approval flag) | **Deferred (BH-03C)**: new migration | File best-effort |
| Voice model metadata + sample **reference** (`voice_models/mod.rs`, `voice_models/sample_store.rs`) | Metadata in-memory + plain JSON (`VOICE_MODELS_FILE`); **raw bytes** in `VOICE_SAMPLES_FILE` | Durable via files | None | Memory ↔ files on write | Metadata none; **raw sample AES-256-GCM** | **Deferred (BH-03C)**: metadata + pointer → Postgres; **raw bytes stay in the encrypted file by design** | File best-effort (sample persist failure → warn); model starts `pending_review` (admin gate) |
| STT server metadata (`stt_servers/mod.rs`, `STT_SERVERS_FILE`) | In-memory + encrypted file (per-field secret encryption) | Durable via encrypted file | None | Memory ↔ encrypted file on write | AES-256-GCM on `ssh_private_key_encrypted` + `api_token_encrypted` (rest plain in blob) | **Deferred / revisit (BH-03C)**: the module doc argues **no new migration is needed** (admin-managed resource, lighter file pattern) — cutover here is an open decision, not automatic | File best-effort |
| Overlay tokens (durable table) | **Not implemented** — no durable table today; runtime overlay tokens are ephemeral (see §3.4 `active_streams`/`overlay_hub`) | n/a | n/a | n/a | n/a | **Deferred (BH-03C, Region D1 captions)**: new `overlay_tokens` table, not created this pass | n/a |

#### 3.4 Ephemeral → target Redis (BH-04; only rate limiters land this pass)

All are per-instance `Mutex<HashMap>`/broadcast state today, so a second instance
diverges. Redis keys: `streamdubber:{env}:…`.

| Store | Owner (veri sahibi) | Durability | TTL | Cache invalidation | Encryption | Migration cutover | Failure policy |
|---|---|---|---|---|---|---|---|
| Rate limiters (`rate_limit.rs` `RateLimiter`/`AuthLimiter`, `pairing::ClaimLimiter`) | In-memory fixed-window counters, per-instance | Ephemeral (lost on restart; acceptable — 60 s window) | 60 s window; entries pruned when map > 4096 or lazily on window elapse. Redis: atomic `INCR` + `EXPIRE(window)` via one Lua script | Per-instance today → **shared counter across instances via Redis** (this pass) | None (IP keys) | **This pass (BH-04)**: Redis backend + `with_redis` | Today lock-poison → deny. Redis error → **503** (never silent allow); absent `REDIS_URL` → in-memory fallback (single-instance) |
| OAuth CSRF state (`auth/oauth_login.rs` `LoginStateStore`) | In-memory single-use map | Ephemeral | 600 s (10 min); consumed once | Per-instance → sticky routing to the issuing instance needed until Redis | None (opaque UUID `state`) | **Deferred (BH-04)** | Unknown/expired → callback rejected |
| Pairing codes (`pairing/mod.rs` `PairingStore`) | In-memory single-use; also best-effort DB write to `sdk_sessions` on claim | Ephemeral (restart drops pending; panel regenerates) | 600 s (10 min); one active code per user | Per-instance (claim must hit the instance that issued the code) | None (6-digit numeric) | **Deferred (BH-04)** | Unknown/expired → error; brute-force bounded by `ClaimLimiter` |
| 2FA / reset codes (`users/mod.rs` `TwoFactorStore`) | In-memory `(email, purpose)` map, single-use | Ephemeral (restart drops → user re-requests) | `CODE_TTL`; reissue throttle (`TooSoon`); `MAX_CODE_ATTEMPTS` then invalidate; constant-time compare | Per-instance (verify must hit the issuing instance) | None (short-lived numeric codes) | **Deferred (BH-04)** | Invalid/expired/too-many → error; lock poison → internal error |
| SDK command queue (`sdk_link/mod.rs` `SdkCommandQueue`) | In-memory, one-time delivery | Ephemeral (restart drops pending; panel re-triggers) | None explicit; consumed on first poll | Per-instance (SDK poll must reach the instance the panel enqueued on) | None | **Deferred (BH-04)** | At-most-once delivery; lost command re-issued by the panel |
| Active-stream presence (`overlay/mod.rs` `ActiveStreams`) | In-memory `user_id → overlay_token` map | Ephemeral (cleared on stream stop/disconnect) | None; last-writer-wins; `clear_if` guards stale cleanup | Per-instance (EBS resolve only sees streams on the same instance) | None (token is a server-side capability, never sent to the viewer) | **Deferred (BH-04)** | Absent → `None` (viewer sees "not live") |
| Overlay snapshot + pub/sub (`overlay/mod.rs` `OverlayHub`) | In-memory `token → (tokio broadcast + VecDeque snapshot)` | Ephemeral (per-process broadcast; snapshot lost on restart) | None; snapshot capped `overlay.snapshot_lines` (def 3); channel cap 32; subscribers cap `overlay.max_subscribers` (def 100) | Per-instance, process-local `tokio::broadcast` — a subscriber only sees subtitles published by the **same** instance | None | **Deferred (BH-04)** — **hardest case**: needs Redis pub/sub for cross-instance fan-out | No subscriber → live send is a no-op (broadcast semantics); final lines still snapshotted |
| Entitlement / revocation cache | **Not implemented** | n/a | n/a | n/a | n/a | **Deferred (BH-09)** | n/a |

## Consequences

**Positive**

- One reference of record for who owns each store, its failure policy, and its
  cutover step — the incremental BH-03/BH-04 tasks now check against this table
  instead of re-deriving ownership from scratch each time.
- The fail-closed decisions are explicit and consistent: production refuses to
  start without `DATABASE_URL`; a configured-but-unreachable `REDIS_URL` fails
  fast; rate-limit backend errors are 503, never a silent allow.
- Records a subtlety that is easy to get wrong: "has a Postgres table" does **not**
  mean "Postgres-authoritative." `users`/`usage` are write-only mirrors,
  `platform_accounts` is unused, and `service_config`/`plans` are DB-first caches —
  only `browser_sessions` is direct-Postgres.

**Negative / accepted limitations**

- Until the deferred cutovers land, the backend is effectively single-instance for
  everything in §3.4 (overlay fan-out, CSRF/pairing/2FA codes, command queue) and
  still file-authoritative for §3.2/§3.3. Running two instances today would weaken
  those paths — the only §3.4 store made instance-safe in this pass is the rate
  limiters.
- `REDIS_URL` staying optional in production (deferred to BH-08) means a
  misconfigured deploy can still fall back to per-instance in-memory rate limiting
  without an error. This is a conscious, time-boxed gap.

**Follow-ups (not in this pass)**

- **BH-03B** — DB read-back + reconcile for `users`/`usage`, then a legacy file→DB
  importer CLI.
- **BH-03C** — the §3.3 domains (new migrations for relay/settings/founding/voice
  metadata; cut `TokenVault` over to `platform_accounts`; decide `stt_servers`;
  create the `overlay_tokens` table for Region D1 captions).
- **BH-04** (remaining) — the other §3.4 categories on Redis.
- **BH-08** — make `REDIS_URL` (and other reachability/typed-env rules) mandatory
  in production.
- **BH-09** — the entitlement/revocation cache.
