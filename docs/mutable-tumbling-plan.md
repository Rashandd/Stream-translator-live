# BH-03 (minimal foundation) + BH-04 (Redis connection + rate limiters)

## Context

`docs/backend-hardening-captions-plan.md` calls for two large pieces of work: BH-03
(make PostgreSQL the single source of truth for persistent domain state — currently
split across Postgres/memory/encrypted files) and BH-04 (move ephemeral, currently
single-instance in-memory state to Redis so the backend can run as multiple
instances). The plan document itself states BH-03 and BH-04 are each too large for
one commit/PR and must be split into sub-steps (full BH-03 = ADR + 8 new migrations +
cutover of 11 domains + a legacy-importer CLI; full BH-04 = 8 categories of ephemeral
state). The user picked BH-03+BH-04 as the next work item, then — when shown the true
size — chose to scope this pass to: a minimal BH-03 foundation (the ADR the plan
mandates as its first step, plus the one concrete BH-03A requirement that a
fail-closed check for `DATABASE_URL` in production be added), followed by moving
quickly into BH-04's core deliverable (a real Redis connection wired into the
backend, proven end-to-end by migrating the rate limiters — the first category BH-04
lists). Everything else (BH-03B/C/D's other 10 domains + legacy importer; BH-04's
remaining 7 ephemeral categories) is explicitly deferred to follow-up tasks.

This branch/worktree: `/home/adem/Projects/GitHub/streamdubber-bh-03-04-postgres-redis`,
branch `feat/bh-03-04-postgres-redis`, branched from `main` (does not include the
sibling `feat/bh-02-config-validation` branch's work, which is a separate
not-yet-merged task — no dependency taken on it here; a small amount of duplicated
"read `APP_ENV`" logic is expected and easily reconciled whenever both branches
merge).

## Part 1 — ADR (docs only, do first)

Create `docs/adr/0001-production-state-ownership.md` (repo's first ADR — no existing
template to match, use a plain Context/Decision/Consequences structure). Body: one
table row per store, columns = veri sahibi (owner) / durability / TTL / cache
invalidation / encryption / migration-cutover / failure policy, covering:
- Already-Postgres-primary: `service_config`, `plans`, `browser_sessions` (auth/session.rs),
  `channel_bindings` (bindings/mod.rs).
- Partial/one-way-mirror today, target Postgres: `users` (UserStore — mirror is
  write-only, no DB read-back yet), `usage` (UsageTracker — same one-way pattern).
