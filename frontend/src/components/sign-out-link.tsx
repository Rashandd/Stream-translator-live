"use client";

import { useState } from "react";
import { adminFetch, userFetch } from "@/src/lib/api";
import { publishLogout } from "@/src/lib/auth-notice";

type Props = {
  href: string;
  label: string;
  errorLabel: string;
  className?: string;
  tokenKind: "user" | "admin";
};

export function SignOutLink({ href, label, errorLabel, className, tokenKind }: Props) {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleClick(event: React.MouseEvent<HTMLAnchorElement>) {
    event.preventDefault();
    setPending(true);
    setError(null);
    try {
      const fetcher = tokenKind === "admin" ? adminFetch : userFetch;
      const response = await fetcher("/api/auth/logout", { method: "POST" });
      if (!response.ok) {
        setError(errorLabel);
        return;
      }
      publishLogout();
      window.location.replace(href);
    } catch {
      setError(errorLabel);
    } finally {
      setPending(false);
    }
  }

  return (
    <span className="relative inline-flex flex-col items-end">
      <a
        aria-disabled={pending}
        className={className}
        href={href}
        onClick={(event) => void handleClick(event)}
      >
        {label}
      </a>
      {error ? <span className="absolute right-0 top-full mt-1 whitespace-nowrap text-xs text-danger" role="alert">{error}</span> : null}
    </span>
  );
}
