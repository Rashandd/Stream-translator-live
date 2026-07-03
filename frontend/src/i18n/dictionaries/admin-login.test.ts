import { describe, expect, it } from "vitest";
import en from "@/src/i18n/dictionaries/en.json";

/** Emoji yasağı (DESIGN.md) — kaba ama yeterli bir tarama. */
const EMOJI = /[\u{1F300}-\u{1FAFF}\u{2600}-\u{27BF}]/u;

describe("admin login config (V2 — env-bootstrap admin girişi)", () => {
  const login = en.admin.login;

  it("has form labels and states for the live login flow", () => {
    expect(login.emailLabel).toBeTruthy();
    expect(login.passwordLabel).toBeTruthy();
    expect(login.submitLabel).toBeTruthy();
    expect(login.submittingLabel).toBeTruthy();
  });

  it("has distinct error messages for invalid, unavailable and network cases", () => {
    expect(login.errorInvalid).toBeTruthy();
    expect(login.errorUnavailable).toBeTruthy();
    expect(login.errorNetwork).toBeTruthy();
    const unique = new Set([login.errorInvalid, login.errorUnavailable, login.errorNetwork]);
    expect(unique.size).toBe(3);
    // Ops kullanıcıyı doğru env değişkenine yönlendirir.
    expect(login.errorUnavailable).toContain("ADMIN_EMAIL");
    expect(login.errorNetwork).toContain("NEXT_PUBLIC_API_URL");
  });

  it("does not promise an active 2FA code flow (SMTP is not wired yet)", () => {
    expect(login.twofaNote.toLowerCase()).toContain("smtp");
  });

  it("uses no emoji anywhere in the login copy", () => {
    for (const value of Object.values(login)) {
      expect(EMOJI.test(String(value)), `emoji found in: ${value}`).toBe(false);
    }
  });
});
