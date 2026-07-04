import { describe, expect, it } from "vitest";
import {
  buildOverlayQueryParams,
  buildOverlayUrl,
  hexToRgba,
  OVERLAY_BUILDER_DEFAULTS,
} from "@/src/lib/overlay-builder-url";
import { OVERLAY_STYLE_DEFAULTS } from "@/src/lib/overlay-style";

describe("hexToRgba", () => {
  it("converts a hex color and 0-100 opacity into an rgba() string", () => {
    expect(hexToRgba("#000000", 60)).toBe("rgba(0,0,0,0.6)");
    expect(hexToRgba("#ffffff", 100)).toBe("rgba(255,255,255,1)");
    expect(hexToRgba("#ff6a4a", 0)).toBe("rgba(255,106,74,0)");
  });

  it("clamps opacity to the 0-100 range", () => {
    expect(hexToRgba("#000000", 150)).toBe("rgba(0,0,0,1)");
    expect(hexToRgba("#000000", -10)).toBe("rgba(0,0,0,0)");
  });
});

describe("buildOverlayQueryParams", () => {
  it("omits lang when the source language (no translation) is selected", () => {
    const params = buildOverlayQueryParams({ ...OVERLAY_BUILDER_DEFAULTS, lang: "" });
    expect(params.has("lang")).toBe(false);
  });

  it("includes lang when a target language is selected", () => {
    const params = buildOverlayQueryParams({ ...OVERLAY_BUILDER_DEFAULTS, lang: "tr" });
    expect(params.get("lang")).toBe("tr");
  });

  it("defaults produce the exact same style inputs as today's /o/[token] defaults", () => {
    const params = buildOverlayQueryParams(OVERLAY_BUILDER_DEFAULTS);
    expect(params.get("font")).toBe(OVERLAY_STYLE_DEFAULTS.font);
    expect(params.get("size")).toBe(OVERLAY_STYLE_DEFAULTS.size);
    expect(params.get("color")).toBe(OVERLAY_STYLE_DEFAULTS.color);
    expect(params.get("bg")).toBe(OVERLAY_STYLE_DEFAULTS.bg);
    expect(params.get("position")).toBe(OVERLAY_STYLE_DEFAULTS.position);
  });
});

describe("buildOverlayUrl", () => {
  it("appends the generated query string onto the template URL", () => {
    const url = buildOverlayUrl("https://overlay.strovak.com/o/•••••••", OVERLAY_BUILDER_DEFAULTS);
    expect(url.startsWith("https://overlay.strovak.com/o/•••••••?")).toBe(true);
    expect(url).toContain(`position=${OVERLAY_STYLE_DEFAULTS.position}`);
  });
});
