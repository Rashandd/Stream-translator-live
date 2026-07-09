import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./billing-plan-card.tsx", import.meta.url), "utf8");
const pageSource = readFileSync(
  new URL("../../app/[locale]/dashboard/billing/page.tsx", import.meta.url),
  "utf8",
);

describe("BillingPlanCard live usage + checkout wiring", () => {
  it("loads the authenticated usage summary, not a public/admin endpoint", () => {
    expect(source).toContain('userFetch("/api/usage/summary")');
  });

  it("starts checkout for the next-priced plan via the real billing endpoint", () => {
    expect(source).toContain('userFetch("/api/billing/checkout"');
    expect(source).toContain("plan: nextPlan.name");
  });

  it("redirects the browser to the Stripe-hosted checkout URL on success", () => {
    expect(source).toContain("window.location.assign(data.url)");
  });

  it("treats an unconfigured Stripe price (404/503) as a distinct, user-visible state", () => {
    expect(source).toContain('res.status === 404 || res.status === 503');
    expect(source).toContain("checkoutUnavailable");
  });

  it("disables the upgrade action once the user is already on the top plan", () => {
    expect(source).toContain("!nextPlan");
    expect(source).toContain("topPlanLabel");
  });

  it("the billing page wires the card to the live plan catalog, not just static dictionary text", () => {
    expect(pageSource).toContain("fetchLivePlans()");
    expect(pageSource).toContain("<BillingPlanCard");
  });
});
