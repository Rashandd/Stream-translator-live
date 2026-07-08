import { describe, expect, it } from "vitest";
import en from "@/src/i18n/dictionaries/en.json";

describe("legal pages dictionary contract", () => {
  it("ships every legal document linked from auth", () => {
    const linkedSlugs = [
      ...en.auth.signup.requiredConsent.links,
      ...en.auth.signup.optionalConsent.links,
    ].map((link) => link.href.split("/").pop());
    const pageSlugs = en.legal.pages.map((page) => page.slug);

    expect(pageSlugs).toEqual(linkedSlugs);
  });

  it("includes the core digital compliance surfaces", () => {
    const titles = en.legal.pages.map((page) => page.title);

    expect(titles).toContain("Terms of Service");
    expect(titles).toContain("Privacy Policy");
    expect(titles).toContain("KVKK Disclosure Notice");
    expect(titles).toContain("Cookie Policy");
    expect(titles).toContain("Data Processing Addendum");
    expect(titles).toContain("Electronic Communications Consent");
  });

  it("keeps legal documents substantial enough to avoid placeholder pages", () => {
    for (const page of en.legal.pages) {
      expect(page.summary.length, page.slug).toBeGreaterThan(40);
      expect(page.sections.length, page.slug).toBeGreaterThanOrEqual(3);
    }
  });
});
