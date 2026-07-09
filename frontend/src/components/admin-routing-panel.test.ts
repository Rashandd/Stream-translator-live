import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import en from "@/src/i18n/dictionaries/en.json";

const source = readFileSync(new URL("./admin-routing-panel.tsx", import.meta.url), "utf8");
const pageSource = readFileSync(
  new URL("../../app/[locale]/admin/(console)/routing/page.tsx", import.meta.url),
  "utf8",
);

describe("AdminRoutingPanel live wiring", () => {
  it("reads the live provider selection from the runtime config endpoint", () => {
    expect(source).toContain('adminFetch("/config")');
  });

  it("writes provider and fallback choices through the real admin config endpoint", () => {
    expect(source).toContain('adminFetch("/admin/config"');
    expect(source).toContain('method: "PUT"');
  });

  it("the routing page wires the panel instead of static disabled buttons", () => {
    expect(pageSource).toContain("<AdminRoutingPanel");
    expect(pageSource).not.toContain("option.active");
  });
});

describe("routing dictionary matches the backend's real registered provider slugs", () => {
  it("STT group offers deepgram and assemblyai as real (non-coming-soon) options", () => {
    const stt = en.admin.routing.groups.find((g) => g.key === "stt");
    expect(stt?.providerConfigKey).toBe("stt.provider");
    expect(stt?.fallbackConfigKey).toBe("stt.fallback");
    const real = stt?.options.filter((o) => o.status === "enabled").map((o) => o.slug);
    expect(real).toEqual(["deepgram", "assemblyai"]);
  });

  it("translation group offers deepl and google as real options", () => {
    const translation = en.admin.routing.groups.find((g) => g.key === "translation");
    expect(translation?.providerConfigKey).toBe("translate.provider");
    const real = translation?.options.filter((o) => o.status === "enabled").map((o) => o.slug);
    expect(real).toEqual(["deepl", "google"]);
  });

  it("tts group only offers elevenlabs as real — no second provider is actually implemented", () => {
    const tts = en.admin.routing.groups.find((g) => g.key === "tts");
    const real = tts?.options.filter((o) => o.status === "enabled").map((o) => o.slug);
    expect(real).toEqual(["elevenlabs"]);
  });
});
