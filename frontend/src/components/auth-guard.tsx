"use client";

import { useEffect, useState, type ReactNode } from "react";
import { adminFetch, userFetch } from "@/src/lib/api";

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

    function redirectToLogin(reason: "session" | "network") {
      const url = new URL(loginHref, window.location.origin);
      url.searchParams.set("auth_error", reason);
      window.location.assign(`${url.pathname}${url.search}`);
    }

    async function check() {
      try {
        const fetcher = tokenKind === "admin" ? adminFetch : userFetch;
        const response = await fetcher("/api/auth/session");
        if (cancelled) {
          return;
        }
        if (!response.ok) {
          redirectToLogin("session");
          return;
        }
        const data = (await response.json()) as { authenticated?: boolean; role?: string };
        const roleAllowed = tokenKind === "admin" ? data.role === "admin" : true;
        if (data.authenticated && roleAllowed) {
          setAuthorized(true);
        } else {
          redirectToLogin("session");
        }
      } catch {
        if (!cancelled) {
          redirectToLogin("network");
        }
      }
    }

    void check();
    return () => {
      cancelled = true;
    };
  }, [tokenKind, loginHref]);

  if (!authorized) {
    return null;
  }
  return <>{children}</>;
}
