---
version: alpha
name: StreamDubber Realtime Broadcast Console
description: A Palabra-inspired, StreamDubber-owned visual memory for the broadcaster dashboard, admin console, setup flows, and public product pages.
colors:
  canvas: "#F7F8FA"
  surface: "#FFFFFF"
  surface-muted: "#EEF2F6"
  surface-warm: "#FFF5F0"
  ink: "#101114"
  text: "#2B2F36"
  muted: "#667085"
  faint: "#98A2B3"
  border: "#DDE3EA"
  hairline: "#EEF2F6"
  primary: "#121316"
  primary-hover: "#2A2D33"
  on-primary: "#FFFFFF"
  accent-coral: "#FF5A3D"
  accent-rose: "#F0436D"
  accent-violet: "#7667FF"
  accent-cyan: "#30B7D6"
  success: "#13A66B"
  warning: "#F59E0B"
  danger: "#E5484D"
  overlay: "#101114"
typography:
  display:
    fontFamily: Inter
    fontSize: 64px
    fontWeight: 650
    lineHeight: 1.02
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
    backgroundColor: "{colors.accent-rose}"
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

## Colors

Use a mostly white and off-white foundation. The interface should breathe, but it must not become empty. Pair clean white surfaces with hairline borders, dark text, and a small number of vivid accents.

Use coral and rose for active translation energy, language movement, and primary product moments. Use violet and cyan as secondary signal colors for latency, WebSocket, SDK pairing, and platform routing. Do not let violet dominate the whole app.

Status colors are functional, not decorative. Green means connected or healthy. Amber means degraded, setup needed, or nearing a limit. Red means failed, disconnected, blocked, or security risk.

## Typography

Use Inter for product UI and marketing text. Use Geist Mono for latency, bitrate, tokens, IDs, stream keys, request IDs, pairing codes, and logs.

Large type is allowed only in true hero areas and important setup moments. Dashboard panels use compact headings and dense, legible data. Never scale type with viewport width. Letter spacing stays at 0.

Headlines should be short and literal. Prefer "Live captions for every stream" over vague claims. Body copy should be concise, concrete, and tied to the current action.

## Layout

Use generous vertical rhythm on public and onboarding pages. Use denser grids in authenticated dashboards. Avoid nested cards and decorative card stacks.

Preferred public-page pattern:

- A centered hero with direct headline, short supporting copy, pill CTAs, proof metrics, and a soft full-width product visual or interactive scene below it.
- A language or platform compatibility grid that uses pill cells and real integration names.
- Alternating full-width sections for product capability, workflow, security, and setup speed.

Preferred dashboard pattern:

- Left navigation or compact top navigation.
- One primary work surface per page.
- Status strip near the top for stream state, SDK connection, latency, and current plan usage.
- Configuration panels that map directly to backend/admin config keys.

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
- Use real product surfaces on the first screen, not a generic landing page.
- Keep StreamDubber's public pages close to Palabra's clarity: large promise, quick setup, visible compatibility, strong metrics.
- Keep authenticated pages closer to an operations console: dense, restrained, fast to scan.
- Pull visible strings, product claims, plan limits, provider choices, language lists, platform lists, feature flags, and theme values from admin-managed config.
- Show the live pipeline clearly: SDK local capture, audio-only cloud processing, captions, translation, and platform distribution.

Don't:

- Copy Palabra's exact copy, images, icons, logos, or brand-specific composition.
- Use emoji as icons, decoration, bullets, status markers, or brand personality.
- Hard-code prices, plans, limits, languages, provider names, integration availability, marketing claims, onboarding steps, download URLs, or feature visibility.
- Use decorative orbs, bokeh, heavy shadows, nested cards, or one-note purple gradients.
- Build Phase 2 features as active UI in Phase 1. Voice cloning can appear only as disabled, configured, or roadmap state when the backend config says so.
- Hide security and privacy details in vague marketing language. StreamDubber's key claim is simple: video stays local; only audio goes to cloud.
