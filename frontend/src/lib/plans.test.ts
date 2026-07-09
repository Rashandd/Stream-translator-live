import { describe, expect, it } from "vitest";
import { formatHours, formatPriceCents, mergeLivePlans, type LivePlan } from "@/src/lib/plans";

describe("formatPriceCents", () => {
  it("renders zero as a bare $0", () => {
    expect(formatPriceCents(0)).toBe("$0");
  });

  it("renders cents as a two-decimal dollar amount", () => {
    expect(formatPriceCents(999)).toBe("$9.99");
    expect(formatPriceCents(9999)).toBe("$99.99");
  });
});

describe("formatHours", () => {
  it("renders a positive count as a monthly allowance", () => {
    expect(formatHours(30)).toBe("30 hours a month");
  });

  it("renders -1 as unlimited", () => {
    expect(formatHours(-1)).toBe("Unlimited hours");
  });
});

const dictionaryPlans = [
  {
    name: "Free",
    price: "$0",
    summary: "Try live captions on your next stream.",
    features: ["Live captions in every language", "5 hours a month", "Watermark"],
    popular: false,
  },
  {
    name: "Streamer",
    price: "$49.99",
    summary: "Everything.",
    features: ["Captions in every language", "100 hours a month", "All platforms"],
    popular: false,
  },
];

describe("mergeLivePlans", () => {
  it("falls back to the static dictionary plans when the API is unreachable", () => {
    expect(mergeLivePlans(dictionaryPlans, null)).toBe(dictionaryPlans);
  });

  it("overrides price and the leading hours figure by matching plan name case-insensitively", () => {
    const live: LivePlan[] = [
      {
        name: "streamer",
        price_cents: 9999,
        monthly_hours: 200,
        max_languages: -1,
        max_platforms: -1,
        has_watermark: false,
        has_voice_clone: true,
      },
    ];
    const merged = mergeLivePlans(dictionaryPlans, live);
    const streamer = merged.find((p) => p.name === "Streamer");
    expect(streamer?.price).toBe("$99.99");
    expect(streamer?.features).toContain("200 hours a month");
    // Unmatched plan (Free) keeps its static values untouched.
    const free = merged.find((p) => p.name === "Free");
    expect(free?.price).toBe("$0");
    expect(free?.features).toContain("5 hours a month");
  });

  it("renders -1 (unlimited) plans without a stale hour count", () => {
    const live: LivePlan[] = [
      {
        name: "streamer",
        price_cents: 9999,
        monthly_hours: -1,
        max_languages: -1,
        max_platforms: -1,
        has_watermark: false,
        has_voice_clone: true,
      },
    ];
    const merged = mergeLivePlans(dictionaryPlans, live);
    const streamer = merged.find((p) => p.name === "Streamer");
    expect(streamer?.features.some((f) => f.startsWith("Unlimited hours"))).toBe(true);
  });

  it("leaves a plan untouched when its name has no live match", () => {
    const live: LivePlan[] = [
      {
        name: "pro",
        price_cents: 2499,
        monthly_hours: 80,
        max_languages: 10,
        max_platforms: -1,
        has_watermark: false,
        has_voice_clone: true,
      },
    ];
    const merged = mergeLivePlans(dictionaryPlans, live);
    expect(merged).toEqual(dictionaryPlans);
  });
});
