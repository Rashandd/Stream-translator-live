import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./sign-out-link.tsx", import.meta.url), "utf8");
const dashboardLayoutSource = readFileSync(
  new URL("../../app/[locale]/dashboard/layout.tsx", import.meta.url),
  "utf8",
);
const adminLayoutSource = readFileSync(
  new URL("../../app/[locale]/admin/(console)/layout.tsx", import.meta.url),
  "utf8",
);

describe("SignOutLink", () => {
  it("clears the matching token on click before navigating away", () => {
    expect(source).toContain('tokenKind === "admin" ? clearAdminToken() : clearUserToken()');
  });

  it("the dashboard layout signs out with the broadcaster token, not a bare link", () => {
    expect(dashboardLayoutSource).toContain("<SignOutLink");
    expect(dashboardLayoutSource).toContain('tokenKind="user"');
  });

  it("the admin layout signs out with the admin token, not a bare link", () => {
    expect(adminLayoutSource).toContain("<SignOutLink");
    expect(adminLayoutSource).toContain('tokenKind="admin"');
  });
});

describe("dashboard and admin layouts gate protected content behind AuthGuard", () => {
  it("dashboard layout wraps its nav/content in an AuthGuard for the broadcaster token", () => {
    expect(dashboardLayoutSource).toContain('<AuthGuard loginHref={localizePath(locale, "/login")} tokenKind="user">');
  });

  it("admin layout wraps its nav/content in an AuthGuard for the admin token", () => {
    expect(adminLayoutSource).toContain(
      '<AuthGuard loginHref={localizePath(locale, "/admin/login")} tokenKind="admin">',
    );
  });
});
