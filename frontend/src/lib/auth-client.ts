/**
 * Yayıncı auth akışı yardımcıları (client-only).
 *
 * `POST /api/auth/login` tek uçtur: admin kimliği admin token'ı, yayıncı
 * kimliği yayıncı token'ı döndürür; SMTP yapılandırıldıysa yayıncı için
 * 2FA kodu maillenir ve yanıt `two_factor_required` olur. 2FA bekleyen
 * e-posta sessionStorage'da tutulur — verify sayfası oradan okur (URL'e
 * e-posta yazılmaz).
 */

import { API_URL, setAdminToken, setUserToken } from "@/src/lib/api";

const PENDING_2FA_KEY = "sd.pending2fa.email";

export type LoginOutcome =
  | { kind: "admin" }
  | { kind: "broadcaster" }
  | { kind: "twofactor" }
  | { kind: "invalid" }
  | { kind: "unavailable" }
  | { kind: "network" };

export async function login(email: string, password: string): Promise<LoginOutcome> {
  try {
    const response = await fetch(`${API_URL}/api/auth/login`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ email, password }),
    });
    if (response.ok) {
      const data = (await response.json()) as {
        token?: string;
        role?: string;
        two_factor_required?: boolean;
      };
      if (data.two_factor_required) {
        savePending2faEmail(email);
        return { kind: "twofactor" };
      }
      if (data.token && data.role === "admin") {
        setAdminToken(data.token);
        return { kind: "admin" };
      }
      if (data.token) {
        setUserToken(data.token);
        return { kind: "broadcaster" };
      }
      return { kind: "invalid" };
    }
    return response.status === 503 ? { kind: "unavailable" } : { kind: "invalid" };
  } catch {
    return { kind: "network" };
  }
}

export function savePending2faEmail(email: string): void {
  if (typeof window !== "undefined") {
    window.sessionStorage.setItem(PENDING_2FA_KEY, email);
  }
}

export function readPending2faEmail(): string | null {
  if (typeof window === "undefined") {
    return null;
  }
  return window.sessionStorage.getItem(PENDING_2FA_KEY);
}

export function clearPending2faEmail(): void {
  if (typeof window !== "undefined") {
    window.sessionStorage.removeItem(PENDING_2FA_KEY);
  }
}
