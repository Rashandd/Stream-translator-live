/**
 * Regression coverage for the cookie-session refresh flow.
 *
 * The backend's `POST /api/auth/refresh` requires a matching `x-csrf-token`
 * header (double-submit — `require_cookie_request` in
 * `backend/src/auth/session.rs`). If this file's silent-refresh call ever
 * drops that header again, every session dies at the 15-minute access-token
 * TTL: `cookieFetch` gets a 401, `refreshSession` gets 403'd by the backend,
 * and the caller falls back to "session expired" instead of recovering.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { adminFetch, API_URL } from "@/src/lib/api";

function stubStaticCookie(cookie: string) {
  vi.stubGlobal("document", { cookie });
}

function headerValue(init: RequestInit | undefined, name: string): string | null {
  return new Headers(init?.headers).get(name);
}

describe("adminFetch (cookie session)", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("does not attach x-csrf-token to safe GET requests", async () => {
    stubStaticCookie("sd_csrf=abc123");
    const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 200 }));
    vi.stubGlobal("fetch", fetchMock);

    await adminFetch("/api/auth/session");

    expect(headerValue(fetchMock.mock.calls[0][1], "x-csrf-token")).toBeNull();
  });

  it("attaches x-csrf-token from the sd_csrf cookie to mutating requests", async () => {
    stubStaticCookie("sd_csrf=abc123");
    const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 200 }));
    vi.stubGlobal("fetch", fetchMock);

    await adminFetch("/admin/config", { method: "PUT", body: "{}" });

    expect(headerValue(fetchMock.mock.calls[0][1], "x-csrf-token")).toBe("abc123");
  });

  it("sends x-csrf-token on the silent POST /api/auth/refresh triggered by a 401", async () => {
    stubStaticCookie("sd_csrf=abc123");
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(new Response(null, { status: 401 })) // original request
      .mockResolvedValueOnce(new Response(null, { status: 200 })) // POST /api/auth/refresh
      .mockResolvedValueOnce(new Response(null, { status: 200 })); // retried request
    vi.stubGlobal("fetch", fetchMock);

    await adminFetch("/admin/config", { method: "PUT", body: "{}" });

    expect(fetchMock).toHaveBeenCalledTimes(3);
    const [refreshUrl, refreshInit] = fetchMock.mock.calls[1];
    expect(refreshUrl).toBe(`${API_URL}/api/auth/refresh`);
    expect(headerValue(refreshInit, "x-csrf-token")).toBe("abc123");
  });

  it("re-reads the sd_csrf cookie before retrying, so a rotated cookie from the refresh response is used instead of the stale one", async () => {
    let cookie = "sd_csrf=old-token";
    vi.stubGlobal("document", {
      get cookie() {
        return cookie;
      },
    });

    // `headers` is a mutable `Headers` instance shared by reference between
    // the original and retried call, so snapshot the value each call saw
    // *at call time* rather than reading it back from `mock.calls` after
    // the fact (that would just show the final, post-mutation state twice).
    const seenCsrfTokens: Array<string | null> = [];
    const fetchMock = vi
      .fn()
      .mockImplementationOnce(async (_url: string, init?: RequestInit) => {
        seenCsrfTokens.push(headerValue(init, "x-csrf-token"));
        return new Response(null, { status: 401 }); // original request, still holds old-token
      })
      .mockImplementationOnce(async () => {
        // Simulates the browser applying refresh's rotated Set-Cookie trio.
        cookie = "sd_csrf=new-token";
        return new Response(null, { status: 200 }); // POST /api/auth/refresh
      })
      .mockImplementationOnce(async (_url: string, init?: RequestInit) => {
        seenCsrfTokens.push(headerValue(init, "x-csrf-token"));
        return new Response(null, { status: 200 }); // retried request
      });
    vi.stubGlobal("fetch", fetchMock);

    await adminFetch("/admin/config", { method: "PUT", body: "{}" });

    expect(seenCsrfTokens).toEqual(["old-token", "new-token"]);
  });

  it("gives up and surfaces the original 401 when the refresh call itself fails", async () => {
    stubStaticCookie("sd_csrf=abc123");
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(new Response(null, { status: 401 })) // original request
      .mockResolvedValueOnce(new Response(null, { status: 403 })); // refresh rejected
    vi.stubGlobal("fetch", fetchMock);

    const response = await adminFetch("/admin/config", { method: "PUT", body: "{}" });

    expect(response.status).toBe(401);
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });
});
