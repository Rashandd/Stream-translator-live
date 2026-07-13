/**
 * Backend API erişimi — taban URL env'den gelir, koda gömülmez.
 *
 * Oturumlar HttpOnly `sd_access`/`sd_refresh` çerezleriyle taşınır
 * (`backend/src/auth/session.rs`); bu dosya çerezi asla göremez/saklamaz,
 * yalnızca her istekte `credentials: "include"` gönderir ve mutasyonlarda
 * (`GET`/`HEAD` dışı) okunabilir `sd_csrf` çerezini `x-csrf-token`
 * başlığına kopyalar (double-submit — `require_cookie_request`).
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

function csrfCookie(): string | null {
  if (typeof document === "undefined") {
    return null;
  }
  const match = document.cookie
    .split("; ")
    .find((row) => row.startsWith("sd_csrf="));
  return match ? decodeURIComponent(match.slice("sd_csrf=".length)) : null;
}

function isSafeMethod(method: string): boolean {
  return method === "GET" || method === "HEAD";
}

/** In-flight session refresh, shared across concurrent 401s so two tabs/requests don't race-rotate the same refresh cookie. */
let refreshInFlight: Promise<boolean> | null = null;

async function refreshSession(): Promise<boolean> {
  if (!refreshInFlight) {
    const csrf = csrfCookie();
    refreshInFlight = fetch(`${API_URL}/api/auth/refresh`, {
      method: "POST",
      credentials: "include",
      headers: csrf ? { "x-csrf-token": csrf } : undefined,
    })
      .then((response) => response.ok)
      .catch(() => false)
      .finally(() => {
        refreshInFlight = null;
      });
  }
  return refreshInFlight;
}

/** Cookie-session fetch shared by admin and broadcaster panels. Retries once via `/api/auth/refresh` on a 401. */
async function cookieFetch(path: string, init: RequestInit = {}): Promise<Response> {
  const method = (init.method ?? "GET").toUpperCase();
  const headers = new Headers(init.headers);
  if (init.body && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }
  if (!isSafeMethod(method)) {
    const csrf = csrfCookie();
    if (csrf) {
      headers.set("x-csrf-token", csrf);
    }
  }
  const request = { ...init, headers, credentials: "include" as const };
  const response = await fetch(`${API_URL}${path}`, request);
  if (response.status !== 401) {
    return response;
  }
  const refreshed = await refreshSession();
  if (!refreshed) {
    return response;
  }
  if (!isSafeMethod(method)) {
    const csrf = csrfCookie();
    if (csrf) {
      headers.set("x-csrf-token", csrf);
    }
  }
  return fetch(`${API_URL}${path}`, request);
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
