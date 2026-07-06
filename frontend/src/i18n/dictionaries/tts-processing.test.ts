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
  fileURLToPath(new URL("../../components/tts-processing-panel.tsx", import.meta.url)),
  "utf8",
);

const EMOJI = /[\u{1F300}-\u{1FAFF}\u{2600}-\u{27BF}]/u;

describe("tts processing card config (Faz 2.2b — sdk_link_api.rs/stt_policy'nin TTS aynası)", () => {
  const processing = en.dashboard.voice.processing;

  it("exists with title, both modes and their badges", () => {
    expect(processing.title).toBeTruthy();
    expect(processing.subtitle).toBeTruthy();
    expect(processing.cloud.name).toBeTruthy();
    expect(processing.cloud.badge).toBeTruthy();
    expect(processing.local.name).toBeTruthy();
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

  it("includes the mandatory cloud-fallback footnote", () => {
    expect(processing.fallbackNote).toBeTruthy();
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

  it("panel talks to the real TTS backend endpoints (not STT's)", () => {
    expect(panelSource).toContain('userFetch("/api/tts/policy")');
    expect(panelSource).toContain('userFetch("/api/tts/mode"');
    expect(panelSource).toContain('command: "tts_benchmark"');
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
