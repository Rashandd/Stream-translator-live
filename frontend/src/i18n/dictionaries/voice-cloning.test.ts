import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import en from "@/src/i18n/dictionaries/en.json";

const studioSource = readFileSync(
  fileURLToPath(new URL("../../components/voice-clone-studio.tsx", import.meta.url)),
  "utf8",
);
const adminPanelSource = readFileSync(
  fileURLToPath(new URL("../../components/admin-voice-models-panel.tsx", import.meta.url)),
  "utf8",
);

const EMOJI = /[\u{1F300}-\u{1FAFF}\u{2600}-\u{27BF}]/u;

describe("voice cloning studio + admin approval queue (Faz 2.1)", () => {
  const { voice } = en.dashboard;
  const { voiceModels } = en.admin;

  it("studio talks to the real clone + list endpoints", () => {
    expect(studioSource).toContain('userFetch("/api/voice/clone"');
    expect(studioSource).toContain('method: "POST"');
    expect(studioSource).toContain('userFetch("/api/voice/models")');
  });

  it("studio supports both microphone recording and file upload", () => {
    expect(studioSource).toContain("getUserMedia");
    expect(studioSource).toContain("MediaRecorder");
    expect(studioSource).toContain('type="file"');
  });

  it("studio sends language_mode + source_languages and fetches the language catalog", () => {
    expect(studioSource).toContain('userFetch("/api/translate/catalog")');
    expect(studioSource).toContain("language_mode: languageMode");
    expect(studioSource).toContain("source_languages: sourceLangs");
  });

  it("admin panel talks to the real queue + approve/reject/assign endpoints", () => {
    expect(adminPanelSource).toContain('adminFetch("/admin/voice/models")');
    expect(adminPanelSource).toContain("/admin/voice/models/${id}/${action}");
    expect(adminPanelSource).toContain("/admin/voice/models/${id}/assign");
    expect(adminPanelSource).toContain("target_language");
  });

  it("has language mode copy for all three modes on both broadcaster and admin sides", () => {
    const modes = ["single_to_single", "multi_to_single", "single_to_multi"] as const;
    for (const mode of modes) {
      expect(voice.form.languageModeOptions[mode].title).toBeTruthy();
      expect(voice.form.languageModeOptions[mode].description).toBeTruthy();
      expect(voice.modelMeta.languageModeLabels[mode]).toBeTruthy();
      expect(voiceModels.languageModeLabels[mode]).toBeTruthy();
    }
  });

  it("has all four status labels on both broadcaster and admin sides", () => {
    for (const statuses of [voice.statusLabels, voiceModels.statusLabels]) {
      for (const key of ["processing", "pending_review", "approved", "rejected"] as const) {
        expect(statuses[key]).toBeTruthy();
      }
    }
  });

  it("surfaces plan-gating and provider-disabled errors distinctly", () => {
    const values = Object.values(voice.errors);
    expect(new Set(values).size).toBe(values.length);
    expect(voice.errors.planLocked.toLowerCase()).toContain("plan");
  });

  it("contains no emoji anywhere in the voice cloning copy", () => {
    const scan = (value: unknown): void => {
      if (typeof value === "string") {
        expect(EMOJI.test(value), `emoji found in: ${value}`).toBe(false);
      } else if (Array.isArray(value)) {
        value.forEach(scan);
      } else if (value && typeof value === "object") {
        Object.values(value).forEach(scan);
      }
    };
    scan(voice);
    scan(voiceModels);
  });
});
