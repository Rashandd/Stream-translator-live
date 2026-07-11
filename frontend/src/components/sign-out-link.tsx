"use client";

import Link from "next/link";
import { adminFetch, clearUserToken, userFetch } from "@/src/lib/api";

type Props = {
  href: string;
  label: string;
  className?: string;
  /** Hangi oturum kapatılacak. */
  tokenKind: "user" | "admin";
};

/**
 * Çıkış linki — tıklandığında `POST /api/auth/logout`'u ateşler (cookie
 * session'ı sunucu tarafında iptal eder) ve yayıncı için ayrıca OAuth
 * Bearer token'ını localStorage'dan siler. Link'in varsayılan navigasyonu
 * engellenmiyor, bu yüzden istek beklenmeden gönderilir — sayfa ayrılsa
 * bile çerezler kısa TTL'leri (§ auth/session.rs) sayesinde zaten güvenli.
 */
export function SignOutLink({ href, label, className, tokenKind }: Props) {
  return (
    <Link
      className={className}
      href={href}
      onClick={() => {
        const fetcher = tokenKind === "admin" ? adminFetch : userFetch;
        void fetcher("/api/auth/logout", { method: "POST" });
        if (tokenKind === "user") {
          clearUserToken();
        }
      }}
    >
      {label}
    </Link>
  );
}
