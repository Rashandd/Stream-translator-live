import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./founding-application-panel.tsx", import.meta.url), "utf8");

describe("founding application panel", () => {
  it("loads and submits through the user-scoped founding application endpoint", () => {
    expect(source).toContain('userFetch("/api/founding/application")');
    expect(source).toContain('userFetch("/api/founding/application", {');
    expect(source).toContain('method: "POST"');
  });

  it("sends platform, channel_url, and message in the submit body", () => {
    expect(source).toContain("JSON.stringify({ platform, channel_url: channelUrl, message })");
  });

  it("treats a 404 response as no application yet, and 422 as invalid input", () => {
    expect(source).toContain("res.status === 404");
    expect(source).toContain("res.status === 422");
  });

  it("renders a status card once an application exists instead of the form", () => {
    expect(source).toContain("labels.statusLabels[application.status]");
    expect(source).toContain("labels.statusHints[application.status]");
  });
});
