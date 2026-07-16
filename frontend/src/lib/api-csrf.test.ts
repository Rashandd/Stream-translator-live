/** Cross-origin CSRF bootstrap and rotation regressions. */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

function responseWithToken(token: string, status = 200): Response {
  return new Response(JSON.stringify({ csrf_token: token }), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function headerValue(init: RequestInit | undefined, name: string): string | null {
  return new Headers(init?.headers).get(name);
}

async function freshApi() {
  vi.resetModules();
  return import("@/src/lib/api");
}

describe("cross-origin cookie-session CSRF", () => {
  beforeEach(() => {
    // Simulates dev.strovak.com: document.cookie cannot see host-only cookies
    // owned by dev.api.strovak.com.
    vi.stubGlobal("document", { cookie: "" });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.unstubAllEnvs();
    vi.restoreAllMocks();
  });

  it("ignores a same-named frontend-host cookie when the API uses another host", async () => {
    vi.stubEnv("NEXT_PUBLIC_API_URL", "https://dev.api.strovak.com");
    vi.stubGlobal("window", {
      location: { hostname: "dev.strovak.com", origin: "https://dev.strovak.com" },
    });
    vi.stubGlobal("document", { cookie: "sd_csrf=stale-frontend-host-token" });
    const seenTokens: Array<string | null> = [];
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(responseWithToken("api-host-token"))
      .mockImplementationOnce(async (_url: string, init?: RequestInit) => {
        seenTokens.push(headerValue(init, "x-csrf-token"));
        return new Response(null, { status: 200 });
      });
    vi.stubGlobal("fetch", fetchMock);
    const { adminFetch } = await freshApi();

    await adminFetch("/admin/config", { method: "PUT", body: "{}" });

    expect(fetchMock.mock.calls.map(([url]) => url)).toEqual([
      "https://dev.api.strovak.com/api/auth/csrf",
      "https://dev.api.strovak.com/admin/config",
    ]);
    expect(seenTokens).toEqual(["api-host-token"]);
  });

  it("keeps using a visible host-only cookie on localhost even when ports differ", async () => {
    vi.stubEnv("NEXT_PUBLIC_API_URL", "http://localhost:8080");
    vi.stubGlobal("window", {
      location: { hostname: "localhost", origin: "http://localhost:3000" },
    });
    vi.stubGlobal("document", { cookie: "sd_csrf=localhost-token" });
    const seenTokens: Array<string | null> = [];
    const fetchMock = vi.fn(async (_url: string, init?: RequestInit) => {
      seenTokens.push(headerValue(init, "x-csrf-token"));
      return new Response(null, { status: 200 });
    });
    vi.stubGlobal("fetch", fetchMock);
    const { adminFetch } = await freshApi();

    await adminFetch("/admin/config", { method: "PUT" });

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(seenTokens).toEqual(["localhost-token"]);
  });

  it("bootstraps and caches the API-host token after an OAuth redirect or in a new tab", async () => {
    const seenTokens: Array<string | null> = [];
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(responseWithToken("api-host-token"))
      .mockImplementation(async (_url: string, init?: RequestInit) => {
        seenTokens.push(headerValue(init, "x-csrf-token"));
        return new Response(null, { status: 200 });
      });
    vi.stubGlobal("fetch", fetchMock);
    const { adminFetch, API_URL } = await freshApi();

    await adminFetch("/admin/config", { method: "PUT", body: "{}" });
    await adminFetch("/admin/config", { method: "PUT", body: "{}" });

    expect(fetchMock.mock.calls[0][0]).toBe(`${API_URL}/api/auth/csrf`);
    expect(fetchMock.mock.calls[0][1]).toMatchObject({
      method: "GET",
      credentials: "include",
      cache: "no-store",
    });
    expect(seenTokens).toEqual(["api-host-token", "api-host-token"]);
    expect(fetchMock).toHaveBeenCalledTimes(3);
  });

  it("caches the token already returned by OAuth session verification", async () => {
    const seenTokens: Array<string | null> = [];
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({ authenticated: true, role: "broadcaster", csrf_token: "oauth-token" }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      )
      .mockImplementationOnce(async (_url: string, init?: RequestInit) => {
        seenTokens.push(headerValue(init, "x-csrf-token"));
        return new Response(null, { status: 200 });
      });
    vi.stubGlobal("fetch", fetchMock);
    const { userFetch, API_URL } = await freshApi();

    const session = await userFetch("/api/auth/session");
    expect((await session.json()).authenticated).toBe(true);
    await userFetch("/api/integrations/twitch/test", { method: "POST" });

    expect(fetchMock.mock.calls.map(([url]) => url)).toEqual([
      `${API_URL}/api/auth/session`,
      `${API_URL}/api/integrations/twitch/test`,
    ]);
    expect(seenTokens).toEqual(["oauth-token"]);
  });

  it("coalesces concurrent token bootstrap calls", async () => {
    let resolveBootstrap!: (response: Response) => void;
    const bootstrap = new Promise<Response>((resolve) => {
      resolveBootstrap = resolve;
    });
    const fetchMock = vi.fn((url: string) => {
      if (url.endsWith("/api/auth/csrf")) {
        return bootstrap;
      }
      return Promise.resolve(new Response(null, { status: 200 }));
    });
    vi.stubGlobal("fetch", fetchMock);
    const { adminFetch, API_URL } = await freshApi();

    const first = adminFetch("/admin/config", { method: "PUT", body: "{}" });
    const second = adminFetch("/admin/config", { method: "PUT", body: "{}" });
    await vi.waitFor(() => {
      expect(fetchMock).toHaveBeenCalledTimes(1);
    });
    resolveBootstrap(responseWithToken("shared-token"));
    await Promise.all([first, second]);

    expect(
      fetchMock.mock.calls.filter(([url]) => url === `${API_URL}/api/auth/csrf`),
    ).toHaveLength(1);
    expect(fetchMock).toHaveBeenCalledTimes(3);
  });

  it("uses the token rotated in a refresh response for the retried mutation", async () => {
    const seen: Array<[string, string | null]> = [];
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(responseWithToken("before-refresh"))
      .mockImplementationOnce(async (url: string, init?: RequestInit) => {
        seen.push([url, headerValue(init, "x-csrf-token")]);
        return new Response(null, { status: 401 });
      })
      .mockImplementationOnce(async (url: string, init?: RequestInit) => {
        seen.push([url, headerValue(init, "x-csrf-token")]);
        return responseWithToken("after-refresh");
      })
      .mockImplementationOnce(async (url: string, init?: RequestInit) => {
        seen.push([url, headerValue(init, "x-csrf-token")]);
        return new Response(null, { status: 200 });
      });
    vi.stubGlobal("fetch", fetchMock);
    const { adminFetch, API_URL } = await freshApi();

    await adminFetch("/admin/config", { method: "PUT", body: "{}" });

    expect(seen).toEqual([
      [`${API_URL}/admin/config`, "before-refresh"],
      [`${API_URL}/api/auth/refresh`, "before-refresh"],
      [`${API_URL}/admin/config`, "after-refresh"],
    ]);
  });

  it("re-bootstraps a token rotated by another tab before retrying a rejected mutation", async () => {
    const seenTokens: Array<string | null> = [];
    const fetchMock = vi
      .fn()
      .mockImplementationOnce(async (_url: string, init?: RequestInit) => {
        seenTokens.push(headerValue(init, "x-csrf-token"));
        return new Response(null, { status: 403 });
      })
      .mockResolvedValueOnce(responseWithToken("other-tab-token"))
      .mockImplementationOnce(async (_url: string, init?: RequestInit) => {
        seenTokens.push(headerValue(init, "x-csrf-token"));
        return new Response(null, { status: 200 });
      });
    vi.stubGlobal("fetch", fetchMock);
    const { adminFetch, cacheCsrfToken } = await freshApi();
    cacheCsrfToken("stale-token");

    const response = await adminFetch("/admin/config", { method: "PUT", body: "{}" });

    expect(response.status).toBe(200);
    expect(seenTokens).toEqual(["stale-token", "other-tab-token"]);
  });

  it("does not replay a genuine 403 when bootstrap returns the same token", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(new Response(null, { status: 403 }))
      .mockResolvedValueOnce(responseWithToken("current-token"));
    vi.stubGlobal("fetch", fetchMock);
    const { adminFetch, cacheCsrfToken } = await freshApi();
    cacheCsrfToken("current-token");

    const response = await adminFetch("/admin/config", { method: "DELETE" });

    expect(response.status).toBe(403);
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it("refreshes a stale cross-tab token before retrying refresh itself", async () => {
    const seen: Array<[string, string | null]> = [];
    const fetchMock = vi
      .fn()
      .mockImplementationOnce(async (url: string) => {
        seen.push([url, null]);
        return new Response(null, { status: 401 });
      })
      .mockImplementationOnce(async (url: string, init?: RequestInit) => {
        seen.push([url, headerValue(init, "x-csrf-token")]);
        return new Response(null, { status: 403 });
      })
      .mockResolvedValueOnce(responseWithToken("current-tab-token"))
      .mockImplementationOnce(async (url: string, init?: RequestInit) => {
        seen.push([url, headerValue(init, "x-csrf-token")]);
        return responseWithToken("rotated-again");
      })
      .mockImplementationOnce(async (url: string) => {
        seen.push([url, null]);
        return new Response(null, { status: 200 });
      });
    vi.stubGlobal("fetch", fetchMock);
    const { adminFetch, API_URL, cacheCsrfToken } = await freshApi();
    cacheCsrfToken("stale-tab-token");

    const response = await adminFetch("/api/auth/session");

    expect(response.status).toBe(200);
    expect(seen).toEqual([
      [`${API_URL}/api/auth/session`, null],
      [`${API_URL}/api/auth/refresh`, "stale-tab-token"],
      [`${API_URL}/api/auth/refresh`, "current-tab-token"],
      [`${API_URL}/api/auth/session`, null],
    ]);
  });

  it("shares one refresh across concurrent 401 responses", async () => {
    let sessionCalls = 0;
    let refreshCalls = 0;
    const fetchMock = vi.fn(async (url: string) => {
      if (url.endsWith("/api/auth/refresh")) {
        refreshCalls += 1;
        return responseWithToken("rotated-token");
      }
      sessionCalls += 1;
      return new Response(null, { status: sessionCalls <= 2 ? 401 : 200 });
    });
    vi.stubGlobal("fetch", fetchMock);
    const { adminFetch, cacheCsrfToken } = await freshApi();
    cacheCsrfToken("initial-token");

    const responses = await Promise.all([
      adminFetch("/api/auth/session"),
      adminFetch("/api/auth/session"),
    ]);

    expect(responses.map((response) => response.status)).toEqual([200, 200]);
    expect(refreshCalls).toBe(1);
    expect(sessionCalls).toBe(4);
  });
});
