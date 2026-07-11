---
version: alpha
name: StreamDubber Realtime Broadcast Console
description: A Palabra-inspired, StreamDubber-owned visual memory for the broadcaster dashboard, admin console, setup flows, and public product pages.
colors:
  canvas: "#F7F8FA"
  surface: "#FFFFFF"
  surface-muted: "#EEF2F6"
  surface-warm: "#FFF5F0"
  ink: "#061A3D"
  text: "#273247"
  muted: "#68748A"
  faint: "#98A2B3"
  border: "#D7E0EA"
  hairline: "#E8EEF5"
  primary: "#061A3D"
  primary-hover: "#273247"
  on-primary: "#FFFFFF"
  accent-coral: "#FF6A4A"
  accent-blue: "#2F6BFF"
  accent-violet: "#9A8CFF"
  accent-cyan: "#36D8E4"
  success: "#13A66B"
  warning: "#F59E0B"
  danger: "#D83A46"
  overlay: "#061A3D"
typography:
  display:
    fontFamily: Inter
    fontSize: 72px
    fontWeight: 650
    lineHeight: 1.03
    letterSpacing: 0em
  h1:
    fontFamily: Inter
    fontSize: 44px
    fontWeight: 650
    lineHeight: 1.08
    letterSpacing: 0em
  h2:
    fontFamily: Inter
    fontSize: 30px
    fontWeight: 620
    lineHeight: 1.16
    letterSpacing: 0em
  h3:
    fontFamily: Inter
    fontSize: 20px
    fontWeight: 620
    lineHeight: 1.28
    letterSpacing: 0em
  body:
    fontFamily: Inter
    fontSize: 16px
    fontWeight: 400
    lineHeight: 1.55
    letterSpacing: 0em
  body-sm:
    fontFamily: Inter
    fontSize: 14px
    fontWeight: 400
    lineHeight: 1.45
    letterSpacing: 0em
  label:
    fontFamily: Inter
    fontSize: 13px
    fontWeight: 560
    lineHeight: 1.25
    letterSpacing: 0em
  data:
    fontFamily: Geist Mono
    fontSize: 13px
    fontWeight: 500
    lineHeight: 1.35
    letterSpacing: 0em
rounded:
  xs: 4px
  sm: 6px
  md: 8px
  lg: 12px
  pill: 999px
spacing:
  xxs: 4px
  xs: 8px
  sm: 12px
  md: 16px
  lg: 24px
  xl: 32px
  xxl: 48px
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.on-primary}"
    typography: "{typography.label}"
    rounded: "{rounded.pill}"
    padding: 12px
    height: 44px
  button-primary-hover:
    backgroundColor: "{colors.primary-hover}"
    textColor: "{colors.on-primary}"
  button-secondary:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    typography: "{typography.label}"
    rounded: "{rounded.pill}"
    padding: 12px
    height: 44px
  input:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    typography: "{typography.body-sm}"
    rounded: "{rounded.md}"
    padding: 12px
    height: 42px
  status-pill:
    backgroundColor: "{colors.surface-muted}"
    textColor: "{colors.text}"
    typography: "{typography.label}"
    rounded: "{rounded.pill}"
    padding: 8px
  card:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.md}"
    padding: 24px
  metric-card:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    rounded: "{rounded.md}"
    padding: 20px
  page:
    backgroundColor: "{colors.canvas}"
    textColor: "{colors.text}"
  section-warm:
    backgroundColor: "{colors.surface-warm}"
    textColor: "{colors.ink}"
  divider:
    backgroundColor: "{colors.hairline}"
    textColor: "{colors.ink}"
    height: 1px
  border-line:
    backgroundColor: "{colors.border}"
    textColor: "{colors.ink}"
    height: 1px
  caption:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.muted}"
    typography: "{typography.body-sm}"
  inverse-caption:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.faint}"
    typography: "{typography.body-sm}"
  button-accent:
    backgroundColor: "{colors.accent-coral}"
    textColor: "{colors.ink}"
    typography: "{typography.label}"
    rounded: "{rounded.pill}"
    padding: 12px
    height: 44px
  language-chip-active:
    backgroundColor: "{colors.accent-blue}"
    textColor: "{colors.ink}"
    typography: "{typography.label}"
    rounded: "{rounded.pill}"
    padding: 8px
  route-chip:
    backgroundColor: "{colors.accent-violet}"
    textColor: "{colors.ink}"
    typography: "{typography.label}"
    rounded: "{rounded.pill}"
    padding: 8px
  latency-chip:
    backgroundColor: "{colors.accent-cyan}"
    textColor: "{colors.ink}"
    typography: "{typography.data}"
    rounded: "{rounded.pill}"
    padding: 8px
  status-success:
    backgroundColor: "{colors.success}"
    textColor: "{colors.ink}"
    typography: "{typography.label}"
    rounded: "{rounded.pill}"
    padding: 8px
  status-warning:
    backgroundColor: "{colors.warning}"
    textColor: "{colors.ink}"
    typography: "{typography.label}"
    rounded: "{rounded.pill}"
    padding: 8px
  status-danger:
    backgroundColor: "{colors.danger}"
    textColor: "{colors.ink}"
    typography: "{typography.label}"
    rounded: "{rounded.pill}"
    padding: 8px
  modal-overlay:
    backgroundColor: "{colors.overlay}"
    textColor: "{colors.on-primary}"
