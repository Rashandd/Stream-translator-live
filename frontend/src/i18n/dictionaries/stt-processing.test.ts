import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import en from "@/src/i18n/dictionaries/en.json";

const iconSource = readFileSync(
  fileURLToPath(new URL("../../components/icon.tsx", import.meta.url)),
  "utf8",
);
const iconMapBody = iconSource.match(/const icons[^{]*\{([\s\S]*?)\}/)?.[1] ?? "";
const registeredIcons = new Set(
  iconMapBody
    .split(",")
    .map((entry) => entry.trim())
    .filter(Boolean),
);

const panelSource = readFileSync(
  fileURLToPath(new URL("../../components/stt-processing-panel.tsx", import.meta.url)),
  "utf8",
);

const EMOJI = /[\u{1F300}-\u{1FAFF}\u{2600}-\u{27BF}]/u;

describe("subtitle processing card config (2.6b — docs/local-stt-plan.md §1)", () => {
  const processing = en.dashboard.captions.processing;

  it("exists with title, both modes and their badges", () => {
    expect(processing.title).toBeTruthy();
    expect(processing.subtitle).toBeTruthy();
    expect(processing.cloud.name).toBeTruthy();
    expect(processing.cloud.badge).toBeTruthy();
    expect(processing.local.name).toBeTruthy();
    // Plan kararı: yerel mod ilk sürümde "deneysel" rozetiyle çıkar.
    expect(processing.local.badge).toBe("Experimental");
  });

  it("uses registered icons for both mode cards", () => {
    for (const icon of [processing.cloud.icon, processing.local.icon]) {
      expect(registeredIcons.has(icon), `icon "${icon}" is not registered`).toBe(true);
    }
  });

  it("states the quota advantage with a multiplier placeholder", () => {
    expect(processing.local.quotaNoteTemplate).toContain("{multiplier}");
    expect(processing.cloud.quotaNote).toBeTruthy();
  });

  it("lists hardware requirements including the VRAM placeholder", () => {
    expect(processing.local.requirements.length).toBeGreaterThanOrEqual(3);
    expect(processing.local.requirements.some((r) => r.includes("{vram}"))).toBe(true);
    expect(processing.local.requirements.some((r) => r.includes("NVIDIA"))).toBe(true);
  });

  it("includes the mandatory cloud-fallback footnote (plan §1)", () => {
    expect(processing.fallbackNote).toBeTruthy();
    // Dipnot iki şeyi söylemek zorunda: kesinti olmaz + normal tarife.
    expect(processing.fallbackNote.toLowerCase()).toContain("cloud");
    expect(processing.fallbackNote.toLowerCase()).toContain("normal rate");
  });

  it("covers the hardware-test flow copy", () => {
    expect(processing.local.testButton).toBeTruthy();
    expect(processing.local.testQueuedNote).toBeTruthy();
    expect(processing.local.checkResultButton).toBeTruthy();
    expect(processing.local.noBenchmarkNote).toBeTruthy();
    expect(processing.local.benchmarkPassed).toBeTruthy();
    expect(processing.local.benchmarkFailed).toBeTruthy();
    expect(processing.local.planLocked).toBeTruthy();
  });

  it("has distinct error messages including benchmark gating", () => {
    const { errors } = processing;
    const values = [errors.unauthorized, errors.network, errors.saveFailed, errors.benchmarkRequired];
    expect(new Set(values).size).toBe(values.length);
    for (const value of values) expect(value).toBeTruthy();
  });

  it("panel talks to the real backend endpoints", () => {
    // Sözleşme: policy okuma, mod yazma ve komut bırakma uçları sabit.
    expect(panelSource).toContain('userFetch("/api/stt/policy")');
    expect(panelSource).toContain('userFetch("/api/stt/mode"');
    expect(panelSource).toContain('userFetch("/api/sdk/commands"');
  });

  it("pricing page explains on-device mode honestly (plan §1/§5.6)", () => {
    const onDevice = en.pricing.onDevice;
    expect(onDevice.title).toBeTruthy();
    expect(onDevice.badge).toBe("Experimental");
    expect(onDevice.points.length).toBeGreaterThanOrEqual(3);
    // Gereksinim + fallback tarifesi fiyat sayfasında da açık söylenir.
    expect(onDevice.points.some((p) => p.includes("NVIDIA"))).toBe(true);
    expect(onDevice.points.some((p) => p.toLowerCase().includes("normal rate"))).toBe(true);
  });

  it("contains no emoji anywhere in the processing copy", () => {
    const scan = (value: unknown): void => {
      if (typeof value === "string") {
        expect(EMOJI.test(value), `emoji found in: ${value}`).toBe(false);
      } else if (Array.isArray(value)) {
        value.forEach(scan);
      } else if (value && typeof value === "object") {
        Object.values(value).forEach(scan);
      }
    };
    scan(processing);
  });
});
