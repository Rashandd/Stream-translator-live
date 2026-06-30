import { describe, expect, it } from "vitest";
import { defaultLocale, isLocale, localizePath, locales } from "@/src/i18n/routing";

describe("isLocale", () => {
  it("accepts supported locales", () => {
    expect(isLocale(defaultLocale)).toBe(true);
    for (const locale of locales) {
      expect(isLocale(locale)).toBe(true);
    }
  });

  it("rejects unknown locales", () => {
    expect(isLocale("xx")).toBe(false);
    expect(isLocale("")).toBe(false);
  });
});

describe("localizePath", () => {
  it("maps root to the locale root", () => {
    expect(localizePath("en", "/")).toBe("/en");
  });

  it("prefixes nested paths with the locale", () => {
    expect(localizePath("en", "/dashboard")).toBe("/en/dashboard");
  });

  it("leaves in-page anchors untouched", () => {
    expect(localizePath("en", "#features")).toBe("#features");
  });
});
