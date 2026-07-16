// StreamDubber Live Captions — shared pure logic.
//
// This file is imported by both viewer.js and config.js, and unit-tested with
// `node --test` (no bundler: the sources ARE the reviewable bundle, per Twitch
// review guidelines on human-readable JavaScript). Everything here is pure /
// DOM-free so it can be tested outside a browser.

// ---------------------------------------------------------------------------
// Deployment constants.
//
// These are the only per-environment values in the bundle. They are fixed per
// released version because Twitch requires every fetched URL to be listed in
// the extension's URL allowlist (see docs/twitch-extension-policy.md and the
// domain decision in docs/twitch-extension-plan.md, A4).
// ---------------------------------------------------------------------------

/**
 * Extension Backend Service origin (must be in the URL Fetching allowlist).
 * The review packaging command replaces this exact value for the selected
 * fixed profile before the archive is uploaded to Twitch.
 */
export const EBS_BASE = "https://ext.strovak.com";

/** Off-site management link shown ONLY on the broadcaster config page. */
export const DASHBOARD_URL = "https://strovak.com/dashboard";

/**
 * Return the EBS origin compiled into this archive. The browser location and
 * query string are intentionally not inputs: Twitch URL allowlists require a
 * submitted build to have one fixed network destination.
 */
export function resolveEbsBase() {
  return EBS_BASE;
}

/** REST endpoint for the channel caption state (poll fallback). */
export function stateUrl(base, channelId) {
  return `${base}/ext/twitch/state/${encodeURIComponent(channelId)}`;
}

/** Bind endpoint used by the broadcaster config page. */
export function bindUrl(base) {
  return `${base}/ext/twitch/bind`;
}

/**
 * Live captions WebSocket URL. The Twitch extension JWT travels as a query
 * parameter because the browser WebSocket API cannot set request headers.
 */
export function captionsWsUrl(base, channelId, token) {
  const ws = base.replace(/^http/, "ws");
  return `${ws}/ext/twitch/captions/${encodeURIComponent(channelId)}?token=${encodeURIComponent(token)}`;
}

// ---------------------------------------------------------------------------
// Language handling.
// ---------------------------------------------------------------------------

const LANG_RE = /^[a-z]{2,3}(-[a-z0-9]{2,8})?$/i;

/** Allowlist check for language codes coming from the backend. */
export function isValidLang(code) {
  return typeof code === "string" && LANG_RE.test(code);
}

/** Keep only valid, unique language codes (bounded, lowercased). */
export function sanitizeLanguages(list) {
  if (!Array.isArray(list)) {
    return [];
  }
  const out = [];
  for (const code of list) {
    if (!isValidLang(code)) {
      continue;
    }
    const lower = code.toLowerCase();
    if (!out.includes(lower)) {
      out.push(lower);
    }
    if (out.length >= 16) {
      break;
    }
  }
  return out;
}

// ---------------------------------------------------------------------------
// Server message parsing.
//
// Dynamic data is never trusted: messages are structurally validated here and
// later rendered exclusively via `textContent` (no innerHTML anywhere).
// ---------------------------------------------------------------------------

function isCaptionLine(value) {
  return (
    value !== null &&
    typeof value === "object" &&
    isValidLang(value.lang) &&
    typeof value.text === "string"
  );
}

/**
 * Parse one WebSocket text frame / one snapshot line into a subtitle object,
 * or return null for anything that is not a well-formed subtitle.
 */
export function parseSubtitle(raw) {
  let msg = raw;
  if (typeof raw === "string") {
    try {
      msg = JSON.parse(raw);
    } catch {
      return null;
    }
  }
  if (msg === null || typeof msg !== "object" || msg.type !== "subtitle") {
    return null;
  }
  if (!isCaptionLine(msg.source) || typeof msg.source.is_final !== "boolean") {
    return null;
  }
  const translations = Array.isArray(msg.translations)
    ? msg.translations.filter(isCaptionLine)
    : [];
  return {
    sequence: typeof msg.sequence === "number" ? msg.sequence : 0,
    source: {
      lang: msg.source.lang.toLowerCase(),
      text: msg.source.text,
      is_final: msg.source.is_final,
    },
    translations: translations.map((t) => ({
      lang: t.lang.toLowerCase(),
      text: t.text,
    })),
  };
}

