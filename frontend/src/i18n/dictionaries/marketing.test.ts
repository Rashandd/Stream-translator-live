import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import en from "@/src/i18n/dictionaries/en.json";

/** Icon names registered in the shared <Icon> map. */
const iconSource = readFileSync(
  fileURLToPath(new URL("../../components/icon.tsx", import.meta.url)),
  "utf8",
);
const homePageSource = readFileSync(
  fileURLToPath(new URL("../../../app/[locale]/page.tsx", import.meta.url)),
  "utf8",
);
const solutionsPageSource = readFileSync(
  fileURLToPath(new URL("../../../app/[locale]/solutions/page.tsx", import.meta.url)),
  "utf8",
);
const distributionPageSource = readFileSync(
  fileURLToPath(new URL("../../../app/[locale]/dashboard/distribution/page.tsx", import.meta.url)),
  "utf8",
);
const platformsPageSource = readFileSync(
  fileURLToPath(new URL("../../components/platform-accounts-panel.tsx", import.meta.url)),
  "utf8",
);
const iconMapBody = iconSource.match(/const icons[^{]*\{([\s\S]*?)\}/)?.[1] ?? "";
const registeredIcons = new Set(
  iconMapBody
    .split(",")
    .map((entry) => entry.trim())
    .filter(Boolean),
);

function expectIconRegistered(name: string) {
  expect(registeredIcons.has(name), `icon "${name}" is not registered in icon.tsx`).toBe(true);
}

describe("landing edge section (the Google-can't wedge)", () => {
  const edge = (en.home as Record<string, unknown>).edge as
    | { eyebrow: string; title: string; items: { title: string; body: string; icon: string }[] }
    | undefined;
  const homeText = JSON.stringify(en.home).toLowerCase();

  it("exists with a headline and at least four differentiators", () => {
    expect(edge).toBeTruthy();
    expect(edge?.eyebrow).toBeTruthy();
    expect(edge?.title).toBeTruthy();
    expect(edge?.items.length).toBeGreaterThanOrEqual(4);
  });

  it("uses only registered icons and non-empty copy", () => {
    for (const item of edge?.items ?? []) {
      expect(item.title).toBeTruthy();
      expect(item.body).toBeTruthy();
      expectIconRegistered(item.icon);
    }
  });

  it("presents the two caption ways and two dub ways on the landing page", () => {
    expect(homeText).toContain("captions baked into your scene");
    expect(homeText).toContain("captions viewers switch on");
    expect(homeText).toContain("dubbed audio on youtube");
    expect(homeText).toContain("a dubbed channel per language");
  });

  it("does not lead the landing page with the machine-local video claim", () => {
    expect(homeText).not.toContain("your video never leaves your machine");
  });

  it("does not render phase/status badges on the landing edge cards", () => {
    expect(homeText).not.toContain("phase 1 active");
    expect(homeText).not.toContain("phase 2 active");
    expect(homeText).not.toContain("phase 3");
    expect(homePageSource).not.toContain("phaseLabel");
    expect(homePageSource).not.toContain("config.statusLabels[item.status");
  });
});

describe("dashboard distribution dictionary", () => {
  const distribution = en.dashboard.distribution;

  it("explains support per platform (YouTube / Twitch / Kick) instead of abstract mode cards", () => {
    const platforms = distribution.platformSupport.rows.map((row) => row.platform);
    expect(platforms).toEqual(["YouTube", "Twitch", "Kick"]);
    for (const row of distribution.platformSupport.rows) {
      expect(row.captions).toBeTruthy();
      expect(row.dubbing).toBeTruthy();
    }
  });

  it("renders platform brand marks for YouTube, Twitch, and Kick cards", () => {
    expect(distributionPageSource).toContain("brandForPlatform(row.platform)");
    expect(platformsPageSource).toContain("brandForPlatform(account.provider)");
    expect(distributionPageSource).toContain("<BrandMark");
    expect(platformsPageSource).toContain("<BrandMark");
  });

  it("delivers Kick captions through a browser extension", () => {
    const kick = distribution.platformSupport.rows.find((row) => row.platform === "Kick");
    expect(kick?.captions.toLowerCase()).toContain("browser extension");
  });

  it("keeps SDK relay primary while allowing Streamlabs only as the optional service path", () => {
    const distributionText = JSON.stringify(distribution).toLowerCase();
    expect(distributionText).not.toContain("restream");
    expect(distributionText).not.toContain("castr");
    expect(distribution.methodSelection.defaultMethodId).toBe("sdk-relay");
    expect(distribution.methodSelection.methods[0]?.id).toBe("sdk-relay");
    const streamlabs = distribution.methodSelection.methods.find((method) => method.provider === "streamlabs");
    expect(streamlabs?.badge.toLowerCase()).toContain("optional");
  });

  it("does not frame cloud routing as only an upload workaround", () => {
    const distributionText = JSON.stringify(distribution).toLowerCase();
    expect(distributionText).not.toContain("cannot handle multiple platforms");
    expect(distributionText).not.toContain("upload can't handle");
    expect(distributionText).not.toContain("make sure your upload");
  });

  it("warns local relay users about the 25 Mbps upstream recommendation", () => {
    expect(distribution.methodSelection.uploadWarning.metric).toContain("25 Mbps");
    expect(distribution.methodSelection.uploadWarning.body.toLowerCase()).toContain("upstream");
  });
});

describe("solutions dictionary", () => {
  const solutions = (en as Record<string, unknown>).solutions as
    | {
        hero: { eyebrow: string; title: string; summary: string };
        items: { title: string; body: string; icon: string; audience: string; status: string }[];
        cta?: { label: string; href: string };
      }
    | undefined;

  it("backs the /solutions nav link", () => {
    const navHasSolutions = en.nav.items.some((item) => item.href === "/solutions");
    expect(navHasSolutions).toBe(true);
    expect(solutions).toBeTruthy();
    expect(solutions?.hero.title).toBeTruthy();
    expect(solutions?.hero.summary).toBeTruthy();
  });

  it("covers streamer-first plus business use-cases with valid audiences", () => {
    const items = solutions?.items ?? [];
    expect(items.length).toBeGreaterThanOrEqual(5);

    // Streamers are the primary audience: the first card targets creators.
    expect(["creators", "both"]).toContain(items[0]?.audience);
    // At least one card must speak to business (the palabra-style secondary market).
    expect(items.some((item) => item.audience === "business" || item.audience === "both")).toBe(true);

    const validAudiences = new Set(["creators", "business", "both"]);
    const validStatuses = new Set(["enabled", "beta", "waitlist", "coming_soon", "disabled"]);
    for (const item of items) {
      expect(item.title).toBeTruthy();
      expect(item.body).toBeTruthy();
      expect(validAudiences.has(item.audience)).toBe(true);
      expect(validStatuses.has(item.status)).toBe(true);
      expectIconRegistered(item.icon);
    }
  });

  it("does not present Phase-2 business use-cases as live products", () => {
    // Roadmap rule: the palabra-style catalog is out of Phase 1 and must show a
    // coming_soon/waitlist/beta status — never "enabled" — for business surfaces.
    for (const item of solutions?.items ?? []) {
      if (item.audience === "business") {
        expect(item.status).not.toBe("enabled");
      }
    }
  });

  it("does not render notify-me forms on the public solutions page", () => {
    expect(solutionsPageSource).not.toContain("WaitlistCta");
    expect(solutionsPageSource).not.toContain("waitlistCta");
  });

  it("does not render top-right status badges on solution cards", () => {
    expect(solutionsPageSource).not.toContain("StatusPill");
    expect(solutionsPageSource).not.toContain("config.statusLabels");
    expect(solutionsPageSource).not.toContain("CheckCircle2");
  });
});