---

## Overview

This file is the frontend memory bank for StreamDubber. Every frontend agent must read it before building dashboard pages, admin screens, onboarding, public product pages, or component libraries.

The visual direction is inspired by Palabra's current public site: calm bright surfaces, large direct messaging, soft red-to-violet energy fields, pill-shaped controls, a language/grid rhythm, proof metrics, and a strong real-time voice translation narrative. StreamDubber must not copy Palabra assets, copy, logo, illustrations, exact gradients, or brand marks. Use the same level of clarity and polish, then make it a broadcaster-first SaaS console.

StreamDubber should feel like a live broadcast control room made simple: fast to scan, confident, private, and operational. The first viewport should always communicate "audio-only cloud pipeline, live captions, translation, and multistream setup" without marketing fog.

The Strovak logo is the visual anchor: deep navy wordmark, cyan-to-blue motion line, small violet audio accent, and generous white space. UI decisions should support that mark with dark navy text, clean white surfaces, precise SVG icons, and restrained blue/cyan/violet accents.

Frontend must be internationalization-ready from the start. Development may ship with English copy only, but copy lives in locale dictionaries/config so Turkish and other languages can be added without rewriting components.

## Colors

Use a mostly white and off-white foundation. The interface should breathe, but it must not become empty. Pair clean white surfaces with hairline borders, dark text, and a small number of vivid accents.

Use blue and coral for active translation energy, language movement, and primary product moments. Use violet and cyan as secondary signal colors for latency, WebSocket, SDK pairing, and platform routing. Do not let violet dominate the whole app.

Status colors are functional, not decorative. Green means connected or healthy. Amber means degraded, setup needed, or nearing a limit. Red means failed, disconnected, blocked, or security risk.

## Dark Theme

Every surface must work in both a light and a dark theme. The colors above (`colors:` in the frontmatter) are the light-theme values and stay canonical — dark theme is a second, separately tuned palette applied via a `.dark` class on `<html>`, not a filter or invert over the light palette.

Dark-theme token values (`frontend/app/globals.css`, `:root.dark`):

| Token | Light | Dark |
|---|---|---|
| canvas | `#F7F8FA` | `#080F1C` |
| surface | `#FFFFFF` | `#0F1829` |
| ink | `#061A3D` | `#F4F7FB` |
| text | `#273247` | `#C7D0DE` |
| muted | `#68748A` | `#8B96A8` |
| faint | `#98A2B3` | `#5C6B82` |
| hairline | `#E8EEF5` | `#1C2A42` |
| border | `#D7E0EA` | `#24344F` |
| primary | `#061A3D` | `#3C69F2` |
| primary-hover | `#273247` | `#4E6FC9` |

