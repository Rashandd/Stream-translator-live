import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import en from "@/src/i18n/dictionaries/en.json";

const pairingSource = readFileSync(
  fileURLToPath(new URL("../../components/sdk-pairing-panel.tsx", import.meta.url)),
  "utf8",
);
const streamKeySource = readFileSync(
  fileURLToPath(new URL("../../components/sdk-stream-key-panel.tsx", import.meta.url)),
  "utf8",
);
const copyRowSource = readFileSync(
  fileURLToPath(new URL("../../components/copy-row.tsx", import.meta.url)),
  "utf8",
);
const sdkPageSource = readFileSync(
  fileURLToPath(new URL("../../../app/[locale]/dashboard/sdk/page.tsx", import.meta.url)),
  "utf8",
);

const EMOJI = /[\u{1F300}-\u{1FAFF}\u{2600}-\u{27BF}]/u;

describe("/dashboard/sdk pairing + stream key wiring (Faz 2.8)", () => {
  const { sdk } = en.dashboard;

  it("pairing card talks to the real backend endpoint", () => {
    expect(pairingSource).toContain('userFetch("/api/pairing/code"');
    expect(pairingSource).toContain('method: "POST"');
  });

  it("stream key card reads and writes the real relay targets endpoint", () => {
    expect(streamKeySource).toContain('userFetch("/api/relay/targets")');
    expect(streamKeySource).toContain('userFetch("/api/relay/targets"');
    expect(streamKeySource).toContain('method: "PUT"');
  });

  it("copy button actually writes to the clipboard now (was decorative)", () => {
    expect(copyRowSource).toContain("navigator.clipboard.writeText");
  });

  it("download buttons link to a real installer URL now (was decorative for every platform)", () => {
    expect(sdkPageSource).toContain("sdkDownloadUrl(platform.name)");
    expect(sdkPageSource).toContain("href={href}");
  });

  it("all three platforms are enabled (macOS/Linux are no longer coming_soon)", () => {
    for (const platform of sdk.platforms) {
      expect(platform.status).toBe("enabled");
    }
  });

  it("has non-placeholder copy for both cards", () => {
    expect(sdk.pairingPlaceholder).toBeTruthy();
    expect(sdk.generateLabel).toBeTruthy();
    expect(sdk.generatingLabel).toBeTruthy();
    expect(sdk.expiresInTemplate).toContain("{minutes}");
    expect(sdk.streamKeyLoading).toBeTruthy();
    expect(sdk.copiedLabel).toBeTruthy();
  });

  it("has distinct, non-empty error messages within each card", () => {
    for (const errors of [sdk.pairingErrors, sdk.streamKeyErrors]) {
      const values = Object.values(errors);
      expect(new Set(values).size).toBe(values.length);
      for (const value of values) expect(value).toBeTruthy();
    }
  });

  it("contains no emoji anywhere in the sdk page copy", () => {
    const scan = (value: unknown): void => {
      if (typeof value === "string") {
        expect(EMOJI.test(value), `emoji found in: ${value}`).toBe(false);
      } else if (Array.isArray(value)) {
        value.forEach(scan);
      } else if (value && typeof value === "object") {
        Object.values(value).forEach(scan);
      }
    };
    scan(sdk);
  });
});
