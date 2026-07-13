"use client";

import { useEffect, useState } from "react";
import { userFetch } from "@/src/lib/api";

type ErrorCopy = {
  accountNotFound: string;
  emailExists: string;
  invalidState: string;
  providerUnavailable: string;
  oauthFailed: string;
  generic: string;
};

type Labels = {
  backLabel: string;
  errors: ErrorCopy;
};

type Props = {
  labels: Labels;
  dashboardHref: string;
  loginHref: string;
};

/**
 * A-02: the backend only ever puts an opaque, allow-listed code in `?error=`;
 * the text shown to the user comes from the dictionary here, never from the
 * URL. Unknown codes fall back to a generic message.
 */
function messageForCode(code: string | null, errors: ErrorCopy): string {
  switch (code) {
    case "account_not_found":
      return errors.accountNotFound;
    case "email_exists":
      return errors.emailExists;
    case "invalid_state":
      return errors.invalidState;
    case "provider_unavailable":
      return errors.providerUnavailable;
    case "oauth_failed":
      return errors.oauthFailed;
    default:
      return errors.generic;
  }
}

/**
 * Only same-origin relative routes are honored (mirror of the backend
 * `sanitize_return_to`), so a tampered `return_to` can never cause an open
 * redirect on the client either.
 */
function safeReturnTo(raw: string | null): string | null {
  if (!raw || !raw.startsWith("/") || raw.startsWith("//")) {
    return null;
  }
  if (raw.includes("\\") || raw.includes("://")) {
    return null;
  }
  return raw;
}

/**
 * Faz 2: on success the backend has already set the HttpOnly session cookies
 * on the redirect to this page — there is no token in the URL or in
 * `localStorage`. The page confirms the cookie session via the session
 * endpoint and then routes. On failure the backend appends `?error=<code>`
 * (an opaque A-02 code) which is mapped to dictionary copy. Runs in the
 * browser, hence `"use client"`.
 */
export function OAuthCompleteHandler({ labels, dashboardHref, loginHref }: Props) {
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const code = new URLSearchParams(window.location.search).get("error");
    if (code) {
      setError(messageForCode(code, labels.errors));
      return;
    }

    let cancelled = false;
    void (async () => {
      try {
        const response = await userFetch("/api/auth/session");
        if (cancelled) {
          return;
        }
        if (!response.ok) {
          setError(labels.errors.generic);
          return;
        }
        const returnTo = safeReturnTo(new URLSearchParams(window.location.search).get("return_to"));
        window.location.assign(returnTo ?? dashboardHref);
      } catch {
        if (!cancelled) {
          setError(labels.errors.generic);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [dashboardHref, labels.errors]);

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
