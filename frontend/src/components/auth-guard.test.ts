import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./auth-guard.tsx", import.meta.url), "utf8");

describe("AuthGuard", () => {
  it("trusts a locally stored OAuth bearer token for the broadcaster dashboard without a network round trip", () => {
    expect(source).toContain('tokenKind === "user" && getUserToken()');
  });

  it("otherwise validates the live cookie session via GET /api/auth/session", () => {
    expect(source).toContain('await fetcher("/api/auth/session")');
    expect(source).toContain('const fetcher = tokenKind === "admin" ? adminFetch : userFetch');
  });

  it("requires role=admin for the admin console, not just any authenticated session", () => {
    expect(source).toContain('tokenKind === "admin" ? data.role === "admin" : true');
  });

  it("redirects with a reason when the session is rejected or the API is unreachable", () => {
    expect(source).toContain('redirectToLogin("session")');
    expect(source).toContain('redirectToLogin("network")');
    expect(source).toContain('url.searchParams.set("auth_error", reason)');
    expect(source).toContain("if (!authorized) {\n    return null;\n  }");
  });
});
