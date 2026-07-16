/**
 * Backend API erişimi — taban URL env'den gelir, koda gömülmez.
 *
 * Oturumlar HttpOnly `sd_access`/`sd_refresh` çerezleriyle taşınır
 * (`backend/src/auth/session.rs`); bu dosya çerezi asla göremez/saklamaz,
 * yalnızca her istekte `credentials: "include"` gönderir ve mutasyonlarda
 * (`GET`/`HEAD` dışı) `sd_csrf` değerini `x-csrf-token` başlığına kopyalar
 * (double-submit — `require_cookie_request`). Localhost'ta API host'unun
 * çerezi `document.cookie` ile görülebilir. Ayrı frontend/API hostlarında
 * host-only çerez görünmez; o durumda güvenli `/api/auth/csrf` ucu değeri
 * yansıtır ve bu modül yalnız bellekte cache'ler.
 *
 * Faz 2: OAuth ile giren yayıncılar da aynı HttpOnly cookie session'ı kullanır
 * (`oauth_login.rs::callback` artık token fragment'ı yerine Set-Cookie döner);
 * `userFetch` browser'da yalnız cookie oturumuna dayanır. Bearer JWT yalnız
 * SDK/CLI/server-to-server istemciler için backend tarafında kalır.
 */

export const API_URL = (process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080").replace(
  /\/+$/,
  "",
);

function frontendSharesApiCookieHost(): boolean {
  if (typeof window === "undefined") {
    return true;
  }
  try {
    const api = new URL(API_URL, window.location.origin);
    return api.hostname === window.location.hostname;
  } catch {
    return false;
  }
}

function csrfCookie(): string | null {
  if (typeof document === "undefined" || !frontendSharesApiCookieHost()) {
    return null;
  }
  const match = document.cookie
    .split("; ")
    .find((row) => row.startsWith("sd_csrf="));
  if (!match) {
    return null;
  }
  try {
    return decodeURIComponent(match.slice("sd_csrf=".length));
  } catch {
    return null;
  }
}

function isSafeMethod(method: string): boolean {
  return method === "GET" || method === "HEAD";
}

function validCsrfToken(value: unknown): string | null {
  return typeof value === "string" && value.length > 0 ? value : null;
}

let cachedCsrfToken: string | null = null;
let csrfBootstrapInFlight: Promise<string | null> | null = null;

/**
 * Remember a token returned in a login/session/refresh response. The value is
 * deliberately module-memory only: a new tab or OAuth full-page redirect
 * bootstraps from the API cookie again instead of persisting stale auth state.
 */
export function cacheCsrfToken(token: string | null | undefined): void {
  cachedCsrfToken = validCsrfToken(token);
}

async function tokenFromResponse(response: Response): Promise<string | null> {
  try {
    const data = (await response.clone().json()) as { csrf_token?: unknown };
    return validCsrfToken(data.csrf_token);
  } catch {
    return null;
  }
}

/**
 * Resolve the double-submit value. A visible cookie always wins (localhost
 * and same-host deployments); otherwise share one credentialed bootstrap
 * request across all callers in this browsing context.
 */
async function currentCsrfToken(forceBootstrap = false): Promise<string | null> {
  const cookieToken = csrfCookie();
  if (cookieToken) {
    cachedCsrfToken = cookieToken;
    return cookieToken;
  }
  if (!forceBootstrap && cachedCsrfToken) {
    return cachedCsrfToken;
  }
  if (!csrfBootstrapInFlight) {
    if (forceBootstrap) {
      cachedCsrfToken = null;
    }
    csrfBootstrapInFlight = fetch(`${API_URL}/api/auth/csrf`, {
      method: "GET",
      credentials: "include",
      cache: "no-store",
    })
      .then(async (response) => {
        if (!response.ok) {
          return null;
        }
        const token = await tokenFromResponse(response);
        cachedCsrfToken = token;
        return token;
      })
      .catch(() => null)
      .finally(() => {
        csrfBootstrapInFlight = null;
      });
  }
  return csrfBootstrapInFlight;
}

