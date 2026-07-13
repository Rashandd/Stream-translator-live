"use client";

import { useEffect, useState, type ReactNode } from "react";
import { adminFetch, userFetch } from "@/src/lib/api";
import {
  AUTH_CHANNEL,
  hasSeenSession,
  LOGOUT_EVENT_KEY,
  markSessionSeen,
  storeAuthNotice,
  type AuthNoticeCode,
} from "@/src/lib/auth-notice";

type Props = {
  children: ReactNode;
  /** Hangi oturum kontrol edilecek — yayıncı paneli mi admin konsolu mu. */
  tokenKind: "user" | "admin";
  /** Oturum yoksa yönlendirilecek giriş sayfası (locale'li). */
  loginHref: string;
};

/**
 * Dashboard/admin layout'larını sarmalar: yetkili bir oturum yoksa içerik
 * hiç render edilmeden giriş sayfasına yönlendirir.
 *
 * Bütün browser auth türleri (şifre, 2FA, OAuth) artık aynı HttpOnly cookie
 * session'ı kullanır (Faz 2); oturum JS'ten görünmez, bu yüzden kontrol her
 * durumda `GET /api/auth/session`'a gerçek bir istek atarak yapılır (backend
 * `AuthUser` extractor'ı çerezi doğrular). localStorage bearer kısayolu
 * kaldırıldı — eski/süresi dolmuş bir token artık geçerli cookie session'ı
 * gölgeleyemez.
 */
export function AuthGuard({ children, tokenKind, loginHref }: Props) {
  const [authorized, setAuthorized] = useState(false);

  useEffect(() => {
    let cancelled = false;

    function redirectToLogin(reason: AuthNoticeCode) {
      storeAuthNotice(reason);
      window.location.replace(loginHref);
    }

    async function check() {
      try {
        const fetcher = tokenKind === "admin" ? adminFetch : userFetch;
        const response = await fetcher("/api/auth/session");
        if (cancelled) {
          return;
        }
        if (!response.ok) {
          redirectToLogin(hasSeenSession() ? "session_expired" : "sign_in_required");
          return;
        }
        const data = (await response.json()) as { authenticated?: boolean; role?: string };
        const roleAllowed = tokenKind === "admin" ? data.role === "admin" : true;
        if (data.authenticated && roleAllowed) {
          markSessionSeen();
          setAuthorized(true);
        } else {
          redirectToLogin(hasSeenSession() ? "session_expired" : "sign_in_required");
        }
      } catch {
        if (!cancelled) {
          redirectToLogin("session_unavailable");
        }
      }
    }

    const handleLogout = () => redirectToLogin("sign_in_required");
    const handleStorage = (event: StorageEvent) => {
      if (event.key === LOGOUT_EVENT_KEY) handleLogout();
    };
    const channel = typeof BroadcastChannel === "undefined" ? null : new BroadcastChannel(AUTH_CHANNEL);
    if (channel) channel.onmessage = handleLogout;
    window.addEventListener("storage", handleStorage);
    void check();
    return () => {
      cancelled = true;
      channel?.close();
      window.removeEventListener("storage", handleStorage);
    };
  }, [tokenKind, loginHref]);

  if (!authorized) {
    return null;
  }
  return <>{children}</>;
}