/** Languages carried by one subtitle: source first, then translations. */
export function languagesOf(subtitle) {
  const langs = [subtitle.source.lang];
  for (const t of subtitle.translations) {
    if (!langs.includes(t.lang)) {
      langs.push(t.lang);
    }
  }
  return langs;
}

/**
 * Text to display for the selected language, or null when the subtitle has
 * no line in that language (the caption stays unchanged in that case).
 */
export function captionTextFor(subtitle, lang) {
  if (subtitle.source.lang === lang) {
    return subtitle.source.text;
  }
  const hit = subtitle.translations.find((t) => t.lang === lang);
  return hit ? hit.text : null;
}

// ---------------------------------------------------------------------------
// Connection status model (drives the status dot + label).
// ---------------------------------------------------------------------------

/** How long without a caption before "live" degrades to "delayed". */
export const DELAYED_AFTER_MS = 12000;

/**
 * Compute the viewer-facing status.
 * Inputs are plain booleans/numbers so the decision table is fully testable.
 */
export function computeStatus({ configured, live, connected, lastCaptionAgeMs }) {
  if (!configured) {
    return "not_configured";
  }
  if (!live) {
    return "offline";
  }
  if (!connected) {
    return "connecting";
  }
  if (lastCaptionAgeMs === null) {
    return "waiting";
  }
  return lastCaptionAgeMs > DELAYED_AFTER_MS ? "delayed" : "live";
}

// ---------------------------------------------------------------------------
// Reconnect backoff (no aggressive retry, per Twitch performance guidance).
// ---------------------------------------------------------------------------

/** Exponential backoff: base * 2^attempt, capped. attempt is 0-based. */
export function nextDelayMs(attempt, baseMs = 1000, maxMs = 30000) {
  const n = Math.max(0, Math.min(attempt, 30));
  return Math.min(baseMs * 2 ** n, maxMs);
}

/** After this many consecutive WebSocket failures, fall back to polling. */
export const WS_FAILURES_BEFORE_POLL = 3;

/** Poll cadence in fallback mode (well under the per-viewer rate limit). */
export const POLL_INTERVAL_MS = 4000;

// ---------------------------------------------------------------------------
// Viewer preferences (anonymous, local-only — no identity linking).
// ---------------------------------------------------------------------------

export const FONT_SIZES = ["s", "m", "l"];

const PREFS_KEY = "streamdubber.captions.prefs";

const DEFAULT_PREFS = Object.freeze({ lang: null, size: "m", visible: true });

/** Clamp any stored/incoming size to a known value. */
export function clampSize(size) {
  return FONT_SIZES.includes(size) ? size : "m";
}

/** The next size in the S → M → L cycle. */
export function cycleSize(size) {
  const i = FONT_SIZES.indexOf(clampSize(size));
  return FONT_SIZES[(i + 1) % FONT_SIZES.length];
}

/** Load preferences; storage errors (sandboxed iframe) fall back to defaults. */
export function loadPrefs(storage) {
  try {
    const raw = storage.getItem(PREFS_KEY);
    if (!raw) {
      return { ...DEFAULT_PREFS };
    }
    const parsed = JSON.parse(raw);
    return {
      lang: isValidLang(parsed.lang) ? parsed.lang.toLowerCase() : null,
      size: clampSize(parsed.size),
      visible: parsed.visible !== false,
    };
  } catch {
    return { ...DEFAULT_PREFS };
  }
}

/** Persist preferences; failures are silently ignored (prefs are optional). */
export function savePrefs(storage, prefs) {
  try {
    storage.setItem(
      PREFS_KEY,
      JSON.stringify({
        lang: prefs.lang,
        size: clampSize(prefs.size),
        visible: prefs.visible !== false,
      }),
    );
  } catch {
    // Preferences are best-effort only.
  }
}

/**
 * Pick the language to display: viewer preference if still available, then
 * the broadcaster default, then the first available language.
 */
export function pickLanguage(available, preferred, fallback) {
  if (preferred && available.includes(preferred)) {
    return preferred;
  }
  if (fallback && available.includes(fallback)) {
    return fallback;
  }
  return available.length > 0 ? available[0] : null;
}
