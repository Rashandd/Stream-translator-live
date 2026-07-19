import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const modalSource = readFileSync(new URL("./login-modal.tsx", import.meta.url), "utf8");
const loginFormSource = readFileSync(new URL("./login-form.tsx", import.meta.url), "utf8");
const verifyFormSource = readFileSync(new URL("./verify-code-form.tsx", import.meta.url), "utf8");

describe("LoginModal", () => {
  it("renders nothing until opened", () => {
    expect(modalSource).toContain("if (!open) {");
    expect(modalSource).toContain("return null;");
  });

  it("is a real modal dialog with the design-system overlay scrim", () => {
    expect(modalSource).toContain('role="dialog"');
    expect(modalSource).toContain('aria-modal="true"');
    expect(modalSource).toContain("bg-emphasis/70");
  });

  it("reloads the page on a successful broadcaster login (cookie already set)", () => {
    expect(modalSource).toContain("onBroadcaster={() => window.location.reload()}");
  });

  it("still hard-navigates for the admin edge case", () => {
    expect(modalSource).toContain("onAdmin={() => window.location.assign(adminHref)}");
  });

  it("keeps the 2FA step inside the modal instead of routing to /login/verify", () => {
    expect(modalSource).toContain('onTwoFactor={() => setStep("verify")}');
    expect(modalSource).toContain("<VerifyCodeForm");
    expect(modalSource).toContain('setStep("login")');
  });

  it("surfaces the reason the login was required as an inline notice", () => {
    expect(modalSource).toContain("NOTICE_LABEL_KEY");
    expect(modalSource).toContain("labels.errors[NOTICE_LABEL_KEY[noticeCode]]");
  });
});

describe("auth forms accept modal callbacks without breaking the page flow", () => {
  it("LoginForm prefers callbacks but keeps the full-navigation fallback", () => {
    expect(loginFormSource).toContain("if (onBroadcaster) onBroadcaster();");
    expect(loginFormSource).toContain("else window.location.assign(dashboardHref);");
    expect(loginFormSource).toContain("if (onTwoFactor) onTwoFactor();");
    expect(loginFormSource).toContain("else if (verifyHref) window.location.assign(verifyHref);");
  });

  it("VerifyCodeForm prefers callbacks but keeps the full-navigation fallback", () => {
    expect(verifyFormSource).toContain("if (onBroadcaster) onBroadcaster();");
    expect(verifyFormSource).toContain("else window.location.assign(dashboardHref);");
    expect(verifyFormSource).toContain("if (onAdmin) onAdmin();");
    expect(verifyFormSource).toContain("else window.location.assign(adminHref);");
  });
});
