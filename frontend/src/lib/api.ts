/**
 * Backend API erişimi — taban URL env'den gelir, koda gömülmez.
 *
 * Oturumlar HttpOnly `sd_access`/`sd_refresh` çerezleriyle taşınır
 * (`backend/src/auth/session.rs`); bu dosya çerezi asla göremez/saklamaz,
 * yalnızca her istekte `credentials: "include"` gönderir ve mutasyonlarda
 * (`GET`/`HEAD` dışı) okunabilir `sd_csrf` çerezini `x-csrf-token`
 * başlığına kopyalar (double-submit — `require_cookie_request`).
 *
 * OAuth ile giren yayıncılar için ayrı bir kalıcı Bearer JWT mekanizması
 * hâlâ geçerli (`oauth-complete.tsx`, `USER_TOKEN_KEY`) — o token'lar cookie
 * session'dan bağımsız, `oauth_login.rs::callback` fragment'la iletir ve
 * asla süresi dolmaz kadar uzun yaşar; bu yüzden `userFetch` ikisini de
 * destekler (Bearer varsa öncelik, yoksa cookie).
 */

export const API_URL = (process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080").replace(
  /\/+$/,
  "",
);

const USER_TOKEN_KEY = "sd.user.token";

export function getUserToken(): string | null {
  if (typeof window === "undefined") {
    return null;
  }
  return window.localStorage.getItem(USER_TOKEN_KEY);
}

export function setUserToken(token: string): void {
  if (typeof window !== "undefined") {
    window.localStorage.setItem(USER_TOKEN_KEY, token);
  }
}

export function clearUserToken(): void {
  if (typeof window !== "undefined") {
    window.localStorage.removeItem(USER_TOKEN_KEY);
  }
}

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
    refreshInFlight = fetch(`${API_URL}/api/auth/refresh`, {
      method: "POST",
      credentials: "include",
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
  return fetch(`${API_URL}${path}`, request);
}

/** Admin panel fetch — `path` `/admin/...` veya `/api/...` ile başlar. Oturum yalnızca HttpOnly cookie'dedir. */
export async function adminFetch(path: string, init: RequestInit = {}): Promise<Response> {
  return cookieFetch(path, init);
}

/**
 * Yayıncı panel fetch — dashboard sayfaları bunu kullanır. `sd.user.token`
 * doluysa (OAuth ile girmiş yayıncı) Bearer başlığı eklenir; boşsa çerez
 * oturumuna (şifre/2FA ile giren yayıncı) düşer.
 */
export async function userFetch(path: string, init: RequestInit = {}): Promise<Response> {
  const token = getUserToken();
  if (!token) {
    return cookieFetch(path, init);
  }
  const headers = new Headers(init.headers);
  headers.set("Authorization", `Bearer ${token}`);
  if (init.body && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }
  return fetch(`${API_URL}${path}`, { ...init, headers });
}
