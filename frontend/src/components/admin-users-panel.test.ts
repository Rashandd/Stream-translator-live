import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./admin-users-panel.tsx", import.meta.url), "utf8");
const pageSource = readFileSync(
  new URL("../../app/[locale]/admin/(console)/users/page.tsx", import.meta.url),
  "utf8",
);

describe("AdminUsersPanel live wiring", () => {
  it("loads users through the admin-token-authenticated endpoint", () => {
    expect(source).toContain('adminFetch(path)');
    expect(source).toContain('"/admin/users"');
  });

  it("searches via the backend's ?q= query param, not client-side filtering only", () => {
    expect(source).toContain("encodeURIComponent(q.trim())");
  });

  it("changes a user's plan through the real admin endpoint", () => {
    expect(source).toContain("adminFetch(`/admin/users/${id}/plan`");
    expect(source).toContain('method: "PUT"');
  });

  it("populates the plan dropdown from the public plan catalog", () => {
    expect(source).toContain("${API_URL}/api/plans");
  });

  it("the admin users page wires the panel instead of a static table", () => {
    expect(pageSource).toContain("<AdminUsersPanel");
    expect(pageSource).not.toContain("users.rows");
  });
});
