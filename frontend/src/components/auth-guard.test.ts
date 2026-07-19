import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./auth-guard.tsx", import.meta.url), "utf8");

describe("AuthGuard", () => {
  it("has no localStorage bearer short-circuit — a stale token can't shadow the cookie session (Faz 2)", () => {
    expect(source).not.toContain("getUserToken");
  });

  it("always validates the live cookie session via GET /api/auth/session", () => {
    expect(source).toContain('await fetcher("/api/auth/session")');
    expect(source).toContain('const fetcher = tokenKind === "admin" ? adminFetch : userFetch');
  });

  it("requires role=admin for the admin console, not just any authenticated session", () => {
    expect(source).toContain('tokenKind === "admin" ? data.role === "admin" : true');
  });

  it("stores a one-time notice and replaces the URL without auth query parameters", () => {
    expect(source).toContain('"session_expired" : "sign_in_required"');
    expect(source).toContain('handleUnauthorized("session_unavailable")');
    expect(source).toContain("storeAuthNotice(reason)");
    expect(source).toContain("window.location.replace(loginHref as string)");
    expect(source).not.toContain("auth_error");
    expect(source).toContain("if (authorized) {\n    return <>{children}</>;\n  }");
  });

  it("in modal mode surfaces a login popup instead of navigating away", () => {
    // Modal dalı yönlendirme yapmaz: notice'i state'e yazar, arkada LoginModal açar.
    expect(source).toContain("if (isModal) {");
    expect(source).toContain("setNoticeCode(reason)");
    expect(source).toContain("<LoginModal noticeCode={noticeCode} open {...props.modalConfig} />");
    // storeAuthNotice/replace yalnız redirect dalında; modal dalı bunları çağırmamalı.
    expect(source).toContain('props.mode === "modal" ? null : props.loginHref');
  });
});
