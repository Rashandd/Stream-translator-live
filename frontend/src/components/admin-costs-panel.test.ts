import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./admin-costs-panel.tsx", import.meta.url), "utf8");
const pageSource = readFileSync(
  new URL("../../app/[locale]/admin/(console)/costs/page.tsx", import.meta.url),
  "utf8",
);

describe("AdminCostsPanel live wiring", () => {
  it("loads the real cost summary through the admin-authenticated endpoint", () => {
    expect(source).toContain('adminFetch("/admin/costs/summary")');
  });

  it("shows usage hours, estimated cost, estimated revenue, and user count", () => {
    expect(source).toContain("total_usage_hours");
    expect(source).toContain("estimated_cost_cents");
    expect(source).toContain("estimated_monthly_revenue_cents");
    expect(source).toContain("user_count");
  });

  it("the admin costs page wires the panel instead of static $0 metric cards", () => {
    expect(pageSource).toContain("<AdminCostsPanel");
    expect(pageSource).not.toContain("costs.metrics");
  });
});
