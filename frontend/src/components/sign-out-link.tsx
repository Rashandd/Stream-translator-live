"use client";

import Link from "next/link";
import { clearAdminToken, clearUserToken } from "@/src/lib/api";

type Props = {
  href: string;
  label: string;
  className?: string;
  /** Hangi oturum token'ı temizlenecek. */
  tokenKind: "user" | "admin";
};

/** Çıkış linki — tıklandığında ilgili token'ı localStorage'dan siler. */
export function SignOutLink({ href, label, className, tokenKind }: Props) {
  return (
    <Link
      className={className}
      href={href}
      onClick={() => (tokenKind === "admin" ? clearAdminToken() : clearUserToken())}
    >
      {label}
    </Link>
  );
}
