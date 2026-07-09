import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./admin-promo-panel.tsx", import.meta.url), "utf8");

describe("admin founding promo panel", () => {
  it("loads and saves through the runtime config endpoints", () => {
    expect(source).toContain('adminFetch("/config")');
    expect(source).toContain('adminFetch("/admin/config"');
    expect(source).toContain('method: "PUT"');
  });

  it("writes the gate, promo-enabled, and promo-message config keys", () => {
    expect(source).toContain('"founding.gate.enabled"');
    expect(source).toContain('"promo.founding.enabled"');
    expect(source).toContain('"promo.founding.message"');
  });

  it("treats 401/403 as an expired admin session", () => {
    expect(source).toContain("res.status === 401 || res.status === 403");
    expect(source).toContain("setUnauthorized(true)");
  });
});
