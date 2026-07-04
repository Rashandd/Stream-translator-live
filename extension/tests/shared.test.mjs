// Unit tests for the pure logic in js/shared.js — run with `node --test`.
import { test } from "node:test";
import assert from "node:assert/strict";

import {
  EBS_BASE_PRODUCTION,
  DELAYED_AFTER_MS,
  captionTextFor,
  captionsWsUrl,
  clampSize,
  computeStatus,
  cycleSize,
  isValidLang,
  languagesOf,
  loadPrefs,
  nextDelayMs,
  parseSubtitle,
  pickLanguage,
  resolveEbsBase,
  sanitizeLanguages,
  savePrefs,
  stateUrl,
} from "../js/shared.js";

function subtitle(overrides = {}) {
  return {
    type: "subtitle",
    session_id: "s1",
    sequence: 7,
    source: { lang: "tr", text: "merhaba", is_final: true },
    translations: [
      { lang: "en", text: "hello" },
      { lang: "de", text: "hallo" },
    ],
    ts_start_ms: 0,
    ts_end_ms: 100,
    ...overrides,
  };
}

test("resolveEbsBase: production host ignores overrides", () => {
  assert.equal(
    resolveEbsBase("abc123.ext-twitch.tv", "?ebs=https://evil.example"),
    EBS_BASE_PRODUCTION,
  );
});

test("resolveEbsBase: localhost may override for the dev rig", () => {
  assert.equal(
    resolveEbsBase("localhost", "?ebs=http://localhost:8080/"),
    "http://localhost:8080",
  );
  assert.equal(resolveEbsBase("localhost", ""), EBS_BASE_PRODUCTION);
  // Non-http schemes are rejected.
  assert.equal(
    resolveEbsBase("localhost", "?ebs=javascript:alert(1)"),
    EBS_BASE_PRODUCTION,
  );
});

test("URL builders encode the channel id and switch protocols", () => {
  assert.equal(
    stateUrl("https://ext.example", "ch/1"),
    "https://ext.example/ext/twitch/state/ch%2F1",
  );
  const ws = captionsWsUrl("https://ext.example", "ch1", "a.b?c");
  assert.ok(ws.startsWith("wss://ext.example/ext/twitch/captions/ch1?token="));
  assert.ok(!ws.includes("?c"), "token must be URL-encoded");
  assert.ok(captionsWsUrl("http://localhost:8080", "c", "t").startsWith("ws://"));
});

test("language validation and sanitization", () => {
  assert.ok(isValidLang("tr"));
  assert.ok(isValidLang("pt-BR"));
  assert.ok(!isValidLang("<img>"));
  assert.ok(!isValidLang(""));
  assert.ok(!isValidLang(42));
  assert.deepEqual(sanitizeLanguages(["TR", "en", "en", "x!", null]), ["tr", "en"]);
  assert.deepEqual(sanitizeLanguages("nope"), []);
  const many = Array.from({ length: 40 }, (_, i) => `l${i}`.slice(0, 2) + String(i % 10));
  assert.ok(sanitizeLanguages(many).length <= 16);
});

test("parseSubtitle accepts well-formed frames only", () => {
  const parsed = parseSubtitle(JSON.stringify(subtitle()));
  assert.equal(parsed.source.text, "merhaba");
  assert.equal(parsed.sequence, 7);
  assert.equal(parsed.translations.length, 2);

  assert.equal(parseSubtitle("{not json"), null);
  assert.equal(parseSubtitle(JSON.stringify({ type: "pong", ts: 1 })), null);
  assert.equal(
    parseSubtitle(JSON.stringify(subtitle({ source: { lang: "!!", text: "x", is_final: true } }))),
    null,
  );
  // Malformed translation entries are dropped, valid ones kept.
  const mixed = parseSubtitle(
    JSON.stringify(subtitle({ translations: [{ lang: "en", text: "ok" }, { lang: 5 }] })),
  );
  assert.deepEqual(mixed.translations, [{ lang: "en", text: "ok" }]);
});

test("caption selection by language", () => {
  const s = parseSubtitle(JSON.stringify(subtitle()));
  assert.deepEqual(languagesOf(s), ["tr", "en", "de"]);
  assert.equal(captionTextFor(s, "tr"), "merhaba");
  assert.equal(captionTextFor(s, "en"), "hello");
  assert.equal(captionTextFor(s, "fr"), null);
});

test("status decision table", () => {
  assert.equal(computeStatus({ configured: false, live: false, connected: false, lastCaptionAgeMs: null }), "not_configured");
  assert.equal(computeStatus({ configured: true, live: false, connected: false, lastCaptionAgeMs: null }), "offline");
  assert.equal(computeStatus({ configured: true, live: true, connected: false, lastCaptionAgeMs: null }), "connecting");
  assert.equal(computeStatus({ configured: true, live: true, connected: true, lastCaptionAgeMs: null }), "waiting");
  assert.equal(computeStatus({ configured: true, live: true, connected: true, lastCaptionAgeMs: 100 }), "live");
  assert.equal(
    computeStatus({ configured: true, live: true, connected: true, lastCaptionAgeMs: DELAYED_AFTER_MS + 1 }),
    "delayed",
  );
});

test("backoff grows exponentially and caps", () => {
  assert.equal(nextDelayMs(0), 1000);
  assert.equal(nextDelayMs(1), 2000);
  assert.equal(nextDelayMs(2), 4000);
  assert.equal(nextDelayMs(10), 30000);
  assert.equal(nextDelayMs(1000), 30000, "huge attempts must not overflow");
});

test("size clamp and cycle", () => {
  assert.equal(clampSize("xl"), "m");
  assert.equal(cycleSize("s"), "m");
  assert.equal(cycleSize("m"), "l");
  assert.equal(cycleSize("l"), "s");
  assert.equal(cycleSize("bogus"), "l");
});

test("preferences survive a storage roundtrip and reject junk", () => {
  const fake = new Map();
  const storage = {
    getItem: (k) => (fake.has(k) ? fake.get(k) : null),
    setItem: (k, v) => fake.set(k, v),
  };
  savePrefs(storage, { lang: "en", size: "l", visible: false });
  assert.deepEqual(loadPrefs(storage), { lang: "en", size: "l", visible: false });

  fake.set("streamdubber.captions.prefs", '{"lang":"<x>","size":"huge","visible":1}');
  assert.deepEqual(loadPrefs(storage), { lang: null, size: "m", visible: true });

  const broken = {
    getItem: () => {
      throw new Error("denied");
    },
    setItem: () => {
      throw new Error("denied");
    },
  };
  assert.deepEqual(loadPrefs(broken), { lang: null, size: "m", visible: true });
  savePrefs(broken, { lang: "en", size: "m", visible: true }); // must not throw
});

test("language pick order: preference, broadcaster default, first", () => {
  assert.equal(pickLanguage(["tr", "en"], "en", "tr"), "en");
  assert.equal(pickLanguage(["tr", "en"], "fr", "en"), "en");
  assert.equal(pickLanguage(["tr", "en"], null, "de"), "tr");
  assert.equal(pickLanguage([], "en", "tr"), null);
});
