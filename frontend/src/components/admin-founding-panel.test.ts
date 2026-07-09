import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./admin-founding-panel.tsx", import.meta.url), "utf8");

describe("admin founding applications panel", () => {
  it("loads applications and approves/revokes through admin-scoped founding endpoints", () => {
    expect(source).toContain('adminFetch("/admin/founding/applications")');
    expect(source).toContain("adminFetch(`/admin/founding/${userId}/${action}`");
    expect(source).toContain('method: "POST"');
  });

  it("disables approve once approved, and revoke unless approved", () => {
    expect(source).toContain('app.status === "approved"');
    expect(source).toContain('app.status !== "approved"');
  });

  it("treats 401/403 as an expired admin session", () => {
    expect(source).toContain("res.status === 401 || res.status === 403");
    expect(source).toContain("setUnauthorized(true)");
  });
});
