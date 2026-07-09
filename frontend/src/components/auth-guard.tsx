"use client";

import { useEffect, useState, type ReactNode } from "react";
import { getAdminToken, getUserToken } from "@/src/lib/api";

type Props = {
  children: ReactNode;
  /** Hangi token türü kontrol edilecek — yayıncı paneli mi admin konsolu mu. */
  tokenKind: "user" | "admin";
  /** Token yoksa yönlendirilecek giriş sayfası (locale'li). */
  loginHref: string;
};

/**
 * Dashboard/admin layout'larını sarmalar: token localStorage'da yoksa
 * içerik hiç render edilmeden giriş sayfasına yönlendirir. Token client-only
 * olduğu için bu kontrol sunucu tarafında yapılamaz — layout'lar async
 * server component kaldığından koruma burada, ayrı bir client bileşende.
 */
export function AuthGuard({ children, tokenKind, loginHref }: Props) {
  const [authorized, setAuthorized] = useState(false);

  useEffect(() => {
    const token = tokenKind === "admin" ? getAdminToken() : getUserToken();
    if (!token) {
      window.location.assign(loginHref);
      return;
    }
    setAuthorized(true);
  }, [tokenKind, loginHref]);

  if (!authorized) {
    return null;
  }
  return <>{children}</>;
}