- File/memory only today, target Postgres (BH-03C domains 4-11, deferred past this
  pass): SDK sessions, relay targets, platform/OAuth accounts (`platform_accounts`
  table exists in schema.md but is unused — real tokens live in `TokenVault`/
  `TOKENS_FILE`), stream/STT/TTS settings, founding applications, voice model
  metadata + sample *reference* (raw audio bytes stay in encrypted file storage, only
  metadata/pointer moves to Postgres), STT server metadata, overlay tokens (new
  table, doesn't exist yet — needed later for Bölge D1, not created in this pass).
- Ephemeral, target Redis (BH-04 domains, only rate limiters land in this pass):
  rate limiters (`rate_limit.rs`, `pairing::ClaimLimiter`), OAuth CSRF state
  (`auth/oauth_login.rs`'s private `LoginStateStore`), pairing codes (`PairingStore`),
  2FA/reset codes (`TwoFactorStore`), SDK command queue (`SdkCommandQueue`),
  active-stream presence (`overlay::ActiveStreams`), overlay snapshot+pubsub
  (`overlay::OverlayHub`), entitlement/revocation cache (not implemented yet, planned
  for BH-09).
- Correction (this plan was originally written while BH-02 was still an unmerged
  sibling branch; BH-02 has since merged into `main`): a typed `Environment` enum
  (`backend/src/config.rs`, `Environment::from_env()`, `Production`/`Development`,
  already imported and used in `main.rs`'s `build_state()`) now exists. BH-03A must
  reuse it directly — do NOT add a second, parallel raw-string `APP_ENV` reader.
  Using two independent environment-detection mechanisms would diverge silently:
  `Environment::from_value` matches `APP_ENV` case-insensitively
  (`eq_ignore_ascii_case`), so a naive new `env == "production"` string check would
  miss `APP_ENV=Production`. BH-08 remains the owner of any *further* typed-model
  work (JWT strength, mTLS, SMTP, CORS, TLS, persistence-file-mode, DB/Redis
  reachability) — this pass only consumes the existing `Environment` enum, it does
  not extend it.

## Part 2 — BH-03A minimal: fail-closed DATABASE_URL in production

Files: `backend/src/main.rs` only — **no new env-parsing code in `config.rs`**,
since `Environment` already exists there (BH-02, merged). Do not reintroduce a raw
`APP_ENV` string reader alongside it.

1. `main.rs`: add a small pure, unit-testable function (same reasoning as
   `Environment::from_value`'s split in `config.rs`: keep the check free of direct
   env-var reads so tests don't race on process-global `set_var`):
   ```rust
   fn enforce_production_database_requirement(
       environment: Environment,
       database_url: &Option<String>,
   ) -> Result<(), String> {
       if environment == Environment::Production && database_url.is_none() {
           return Err(
               "DATABASE_URL is required when APP_ENV=production — refusing to start \
                with file-only persistence (BH-03A)".to_string(),
           );
       }
       Ok(())
   }
   ```
2. Call it in `run()` right after `let config = Config::from_env()?;`. `Environment`
   is already imported in `main.rs` (`config::{Config, Environment, ServiceConfig}`);
   today it's only computed later, inside `build_state()` (~line 193). Compute it
   once here instead and pass it into `build_state()` as a new parameter (drop the
   redundant second `Environment::from_env()` call there):
   ```rust
   let environment = Environment::from_env();
   enforce_production_database_requirement(environment, &config.database_url)
       .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
   ```
3. Add `#[cfg(test)] mod tests` to `main.rs` (currently has none) with 3 cases:
   production+no-DB → error; production+DB → ok; development+no-DB → ok — construct
   `Environment::Production`/`Environment::Development` directly, no env-var
   mutation needed since the function takes `Environment`, not a raw string.

## Part 3 — BH-04 first increment: Redis connection manager + rate limiters

### 3a. Dependency

Add to root `Cargo.toml` `[workspace.dependencies]` (verify latest version on
crates.io at implementation time — do not blindly trust a hardcoded version number):
```toml
redis = { version = "<latest 1.x>", default-features = false, features = ["tokio-comp", "connection-manager", "script"] }
```
Add `redis.workspace = true` to `backend/Cargo.toml` `[dependencies]`.

### 3b. New `backend/src/cache.rs` module (mirrors `backend/src/db/mod.rs`'s shape)

```rust
use redis::aio::ConnectionManager;

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("redis connection failed: {0}")]
    Connect(#[from] redis::RedisError),
}

/// Fail-fast like `db::connect`: explicit REDIS_URL that's unreachable aborts
/// startup. `ConnectionManager` itself auto-reconnects on later transient drops.
pub async fn connect(redis_url: &str) -> Result<ConnectionManager, CacheError> {
    let client = redis::Client::open(redis_url)?;
    Ok(client.get_connection_manager().await?)
}

/// `streamdubber:{env}:{parts...}` per plan §15's key format.
pub fn key(env: &str, parts: &[&str]) -> String {
    let mut out = format!("streamdubber:{env}");
    for part in parts {
        out.push(':');
        out.push_str(part);
    }
    out
}
```
Register `pub mod cache;` in `backend/src/lib.rs`.

### 3c. `Config` + `AppState` wiring

- `config.rs`: add `pub redis_url: Option<String>` to `Config`, read from `REDIS_URL`
  in `from_env()` (same pattern as `database_url`). Also add
  `impl Environment { pub fn as_label(&self) -> &'static str { ... } }` returning
  `"production"`/`"development"` — this is the single source for the Redis key
  namespace segment; do not add a separate string-producing function (see Part 2
  correction — no `app_env_label()`).
- `state.rs`: add `pub redis: Option<redis::aio::ConnectionManager>` field (raw type,
  no wrapper — matches existing `pub db: Option<sqlx::PgPool>` convention exactly),
  add `impl FromRef<AppState> for Option<redis::aio::ConnectionManager>`, set
  `redis: None` in `skeleton()`.
- `main.rs` `run()`: after the existing `db_pool` construction, connect Redis the
  same way — `Some(url) => cache::connect(url).await?` (fail-fast via `?`),
  `None => { warn!(...); None }`. Pass `redis` into `build_state()` (new parameter),
  alongside the `environment: Environment` computed in Part 2 (also now a
  `build_state()` parameter instead of being recomputed inside it). Inside
  `build_state()`, use `environment.as_label()` (not a new `app_env_label()`
  function) to construct `auth_limiter`/`pairing_claim_limiter` via `with_redis(...)`
  when `redis.is_some()`, else the existing `::new()`.

### 3d. `rate_limit.rs` — generalize the core, add Redis backend

Read the full current file first (know every call site before touching it: grep
`AuthLimiter` across `backend/src/`). Add:
```rust
#[derive(Debug, thiserror::Error)]
pub enum RateLimitError {
    #[error("redis rate limiter backend error: {0}")]
    Redis(#[from] redis::RedisError),
}

pub fn to_http_error(err: &RateLimitError) -> (axum::http::StatusCode, String) {
    tracing::error!(%err, "rate limiter backend unavailable");
    (axum::http::StatusCode::SERVICE_UNAVAILABLE,
     "rate limiting temporarily unavailable, try again shortly".to_string())
}
```
Generalize `RateLimiter` to hold `redis: Option<redis::aio::ConnectionManager>` +
`env_label: String` alongside the existing `Mutex<HashMap<...>>`. `check` becomes
`pub async fn check(&self, endpoint: &str, key: &str, limit: u32) -> Result<bool, RateLimitError>`
— `endpoint` generalizes today's hardcoded `"login:"`/`"signup:"` string-prefix trick
into an explicit parameter reused for the Redis key. Redis path uses one atomic Lua
script (single round-trip, avoids the INCR-then-EXPIRE gap that would otherwise leave
a TTL-less key on a crash mid-operation):
```lua
local count = redis.call('INCR', KEYS[1])
if count == 1 then redis.call('EXPIRE', KEYS[1], ARGV[1]) end
return count
```
via `redis::Script`, `static ... LazyLock<Script>` (std `LazyLock`, stable, no new
dep). Memory path keeps today's exact logic, just keyed by `"{endpoint}:{key}"`.

`AuthLimiter`: unchanged field shape (`inner: RateLimiter`), add `with_redis(redis, env_label)`
constructor, `check_login`/`check_signup` become `async fn ... -> Result<bool, RateLimitError>`
calling `self.inner.check("auth-login", ...)` / `self.inner.check("auth-signup", ...)`.

`pairing/mod.rs`'s `ClaimLimiter`: currently ~15 lines duplicating `RateLimiter`'s
fixed-window logic independently — refactor to delegate to the same shared core
(`inner: RateLimiter`, `with_redis` constructor, `check()` calls
`self.inner.check("pairing-claim", key, CLAIM_RATE_LIMIT_PER_MIN)`), same as
`AuthLimiter`. This is in-scope cleanup, not scope creep, since it collapses
duplicated logic that the Redis work would otherwise have to duplicate a second time.
Do **not** touch `WaitlistLimiter` (`waitlist.rs`) or `ExtTwitch::check_rate`
(`ext_twitch/mod.rs`) — confirmed independent duplicates too, but out of scope for
this pass; leave a one-line comment noting them as a future fold-in.

### 3e. Call-site updates (convert bool-check to async Result match)

- `backend/src/users/mod.rs` (signup handler, login handler — grep exact line
  numbers, they shift from BH-02-unrelated changes on main): change
  `if !app.auth_limiter.check_signup(...)` to
  `match app.auth_limiter.check_signup(...).await { Ok(true) => {}, Ok(false) => { ...existing 429... }, Err(err) => return Err(rate_limit::to_http_error(&err)) }`.
  Same shape for `check_login`.
- `backend/src/pairing/mod.rs` (claim handler): same three-way match for the
  `ClaimLimiter::check` call, reusing the existing "too many pairing attempts"
  message on the `Ok(false)` arm.

### 3f. Fail-closed policy (record in the ADR from Part 1, implement here)

- **Startup**: `REDIS_URL` stays optional in every environment in this pass (mirrors
  `DATABASE_URL`'s current status) — absent means the in-memory limiter is used
  (single-instance only, a scaling limit, not a security regression, since that's
  already today's behavior everywhere). Present-but-unreachable is a hard startup
  failure (`cache::connect`'s `?`), identical to how `db::connect` already treats a
  misconfigured `DATABASE_URL`. Making `REDIS_URL` mandatory in production is
  deferred to BH-08 (needs the typed `Environment` model that task is chartered to
  add).
- **Runtime**: whenever Redis is configured, any error from a rate-limit check
  (timeout, connection refused, script error) becomes `503` — never a silent allow.
  This is unconditional (not gated by environment), extending the precedent already
  set by `SessionStore::rotate()` (a Postgres error during refresh → `401`, not a
  silent pass-through), not the different, permissive precedent set by
  `ChannelBindingStore` (memory-authoritative cache, best-effort DB write) — a rate
  limiter is a security control, not a durability cache, so the stricter precedent
  applies.

### 3g. Tests

Existing in-memory rate-limiter tests: update call sites for the new
async/`Result` signature, behavior unchanged. New tests in `rate_limit.rs` (or a
`backend/tests/rate_limit_redis.rs` integration file if that reads cleaner):
- `unreachable_redis_url_fails_fast_at_connect` — **not ignored**, needs no live
  Redis (connects to `redis://127.0.0.1:1`, asserts `Err`) — runs in normal CI.
- Following the exact `#[ignore = "requires TEST_DATABASE_URL..."]` convention
  already used in this codebase (see `waitlist.rs`, `billing/persist.rs`), add
  `#[ignore = "requires TEST_REDIS_URL (a running Redis instance)"]` tests:
  - `two_rate_limiter_instances_share_counter_through_redis` — two independent
    `RateLimiter::with_redis` against the same URL, same key, assert combined cap.
  - `ttl_expiry_clears_the_underlying_redis_key` — assert TTL set after first check
    (raw `TTL` command), then assert key gone after the window elapses.
  - `concurrent_increments_never_exceed_the_limit` — ~50 concurrent `check()` calls
    via `tokio::spawn`/`join_all` (futures_util already a dev-dependency), assert
    exactly `limit` succeed.
  - `redis_backed_limiter_allows_up_to_limit_then_blocks` — parity with the existing
    in-memory test, against real Redis.
- No local Redis/docker is available in this environment this session (same
  situation as BH-02's DB-gated test) — these `#[ignore]`d tests won't be run here;
  note that in the final handoff, matching how the BH-02 DB test was handled.

### 3h. Rollout docs (explicitly required by the plan's BH-04 acceptance note)

- `infra/docker-compose.yml`: add `depends_on: redis: condition: service_healthy` to
  the `backend` service; remove the "backend doesn't connect yet" comment above it.
- `.env.example`: remove the "backend henüz kullanmıyor" comment above `REDIS_URL`.
- `CLAUDE.md`: update the two "Redis compose'da tanımlı ama backend HENÜZ
  bağlanmıyor" notes (Stack section + Docker section) to reflect that rate limiting
  now uses Redis when configured.
- `infra/deploy.md`: document `REDIS_URL` in the env table alongside `DATABASE_URL`
  as configured (not yet "required," per the deferred-to-BH-08 decision above).

## Verification

1. `cargo build -p streamdubber-backend` (root of the worktree) — confirms the new
   `redis` dependency resolves and everything compiles.
2. `cargo test -p streamdubber-backend --no-fail-fast` — all existing tests must
   stay green (461+ before this pass); new unit tests (main.rs's 3 fail-closed
   cases, cache.rs's fail-fast-connect case, rate_limit.rs's updated async tests)
   must pass. `#[ignore]`d Redis integration tests are expected to be skipped (no
   local Redis available).
3. `cargo clippy -p streamdubber-backend --all-targets --all-features -- -D warnings`
   — must be clean.
4. `cargo fmt` — format ONLY the files actually touched (learned from the BH-02
   session: never run `rustfmt`/`cargo fmt` against `lib.rs` or `main.rs` directly,
   since both are crate roots and rustfmt will recursively reformat every module
   reachable via `mod`/`use` — format each touched file individually, or hand-format
   small additions, then verify with `cargo fmt --check -p streamdubber-backend --
   --check | grep "^Diff in" | grep -E "<only the touched filenames>"` before
   committing).
5. `git status`/`git diff --stat` before every commit — confirm only intentionally
   touched files appear (same discipline as the BH-02 session, after the earlier
   whole-crate-reformat mishap there).
6. Manual/documented-but-not-automated: with a real Redis, `docker compose stop
   redis` then hit `/api/auth/login` repeatedly — confirm `503`, not a silent
   `200`/bypass, once past the rate limit. Note in the handoff that this specific
   scenario needs a live Redis this session doesn't have, same treatment as the
   `#[ignore]`d tests.

## Commits (multiple, one responsibility each — do not squash)

1. `docs: add production state ownership ADR (BH-03)`
2. `feat: require DATABASE_URL in production (BH-03A)`
3. `feat: add redis connection manager (BH-04)`
4. `feat: migrate rate limiters to redis-backed atomic incr+expire (BH-04)`
5. `chore: wire redis into docker-compose and update deploy docs (BH-04)`

## After this pass

Update `docs/backend-hardening-captions-plan.md` §11 checklist and
`streamdubber_roadmap_final.md`: do **not** tick BH-03 or BH-04 as complete (both
have substantial remaining work — BH-03B/C/D's other domains + legacy importer;
BH-04's other 7 ephemeral categories) — instead add a status blockquote under each
heading (matching the BH-01/BH-02 pattern already in the doc) describing exactly
what landed and what's deferred, so the next session picks up cleanly. Update the
worktree-local `claude.md` note per CLAUDE.md's requirements. Do not push or merge
unless the user explicitly asks.
