import { describe, expect, it } from "vitest";
import { getDictionary } from "@/src/i18n/get-dictionary";

describe("getDictionary", () => {
  it("returns a dictionary with brand metadata", async () => {
    const dict = await getDictionary("en");
    expect(dict.brand.name).toBeTruthy();
    expect(dict.brand.logoAsset).toMatch(/^\//);
  });

  it("falls back to the default locale for unknown input", async () => {
    const fallback = await getDictionary("xx");
    const en = await getDictionary("en");
    expect(fallback).toEqual(en);
  });
});