/** In-flight session refresh, shared across concurrent 401s in this browsing context. */
let refreshInFlight: Promise<boolean> | null = null;

async function sendRefresh(csrf: string): Promise<Response> {
  return fetch(`${API_URL}/api/auth/refresh`, {
    method: "POST",
    credentials: "include",
    headers: { "x-csrf-token": csrf },
  });
}

async function performRefresh(): Promise<boolean> {
  let csrf = await currentCsrfToken();
  if (!csrf) {
    return false;
  }

  let response = await sendRefresh(csrf);
  if (response.status === 403) {
    // Another tab may have rotated the API-host cookie. Re-read it through the
    // bootstrap endpoint and retry only when the value actually changed; a
    // genuine authorization 403 must never be replayed.
    const current = await currentCsrfToken(true);
    if (!current || current === csrf) {
      return false;
    }
    csrf = current;
    response = await sendRefresh(csrf);
  }
  if (!response.ok) {
    return false;
  }

  const rotated = await tokenFromResponse(response);
  if (rotated) {
    cachedCsrfToken = rotated;
  } else {
    // The backend normally returns BrowserSession JSON. This fallback also
    // tolerates a rolling deployment with an older/empty refresh response.
    await currentCsrfToken(true);
  }
  return true;
}

async function refreshSession(): Promise<boolean> {
  if (!refreshInFlight) {
    refreshInFlight = performRefresh()
      .catch(() => false)
      .finally(() => {
        refreshInFlight = null;
      });
  }
  return refreshInFlight;
}

async function sendCookieRequest(
  path: string,
  init: RequestInit,
  headers: Headers,
  method: string,
): Promise<Response> {
  let sentCsrf: string | null = null;
  if (!isSafeMethod(method)) {
    sentCsrf = await currentCsrfToken();
    if (sentCsrf) {
      headers.set("x-csrf-token", sentCsrf);
    }
  }

  const request = { ...init, headers, credentials: "include" as const };
  let response = await fetch(`${API_URL}${path}`, request);
  if (!isSafeMethod(method) && response.status === 403) {
    const current = await currentCsrfToken(true);
    if (current && current !== sentCsrf) {
      headers.set("x-csrf-token", current);
      response = await fetch(`${API_URL}${path}`, request);
    }
  }
  return response;
}

async function rememberSessionToken(path: string, response: Response): Promise<void> {
  if (path === "/api/auth/logout" && response.ok) {
    cachedCsrfToken = null;
    return;
  }
  if (path !== "/api/auth/session" || !response.ok) {
    return;
  }
  const token = await tokenFromResponse(response);
  if (token) {
    cachedCsrfToken = token;
  }
}

/** Cookie-session fetch shared by admin and broadcaster panels. Retries once via `/api/auth/refresh` on a 401. */
async function cookieFetch(path: string, init: RequestInit = {}): Promise<Response> {
  const method = (init.method ?? "GET").toUpperCase();
  const headers = new Headers(init.headers);
  if (init.body && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }

  const response = await sendCookieRequest(path, init, headers, method);
  if (response.status !== 401) {
    await rememberSessionToken(path, response);
    return response;
  }
  const refreshed = await refreshSession();
  if (!refreshed) {
    return response;
  }
  const retried = await sendCookieRequest(path, init, headers, method);
  await rememberSessionToken(path, retried);
  return retried;
}

/** Admin panel fetch — `path` `/admin/...` veya `/api/...` ile başlar. Oturum yalnızca HttpOnly cookie'dedir. */
export async function adminFetch(path: string, init: RequestInit = {}): Promise<Response> {
  return cookieFetch(path, init);
}

/**
 * Yayıncı panel fetch — dashboard sayfaları bunu kullanır. Faz 2: browser
 * oturumu yalnız HttpOnly cookie'dedir; eski bearer önceliği kaldırıldı
 * (bearer yalnız SDK/CLI için backend `authz`'de desteklenir).
 */
export async function userFetch(path: string, init: RequestInit = {}): Promise<Response> {
  return cookieFetch(path, init);
}
