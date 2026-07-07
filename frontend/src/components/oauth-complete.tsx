"use client";

import { useEffect, useState } from "react";
import { setUserToken } from "@/src/lib/api";

type Labels = {
  backLabel: string;
};

type Props = {
  labels: Labels;
  dashboardHref: string;
  loginHref: string;
};

/**
 * The backend redirects here with either `#token=...` (fragment — never
 * sent to a server, unlike a query param) on success, or `?error=...` on
 * failure. Reading the fragment requires the browser, hence `"use client"`.
 */
export function OAuthCompleteHandler({ labels, dashboardHref, loginHref }: Props) {
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const hashParams = new URLSearchParams(window.location.hash.replace(/^#/, ""));
    const token = hashParams.get("token");
    if (token) {
      setUserToken(token);
      window.location.assign(dashboardHref);
      return;
    }
    const queryError = new URLSearchParams(window.location.search).get("error");
    setError(queryError || "Sign-in failed.");
  }, [dashboardHref]);

  if (!error) {
    return null;
  }

  return (
    <div className="mt-6 space-y-4">
      <p className="rounded-lg border border-danger/25 bg-danger/10 px-3 py-2 text-sm text-ink" role="alert">
        {error}
      </p>
      <a className="block text-center text-sm font-semibold text-blue hover:text-ink" href={loginHref}>
        {labels.backLabel}
      </a>
    </div>
  );
}
