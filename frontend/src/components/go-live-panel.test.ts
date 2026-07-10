import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./go-live-panel.tsx", import.meta.url), "utf8");

describe("go live panel", () => {
  it("computes readiness from live backend signals instead of static dictionary data", () => {
    expect(source).toContain('userFetch("/api/sdk/devices")');
    expect(source).toContain('userFetch("/api/integrations")');
    expect(source).toContain("fetchFoundingStatus()");
    expect(source).not.toContain("step.done");
    expect(source).not.toContain("mode.steps.every");
  });

  it("requires an approved founding application only when the gate is enabled", () => {
    expect(source).toContain("!founding.enabled || founding.application?.status === \"approved\"");
  });

  it("requires a connected SDK device, and a connected platform for cloud mode", () => {
    expect(source).toContain("devices.some((d) => d.connected)");
    expect(source).toContain("(!isCloud || platformConnected)");
  });

  it("disables the start button until ready", () => {
    expect(source).toContain("disabled={!ready}");
  });
});
