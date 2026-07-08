import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./admin-legal-panel.tsx", import.meta.url), "utf8");
const legalPageSource = readFileSync(
  new URL("../../app/[locale]/legal/[slug]/page.tsx", import.meta.url),
  "utf8",
);

describe("admin legal HTML editor", () => {
  it("loads runtime config and saves legal html through admin config", () => {
    expect(source).toContain('adminFetch("/config")');
    expect(source).toContain('adminFetch("/admin/config"');
    expect(source).toContain("legal.${slug}.html");
  });

  it("uses textarea editing for direct HTML", () => {
    expect(source).toContain("<textarea");
    expect(source).toContain("dangerouslySetInnerHTML");
  });

  it("seeds the editable draft from the en.json fallback until a custom save exists", () => {
    expect(source).toContain("saved?.trim() ? saved : fallbackHtml(page)");
    expect(source).toContain("value.trim() ? value : fallbackHtml(selectedPage)");
  });

  it("public legal page reads configured HTML before fallback content", () => {
    expect(legalPageSource).toContain("loadRuntimeLegalHtml");
    expect(legalPageSource).toContain("dangerouslySetInnerHTML");
    expect(legalPageSource).toContain("legal.${slug}.html");
  });
});
