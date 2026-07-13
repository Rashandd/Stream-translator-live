import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import en from "@/src/i18n/dictionaries/en.json";

const loginFormSource = readFileSync(
  new URL("../../components/login-form.tsx", import.meta.url),
  "utf8",
);
const signupFormSource = readFileSync(
  new URL("../../components/signup-form.tsx", import.meta.url),
  "utf8",
);
const authClientSource = readFileSync(new URL("../../lib/auth-client.ts", import.meta.url), "utf8");
const verifyFormSource = readFileSync(
  new URL("../../components/verify-code-form.tsx", import.meta.url),
  "utf8",
);
const adminVerifySource = readFileSync(
  new URL("../../../app/[locale]/admin/login/verify/page.tsx", import.meta.url),
  "utf8",
);

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
    expect("emailPlaceholder" in en.auth.login).toBe(false);
    expect("passwordPlaceholder" in en.auth.login).toBe(false);
  });

  it("login legal note replaces the old email-code helper copy", () => {
    expect(JSON.stringify(en.auth.login)).not.toContain("We'll email you a 6-digit code");
    expect("legalNote" in en.auth.login).toBe(false);
  });

  it("signup form has required legal consent and optional commercial consent copy", () => {
    expect(en.auth.signup.submitLabel).toBeTruthy();
    expect(en.auth.signup.submittingLabel).toBeTruthy();
    expect("emailPlaceholder" in en.auth.signup).toBe(false);
    expect("passwordPlaceholder" in en.auth.signup).toBe(false);
    expect("subtitle" in en.auth.signup).toBe(false);
    expect(JSON.stringify(en.auth.signup)).not.toContain("No credit card needed");
    expect(en.auth.signup.requiredConsent.label.toLowerCase()).toContain("by creating");
    expect(en.auth.signup.requiredConsent.links.map((link) => link.label)).toEqual([
      "Terms",
      "Privacy",
      "KVKK",
      "Cookies",
      "DPA",
    ]);
    expect(en.auth.signup.optionalConsent.label.toLowerCase()).toContain("optional");
    expect(en.auth.signup.optionalConsent.links.map((link) => link.label)).toEqual([
      "Electronic communications",
    ]);
  });

  it("signup form sends required legal consent separately from optional marketing consent", () => {
    expect(signupFormSource).toContain("legal_terms_accepted: true");
    expect(signupFormSource).toContain("marketing_consent");
    expect(signupFormSource).not.toContain("legalTermsAccepted");
    expect(signupFormSource).toContain("optionalConsent");
  });

  it("signup completes authentication in one request", () => {
    expect(signupFormSource).toContain("/api/auth/signup");
    expect(signupFormSource).not.toContain("await login(");
    expect(signupFormSource).toContain("outcome.two_factor_required");
    expect(signupFormSource).toContain("outcome.authenticated");
  });

  it("2fa keeps identity in an HttpOnly challenge instead of sessionStorage", () => {
    expect(authClientSource).not.toContain("sessionStorage");
    expect(authClientSource).not.toContain("PENDING_2FA");
    expect(verifyFormSource).toContain("JSON.stringify({ code })");
    expect(verifyFormSource).not.toContain("email");
    expect(verifyFormSource).toContain("/api/auth/2fa/resend");
  });

  it("admin verify page uses the real shared verify form", () => {
    expect(adminVerifySource).toContain("<VerifyCodeForm");
    expect(adminVerifySource).not.toContain("devNotice");
    expect(adminVerifySource).not.toContain("<form action=");
  });

  it("public auth forms do not render email or password placeholders", () => {
    expect(loginFormSource).not.toContain("placeholder=");
    expect(signupFormSource).not.toContain("placeholder=");
    expect(JSON.stringify(en.auth.login)).not.toContain("you@example.com");
    expect(JSON.stringify(en.auth.signup)).not.toContain("you@example.com");
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
      "signInRequired",
      "logoutFailed",
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

  it("explains why an authenticated dashboard visit returned to sign-in", () => {
    expect(en.auth.errors.sessionExpired).toBeTruthy();
    expect(en.auth.errors.sessionUnavailable).toBeTruthy();
  });

  it("auth copy contains no emoji (DESIGN.md)", () => {
    const flat = JSON.stringify(en.auth);
    expect(/[\u{1F300}-\u{1FAFF}\u{2600}-\u{27BF}]/u.test(flat)).toBe(false);
  });
});
