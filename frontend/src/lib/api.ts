/**
 * Backend API erişimi — taban URL env'den gelir, koda gömülmez.
 *
 * Admin oturum token'ı localStorage'da tutulur; tüm admin sayfaları
 * `adminFetch` ile Bearer başlığını otomatik ekler. Token yoksa veya
 * 401 dönerse çağıran taraf login sayfasına yönlendirir.
 */

export const API_URL = (process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080").replace(
  /\/+$/,
  "",
);

const ADMIN_TOKEN_KEY = "sd.admin.token";

export function getAdminToken(): string | null {
  if (typeof window === "undefined") {
    return null;
  }
  return window.localStorage.getItem(ADMIN_TOKEN_KEY);
}

export function setAdminToken(token: string): void {
  if (typeof window !== "undefined") {
    window.localStorage.setItem(ADMIN_TOKEN_KEY, token);
  }
}

export function clearAdminToken(): void {
  if (typeof window !== "undefined") {
    window.localStorage.removeItem(ADMIN_TOKEN_KEY);
  }
}

/** Admin token'lı fetch — `path` `/admin/...` veya `/api/...` ile başlar. */
export async function adminFetch(path: string, init: RequestInit = {}): Promise<Response> {
  const headers = new Headers(init.headers);
  const token = getAdminToken();
  if (token) {
    headers.set("Authorization", `Bearer ${token}`);
  }
  if (init.body && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }
  return fetch(`${API_URL}${path}`, { ...init, headers });
}
