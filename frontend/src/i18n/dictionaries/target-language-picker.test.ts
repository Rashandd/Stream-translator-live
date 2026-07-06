import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import en from "@/src/i18n/dictionaries/en.json";

const panelSource = readFileSync(
  fileURLToPath(new URL("../../components/target-language-picker.tsx", import.meta.url)),
  "utf8",
);

const EMOJI = /[\u{1F300}-\u{1FAFF}\u{2600}-\u{27BF}]/u;

describe("target caption language picker copy (Faz 2.0 — DeepL dil matrisi)", () => {
  const picker = en.dashboard.captions.languagePicker;

  it("exists with title, subtitle template and loading state", () => {
    expect(picker.title).toBeTruthy();
    expect(picker.subtitle).toContain("{max}");
    expect(picker.unlimitedNote).toBeTruthy();
    expect(picker.loading).toBeTruthy();
  });

  it("covers auto-detect vs manual source language copy", () => {
    expect(picker.sourceTitle).toBeTruthy();
    expect(picker.autoDetectLabel).toBeTruthy();
    expect(picker.autoDetectHint).toBeTruthy();
    expect(picker.manualLabel).toBeTruthy();
  });

  it("states the selection count with a placeholder", () => {
    expect(picker.selectedCountTemplate).toContain("{count}");
    expect(picker.selectedCountTemplate).toContain("{max}");
  });

  it("has the formality warning note (roadmap 2.0 bullet 5)", () => {
    expect(picker.noFormalityNote).toBeTruthy();
  });

  it("has distinct, non-empty error messages", () => {
    const { errors } = picker;
    const values = [errors.unauthorized, errors.network, errors.saveFailed, errors.tooMany, errors.empty];
    expect(new Set(values).size).toBe(values.length);
    for (const value of values) expect(value).toBeTruthy();
  });

  it("talks to the real backend endpoint (GET + PUT)", () => {
    expect(panelSource).toContain('userFetch("/api/translate/languages")');
    expect(panelSource).toContain('userFetch("/api/translate/languages"');
    expect(panelSource).toContain('method: "PUT"');
  });

  it("enforces the plan cap client-side before saving", () => {
    expect(panelSource).toContain("max_languages");
    expect(panelSource).toContain("atLimit");
  });

  it("contains no emoji anywhere in the copy", () => {
    const scan = (value: unknown): void => {
      if (typeof value === "string") {
        expect(EMOJI.test(value), `emoji found in: ${value}`).toBe(false);
      } else if (Array.isArray(value)) {
        value.forEach(scan);
      } else if (value && typeof value === "object") {
        Object.values(value).forEach(scan);
      }
    };
    scan(picker);
  });
});
