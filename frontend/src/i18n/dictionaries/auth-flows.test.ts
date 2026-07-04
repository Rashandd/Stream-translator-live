import { describe, expect, it } from "vitest";
import en from "@/src/i18n/dictionaries/en.json";

/**
 * Yayıncı auth akışı (1.4) sözlük sözleşmesi — login/signup/verify/reset
 * client formlarının beklediği anahtarlar. Bir anahtar silinirse form
 * derlenmeden önce burada yakalanır.
 */
describe("auth flow dictionary contract", () => {
  it("login form has submit/submitting labels and forgot link", () => {
    expect(en.auth.login.submitLabel).toBeTruthy();
    expect(en.auth.login.submittingLabel).toBeTruthy();
    expect(en.auth.login.forgotLabel).toBeTruthy();
  });

  it("signup form has its labels and terms note", () => {
    expect(en.auth.signup.submitLabel).toBeTruthy();
    expect(en.auth.signup.submittingLabel).toBeTruthy();
    expect(en.auth.signup.termsNote).toBeTruthy();
  });

  it("verify form is a real 2fa step (code entry, no dev stub)", () => {
    expect(en.auth.verify.codeLabel).toBeTruthy();
    expect(en.auth.verify.submittingLabel).toBeTruthy();
    expect("devNotice" in en.auth.verify).toBe(false);
  });

  it("reset flow is code-based with a second step", () => {
    expect(en.auth.reset.subtitle.toLowerCase()).toContain("code");
    expect(en.auth.reset.codeLabel).toBeTruthy();
    expect(en.auth.reset.newPasswordLabel).toBeTruthy();
    expect(en.auth.reset.confirmLabel).toBeTruthy();
    expect(en.auth.reset.doneMessage).toBeTruthy();
  });

  it("shared error messages exist for every failure path", () => {
    const errors = en.auth.errors;
    for (const key of [
      "invalid",
      "network",
      "unavailable",
      "emailTaken",
      "invalidFields",
      "weakPassword",
      "codeInvalid",
      "codeTooMany",
      "sessionMissing",
    ] as const) {
      expect(errors[key], `auth.errors.${key}`).toBeTruthy();
    }
  });

  it("auth copy contains no emoji (DESIGN.md)", () => {
    const flat = JSON.stringify(en.auth);
    expect(/[\u{1F300}-\u{1FAFF}\u{2600}-\u{27BF}]/u.test(flat)).toBe(false);
  });
});
