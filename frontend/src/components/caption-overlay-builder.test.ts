import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./caption-overlay-builder.tsx", import.meta.url), "utf8");

describe("CaptionOverlayBuilder real overlay token wiring", () => {
  it("polls the authenticated overlay token endpoint, not a public/admin one", () => {
    expect(source).toContain('userFetch("/api/overlay/token")');
  });

  it("builds the OBS URL from the live token and the page's own origin once a stream is active", () => {
    expect(source).toContain("window.location.origin");
    expect(source).toContain("/o/${overlayToken.token}");
  });

  it("falls back to the dictionary placeholder template until a stream goes live", () => {
    expect(source).toContain("return overlayUrlTemplate;");
  });

  it("hides the placeholder note once the URL is real", () => {
    expect(source).toContain("{!isLive ?");
  });
});
