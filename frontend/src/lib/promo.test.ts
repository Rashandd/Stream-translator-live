import { describe, expect, it } from "vitest";
import { parseFoundingPromo } from "@/src/lib/promo";

describe("parseFoundingPromo", () => {
  it("reads enabled=true and the message from the config snapshot", () => {
    const promo = parseFoundingPromo({
      "promo.founding.enabled": "true",
      "promo.founding.message": "Founding testers stream free.",
    });
    expect(promo.enabled).toBe(true);
    expect(promo.message).toBe("Founding testers stream free.");
  });

  it("treats anything other than the literal string 'true' as disabled", () => {
    expect(parseFoundingPromo({ "promo.founding.enabled": "false" }).enabled).toBe(false);
    expect(parseFoundingPromo({}).enabled).toBe(false);
  });

  it("defaults message to an empty string when the key is missing", () => {
    expect(parseFoundingPromo({ "promo.founding.enabled": "true" }).message).toBe("");
  });
});