Accent colors (`cyan`, `violet`, `coral`, `success`, `warning`) keep their light-theme hex unchanged in dark mode — they already read clearly on both a white and a near-black surface. `blue` and `danger` get a slightly lightened dark-mode variant (`#447AFF`, `#DD545E`) so they still clear WCAG AA (4.5:1) when used as plain foreground text (links, error copy) against the dark `surface` color; at low opacity (backgrounds, tinted chips) the shift is imperceptible.

Rules:

- Never invert or filter the light palette to produce dark mode. Tune each token so it still reads as "StreamDubber," not as a color-inverted accident.
- `primary`/`primary-hover` flip from navy to a bright blue in dark mode — solid navy-on-near-black has no separation, so the CTA needs a genuinely different color, not just a lighter navy.
- Default theme follows the OS/browser `prefers-color-scheme`. An explicit user choice (the theme toggle) is stored and always wins over the system preference; the app should keep watching for OS preference changes only while no explicit choice has been made.
- Brand marks (the Strovak logo) need their own dark-mode asset — recoloring navy wordmark pixels to `ink` is not enough on its own, since raster logo exports often carry invisible-on-white antialiasing fringe that becomes a visible light halo on a dark background. Any new logo export must be checked on both the light and dark canvas colors, not just white.

## Typography

Use Inter for product UI and marketing text. Use Geist Mono for latency, bitrate, tokens, IDs, stream keys, request IDs, pairing codes, and logs.

Large type is allowed only in true hero areas and important setup moments. Dashboard panels use compact headings and dense, legible data. Never scale type with viewport width. Letter spacing stays at 0.

Headlines should be short and literal. Prefer "Live captions for every stream" over vague claims. Body copy should be concise, concrete, and tied to the current action.

## Layout

Use generous vertical rhythm on public and onboarding pages. Use denser grids in authenticated dashboards. Avoid nested cards and decorative card stacks.

Preferred public-page pattern:

- A classic navbar with a large readable Strovak logo, direct text links, and one clear CTA.
- A short first viewport: direct headline, one supporting sentence, two CTAs, and a clean product/animation placeholder.
- Detailed product, platform, security, and solution explanations should move to later pages or dashboard flows, not the initial landing view.

Preferred dashboard pattern:

- Left navigation or compact top navigation.
- One primary work surface per page.
- Status strip near the top for stream state, SDK connection, latency, and current plan usage.
- Configuration panels that map directly to backend/admin config keys.

## Responsive & Mobile

StreamDubber is mobile-aware from the first commit. English-only copy is acceptable during development, but every screen must stay usable from a 360px phone up to a wide desktop. Design mobile-first: start from the small layout, then add complexity at larger breakpoints, never the reverse.

Use a single, consistent set of breakpoints aligned with Tailwind defaults: base (mobile, under 640px), `sm` (640px), `md` (768px), `lg` (1024px), `xl` (1280px). Do not invent custom per-component breakpoints. Type sizes step between fixed values at breakpoints — never scale type fluidly with viewport width.

Full-bleed elements (the navbar background, gradient wash bands, section dividers) span the full width. Readable content — headings, body copy, cards, forms, tables — stays inside a centered max-width container (around 1280px) with horizontal padding of 16px on mobile, 24px at `sm`, and 32px at `lg`. Never let text run edge to edge on large screens.

Navigation collapses on mobile. Below `md`, primary nav links and secondary controls move into a single hamburger menu or a compact sheet; only the logo and the primary CTA stay visible in the bar. Every navigation item reachable on desktop must remain reachable on mobile — never hide links with no mobile equivalent.

Layout reflow rules:

