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

const EMOJI = /[\u{1F300}-\u{1FAFF}\u{2600}-\u{27BF}]/u;

describe("twitch extension dashboard card config (2.4 B4)", () => {
  const extension = en.dashboard.platforms.extension;

  it("exists with title, summary and a 3-step linking flow", () => {
    expect(extension.title).toBeTruthy();
    expect(extension.statusLabel).toBeTruthy();
    expect(extension.summary).toBeTruthy();
    expect(extension.stepsLabel).toBeTruthy();
    expect(extension.steps.length).toBe(3);
  });

  it("carries every label the interactive link component consumes", () => {
    expect(extension.codeButton).toBeTruthy();
    expect(extension.generatingLabel).toBeTruthy();
    expect(extension.codeReadyLabel).toBeTruthy();
    expect(extension.validForLabel).toBeTruthy();
    expect(extension.copyLabel).toBeTruthy();
    expect(extension.copiedLabel).toBeTruthy();
    expect(extension.linkedLabel).toBeTruthy();
    expect(extension.notLinkedLabel).toBeTruthy();
    expect(extension.unlinkLabel).toBeTruthy();
    expect(extension.confirmUnlink).toBeTruthy();
    expect(extension.signInNote).toBeTruthy();
    expect(extension.disabledNote).toBeTruthy();
    expect(extension.errors.network).toBeTruthy();
    expect(extension.errors.failed).toBeTruthy();
  });

  it("uses a registered lucide icon and no emoji anywhere", () => {
    expect(registeredIcons.has(extension.icon)).toBe(true);
    expect(JSON.stringify(extension)).not.toMatch(EMOJI);
  });

  it("keeps viewer-facing commerce language out of the card", () => {
    // Policy: extension akışında satın alma/kayıt CTA dili kullanılmaz.
    const text = JSON.stringify(extension).toLowerCase();
    for (const banned of ["buy", "purchase", "subscribe now", "upgrade"]) {
      expect(text).not.toContain(banned);
    }
  });
});
