import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./platform-accounts-panel.tsx", import.meta.url), "utf8");

describe("platform accounts panel", () => {
  it("loads live connection status from the user-scoped integrations endpoint", () => {
    expect(source).toContain('userFetch("/api/integrations")');
  });

  it("connects through the authorize redirect and disconnects through DELETE", () => {
    expect(source).toContain('userFetch(`/api/integrations/${provider}/authorize`)');
    expect(source).toContain('userFetch(`/api/integrations/${provider}`, { method: "DELETE" })');
  });

  it("disables the action button for accounts marked disabled (Kick placeholder)", () => {
    expect(source).toContain("account.disabled");
  });

  it("reflects live connected status instead of static mock data", () => {
    expect(source).toContain("findIntegration(integrations, account.provider)");
    expect(source).toContain("status?.connected");
  });
});
