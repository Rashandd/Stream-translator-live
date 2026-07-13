/**
 * Yayıncı auth akışı yardımcıları (client-only).
 *
 * `POST /api/auth/login` tek uçtur. Tarayıcıdan çağrıldığında (her zaman,
 * `Origin` başlığı yüzünden) yanıt bir bearer token değil, HttpOnly
 * `sd_access`/`sd_refresh`/`sd_csrf` çerezleri kuran bir `BrowserSession`
 * gövdesidir (`backend/src/auth/session.rs::session_response`) — rol
 * `data.role`'den okunur, çerezler zaten `Set-Cookie` ile kurulmuştur,
 * burada saklanacak bir token yoktur. SMTP yapılandırıldıysa yayıncı için
 * 2FA kodu maillenir ve yanıt `two_factor_required` olur. Bekleyen kimlik
 * yalnız backend'in HttpOnly `sd_auth_challenge` çerezindedir; JavaScript
 * e-posta, subject veya challenge id saklamaz.
 */

import { API_URL } from "@/src/lib/api";

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
      credentials: "include",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ email, password }),
    });
    if (response.ok) {
      const data = (await response.json()) as {
        authenticated?: boolean;
        role?: string;
        two_factor_required?: boolean;
      };
      if (data.two_factor_required) {
        return { kind: "twofactor" };
      }
      if (data.authenticated && data.role === "admin") {
        return { kind: "admin" };
      }
      if (data.authenticated) {
        return { kind: "broadcaster" };
      }
      return { kind: "invalid" };
    }
    return response.status === 503 ? { kind: "unavailable" } : { kind: "invalid" };
  } catch {
    return { kind: "network" };
  }
}
