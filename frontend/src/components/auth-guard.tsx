"use client";

import { useEffect, useState, type ReactNode } from "react";
import { adminFetch, getUserToken, userFetch } from "@/src/lib/api";

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
 * Şifre/2FA ile giren kullanıcılar için oturum HttpOnly çerezdedir ve
 * JS'ten görünmez, bu yüzden kontrol `GET /api/auth/session`'a gerçek bir
 * istek atarak yapılır (backend `AuthUser` extractor'ı çerezi doğrular).
 * OAuth ile giren yayıncılar hâlâ localStorage'daki kalıcı Bearer token'ı
 * kullanır (`oauth-complete.tsx`) — o token varsa ağ isteğine gerek yok,
 * `userFetch` zaten her istekte ekliyor ve backend geçersizse 401 döner.
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
      if (tokenKind === "user" && getUserToken()) {
        if (!cancelled) {
          setAuthorized(true);
        }
        return;
      }
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
