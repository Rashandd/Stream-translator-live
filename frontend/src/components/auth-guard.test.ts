import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./auth-guard.tsx", import.meta.url), "utf8");

describe("AuthGuard", () => {
  it("checks the requested token kind before rendering protected content", () => {
    expect(source).toContain('tokenKind === "admin" ? getAdminToken() : getUserToken()');
  });

  it("redirects to the login page instead of rendering children when no token exists", () => {
    expect(source).toContain("window.location.assign(loginHref)");
    expect(source).toContain("if (!authorized) {\n    return null;\n  }");
  });
});
