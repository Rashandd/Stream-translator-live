import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import en from "@/src/i18n/dictionaries/en.json";

describe("how it works page", () => {
  it("the route file exists", () => {
    const path = fileURLToPath(
      new URL("../../../app/[locale]/how-it-works/page.tsx", import.meta.url),
    );
    expect(existsSync(path)).toBe(true);
  });

  it("is reachable from the public nav and footer", () => {
    const navHrefs = en.nav.items.map((item) => item.href);
    expect(navHrefs).toContain("/how-it-works");
    const footerHrefs = en.footer.columns.flatMap((col) => col.items.map((item) => item.href));
    expect(footerHrefs).toContain("/how-it-works");
  });

  it("walks through install, pairing, OBS overlay, and going live", () => {
    const steps = en.howItWorks.steps.map((s) => `${s.title} ${s.body}`.toLowerCase());
    expect(steps.some((s) => s.includes("download") || s.includes("app"))).toBe(true);
    expect(steps.some((s) => s.includes("pair") || s.includes("code"))).toBe(true);
    expect(steps.some((s) => s.includes("obs") || s.includes("overlay"))).toBe(true);
    expect(steps.some((s) => s.includes("live"))).toBe(true);
  });

  it("states the video-stays-local privacy claim explicitly", () => {
    expect(en.howItWorks.privacyNote.toLowerCase()).toContain("video");
  });

  it("answers plan-limit and OBS-requirement questions in the FAQ", () => {
    const faq = en.howItWorks.faq.map((f) => `${f.question} ${f.answer}`.toLowerCase());
    expect(faq.some((f) => f.includes("hour") || f.includes("plan"))).toBe(true);
    expect(faq.some((f) => f.includes("obs"))).toBe(true);
  });

  it("sends the CTA to signup, not the dashboard demo", () => {
    expect(en.howItWorks.primaryCta.href).toBe("/signup");
  });
});