- Multi-column card grids collapse to one column on mobile, two at `sm`, three or four at `lg`. Use `gap` for rhythm, not nested cards.
- The public hero stacks vertically on mobile: headline, one sentence, stacked full-width CTAs, then the product visual below — not side by side.
- Dashboard side navigation collapses to a top bar or drawer on mobile; the status strip wraps to multiple rows instead of overflowing.
- Tables that cannot fit become stacked key/value rows or horizontally scroll inside their own container — the page itself never scrolls sideways.

Touch targets are at least 44px in the smallest dimension. Pills, icon buttons, toggles, and language chips must stay tappable. Hover-only affordances need a tap or focus equivalent; tooltips must not hide critical information on touch devices.

Test every new frontend surface at 360px, 768px, and 1280px before considering it done. No horizontal overflow, no clipped controls, no unreadable shrunken type at any of those widths.

## Elevation & Depth

Use almost no heavy shadow. Depth comes from spacing, borders, muted surfaces, and occasional soft gradient bands. Cards can have a subtle shadow only when they are repeated items, modals, or focused setup tools.

The signature atmospheric treatment is a soft, diffused coral-to-violet-to-cyan gradient, used behind hero/product visuals or as a subtle section wash. It must never replace actual UI, screenshots, integration logos, or product state.

## Shapes

Controls use pills when they are CTAs, filters, language chips, platform chips, or status indicators. Cards and panels use 8px radius by default. Large rounded blobs are reserved for background energy fields only, never for data containers.

Icons should come from lucide when available. Do not invent manual SVG icons for common actions. Use icon-only buttons for familiar tools and add tooltips for less obvious controls.

Never use emoji in the product UI, docs intended as frontend memory, marketing surfaces, dashboard cards, buttons, empty states, or status labels. Use professional SVG icons instead. Prefer lucide SVG icons for generic actions and carefully prepared brand SVG/logo assets only when usage rights and brand guidelines are clear.

## Components

Buttons are compact, pill-shaped, and action-specific. Primary buttons are dark with white text. Secondary buttons are white with a border. Destructive actions use explicit confirmation and red only at the point of risk.

Forms must look operational, not decorative. Every input maps to a known config key, user setting, integration credential flow, or DB-backed option. Selects, toggles, segmented controls, and sliders should reveal the real system model.

Metric cards show one number, one label, and one context hint. Good examples: current latency, active streams, seconds used this month, connected SDK version, STT provider, translation fallback.

Tables are preferred for admin lists, users, pricing plans, provider config, audit logs, and usage records. Cards are preferred for onboarding steps, integration tiles, and repeated platform connection items.

## Do's and Don'ts

Do:

- Read this file before creating or changing frontend UI.
- Use professional SVG icons and never use emoji.
- Keep frontend copy locale-aware; English-only development is acceptable, hard-coded monolingual components are not.
- Keep the landing page short, clear, and focused on what Strovak does.
- Use real product surfaces on the first screen, not a generic landing page.
- Keep StreamDubber's public pages close to Palabra's clarity: large promise, quick setup, visible compatibility, strong metrics.
- Keep authenticated pages closer to an operations console: dense, restrained, fast to scan.
- Pull visible strings, product claims, plan limits, provider choices, language lists, platform lists, feature flags, and theme values from admin-managed config.
- Show the live pipeline clearly: SDK local capture, audio-only cloud processing, captions, translation, and platform distribution.

Don't:

- Copy Palabra's exact copy, images, icons, logos, or brand-specific composition.
- Use emoji as icons, decoration, bullets, status markers, or brand personality.
- Put dense feature details, platform matrices, provider internals, or long explanations on the first landing viewport.
- Hard-code prices, plans, limits, languages, provider names, integration availability, marketing claims, onboarding steps, download URLs, or feature visibility.
- Use decorative orbs, bokeh, heavy shadows, nested cards, or one-note purple gradients.
- Build Phase 2 features as active UI in Phase 1. Voice cloning can appear only as disabled, configured, or roadmap state when the backend config says so.
- Hide security and privacy details in vague marketing language. StreamDubber's key claim is simple: video stays local; only audio goes to cloud.
