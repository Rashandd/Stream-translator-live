import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import en from "@/src/i18n/dictionaries/en.json";

/** app/[locale] altında bir href'in karşılığı olan page.tsx var mı? */
function pageExists(href: string): boolean {
  const rel = href === "/" ? "page.tsx" : `${href.replace(/^\//, "")}/page.tsx`;
  const candidates = [
    new URL(`../../../app/[locale]/${rel}`, import.meta.url),
    // Route group'lar ((console) gibi) href'te görünmez — bilinenleri dene.
    new URL(`../../../app/[locale]/admin/(console)/${rel.replace(/^admin\//, "")}`, import.meta.url),
  ];
  if (href.startsWith("/legal/")) {
    candidates.push(new URL("../../../app/[locale]/legal/[slug]/page.tsx", import.meta.url));
  }
  return candidates.some((url) => existsSync(fileURLToPath(url)));
}

function collectHrefs(node: unknown, hrefs: Set<string>) {
  if (Array.isArray(node)) {
    for (const item of node) collectHrefs(item, hrefs);
    return;
  }
  if (node && typeof node === "object") {
    for (const [key, value] of Object.entries(node)) {
      if ((key === "href" || key === "ctaHref") && typeof value === "string" && value.startsWith("/")) {
        hrefs.add(value);
      } else {
        collectHrefs(value, hrefs);
      }
    }
  }
}

describe("public navigation and links", () => {
  it("every internal href in the dictionary resolves to an existing page", () => {
    const hrefs = new Set<string>();
    collectHrefs(en, hrefs);
    const missing = [...hrefs].filter((href) => !pageExists(href));
    expect(missing, `dead links: ${missing.join(", ")}`).toEqual([]);
  });

  it("the public header offers both a sign-in link and a primary CTA", () => {
    expect(en.nav.signIn?.href).toBeTruthy();
    expect(en.nav.signIn?.label).toBeTruthy();
    expect(en.nav.cta.href).toBeTruthy();
  });

  it("the primary marketing CTAs send a new visitor to create an account, not into the dashboard demo", () => {
    expect(en.nav.cta.href).toBe("/signup");
    expect(en.home.hero.primaryCta.href).toBe("/signup");
    expect(en.pricing.ctaHref).toBe("/signup");
  });

  it("pricing is reachable from the public nav", () => {
    const items = en.nav.items.map((item) => item.href);
    expect(items).toContain("/pricing");
  });

  it("security is not shown in the public header nav", () => {
    const items = en.nav.items.map((item) => item.href);
    expect(items).not.toContain("/security");
  });
});

describe("pricing page config", () => {
  it("lists the four plans with a free tier first", () => {
    expect(en.pricing.plans.length).toBeGreaterThanOrEqual(4);
    expect(en.pricing.plans[0].price).toContain("$0");
    for (const plan of en.pricing.plans) {
      expect(plan.name).toBeTruthy();
      expect(plan.price).toBeTruthy();
      expect(plan.features.length).toBeGreaterThanOrEqual(2);
    }
  });

  it("describes paid plans as lower-plan inclusions plus extras", () => {
    const starter = en.pricing.plans.find((plan) => plan.name === "Starter");
    const pro = en.pricing.plans.find((plan) => plan.name === "Pro");
    const streamer = en.pricing.plans.find((plan) => plan.name === "Streamer");
    expect(starter?.features[0]).toBe("Everything in Free, plus:");
    expect(pro?.features[0]).toBe("Everything in Starter, plus:");
    expect(streamer?.features[0]).toBe("Everything in Pro, plus:");
  });

  it("does not show pricing catalog fine print or on-device promo copy", () => {
    expect("note" in en.pricing).toBe(false);
    expect("onDevice" in en.pricing).toBe(false);
    expect(JSON.stringify(en.pricing)).not.toContain("Prices and limits shown here");
  });
});

describe("landing comprehension for a new customer", () => {
  it("hero leads with the free plan", () => {
    expect(en.home.hero.primaryCta.label.toLowerCase()).toContain("free");
    expect(en.home.hero.note).toBeTruthy();
  });

  it("edge cards each declare an availability status", () => {
    const known = new Set(Object.keys(en.statusLabels));
    for (const item of en.home.edge.items) {
      expect(known.has(item.status), `unknown status "${item.status}" on "${item.title}"`).toBe(true);
    }
    // En az bir kart bugün çalışıyor olmalı, en az biri de yol haritasında.
    const statuses = en.home.edge.items.map((item) => item.status);
    expect(statuses).toContain("enabled");
    expect(statuses.some((s) => s !== "enabled")).toBe(true);
  });

  it("keeps engineering jargon out of user-facing copy", () => {
    const surfaces = JSON.stringify({ home: en.home, dashboard: en.dashboard.overview }).toLowerCase();
    expect(surfaces).not.toContain("hard-coded");
    expect(surfaces).not.toContain("config-driven");
    expect(surfaces).not.toContain("driven by config");
  });
});

describe("auth flow completeness", () => {
  it("signup and password reset copy exist", () => {
    expect(en.auth.signup.title).toBeTruthy();
    expect(en.auth.signup.submitLabel).toBeTruthy();
    expect(en.auth.reset.title).toBeTruthy();
  });
});

describe("security page config", () => {
  it("makes the core privacy claim: video stays local, only audio to cloud", () => {
    const text = JSON.stringify(en.security).toLowerCase();
    expect(text).toContain("video");
    expect(text).toContain("audio");
    expect(en.security.claims.length).toBeGreaterThanOrEqual(4);
  });
});
