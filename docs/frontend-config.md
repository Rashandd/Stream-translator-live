# Frontend Runtime Configuration

This document defines how StreamDubber keeps frontend behavior configurable from the admin panel. It complements `DESIGN.md`: design gives the visual memory, runtime config gives the source of truth for what the UI shows.

## Principle

Frontend code must not hard-code business values, product copy, pricing, plan limits, enabled providers, supported languages, platform availability, feature visibility, setup steps, SDK download URLs, or public marketing claims.

The only acceptable constants in frontend code are stable schema keys, route names, component names, accessibility fallbacks, and safe loading/error defaults. Everything user-visible that can change without a deploy must come from DB-backed config.

## Sources Of Truth

- `plans`: plan names, prices, monthly hours, language/platform limits, watermark status, Stripe price IDs, active/inactive state.
- `service_config`: provider toggles, fallback rules, stream limits, VAD state, and other operational settings.
- `ui_config`: recommended new table or `service_config` namespace for user-visible UI and theme config.
- `platform_accounts`: connected integrations and per-user platform state.
- `sdk_sessions`: pairing code, device name, SDK version, active state, last seen time.
- `streams` and `usage_records`: live status, monthly usage, limits, and stream history.

If a dedicated `ui_config` table is added, use this shape:

```sql
CREATE TABLE ui_config (
    key         TEXT PRIMARY KEY,
    value       JSONB NOT NULL,
    description TEXT,
    is_public   BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_by  UUID REFERENCES users(id)
);
```

Use `is_public = true` only for values that are safe before login, such as public theme tokens, public landing copy, public language lists, public platform logos, and public pricing summaries. Secrets never live here.

## Config Namespaces

Use namespaced keys so admin editing, caching, and frontend consumption stay predictable.

```text
ui.brand.name
ui.brand.logo_asset
ui.brand.site_url
ui.theme.active
ui.theme.tokens
ui.public.home.hero
ui.public.home.metrics
ui.public.home.compatibility_logos
ui.public.home.sections
ui.nav.broadcaster
ui.nav.admin
ui.dashboard.home.cards
ui.dashboard.onboarding.steps
ui.dashboard.language_defaults
ui.sdk.downloads
ui.integrations.platforms
ui.feature_flags
ui.security.claims
ui.billing.plan_copy
```

Operational provider keys stay in `service_config`:

```text
stt.provider
stt.fallback
translate.provider
translate.fallback
tts.provider
stream.max_bitrate
vad.enabled
```

Plan data stays in `plans`; the frontend may read decorated copy from `ui.billing.plan_copy`, but the price and limit truth comes from DB rows.

## API Contract

Recommended endpoints:

```text
GET /api/public-config
GET /api/me/config
GET /api/admin/config
PATCH /api/admin/config/:key
```

`/api/public-config` returns only public-safe config. `/api/me/config` merges public config, authenticated user state, plan state, SDK state, integration state, feature flags, and current service config. `/api/admin/config` exposes editable keys with descriptions, validation metadata, audit data, and draft/published status if drafts are implemented.

## Frontend Rules

- Every page loader must fetch config before rendering user-visible business content.
- Components receive labels, descriptions, options, limits, and visibility as props from config-aware page loaders.
- Empty states and errors may use safe fallback text, but product copy must still come from config when data loads.
- Language selectors use configured language arrays, not local hard-coded lists.
- Provider selectors use configured provider arrays and current `service_config` values.
- Platform cards use configured platform metadata and per-user `platform_accounts`.
- Pricing pages render from `plans` plus optional configured marketing copy.
- Dashboard metric cards render from configured card definitions plus live API data.
- Theme tokens can be exported from `DESIGN.md`, but active theme selection and public-facing brand values are admin-managed.

## Admin Panel Requirements

Admin must be able to change, without redeploy:

- STT, translation, and TTS provider toggles.
- Fallback rules.
- Public hero copy and proof metrics.
- Supported public languages.
- Enabled platform integrations.
- Pricing plan visibility, names, limits, and features.
- Feature flags for Phase 1 vs later phases.
- SDK download links and minimum supported versions.
- Dashboard navigation visibility.
- Security and privacy claims shown on public pages.

Every admin config write creates an `audit_logs` entry with old value, new value, actor, timestamp, and IP address.

## Caching

Use ETags or version numbers for public config. Cache public config at the edge for a short TTL, then invalidate on admin publish. Authenticated config can use Redis with per-user keys and short TTLs.

Admin changes that affect live pipeline behavior should publish an internal invalidation event so backend workers and frontend sessions refresh without restart.

## Test Expectations

- Unit test config parsers and default validation.
- Integration test that changing a config key changes the rendered frontend behavior without code changes.
- E2E test public page renders from `/api/public-config`.
- E2E test admin edits a provider toggle and dashboard reflects the new active provider.
- Regression test that no pricing, language, provider, platform, or marketing copy arrays are introduced as frontend constants.
