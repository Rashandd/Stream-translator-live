"use client";

import { useState } from "react";
import { API_URL } from "@/src/lib/api";

type Labels = {
  codeLabel: string;
  codePlaceholder: string;
  submitLabel: string;
  submittingLabel: string;
  resendLabel: string;
  resendingLabel: string;
  resendSent: string;
  errorInvalid: string;
  errorTooMany: string;
  errorNetwork: string;
  errorSessionMissing: string;
};

type Props = {
  labels: Labels;
  dashboardHref: string;
  adminHref: string;
};

/**
 * Ortak broadcaster/admin 2FA formu. Pending identity yalnız HttpOnly
 * `sd_auth_challenge` cookie'sindedir; body yalnız 6 haneli kodu taşır.
 */
export function VerifyCodeForm({ labels, dashboardHref, adminHref }: Props) {
  const [code, setCode] = useState("");
  const [pending, setPending] = useState(false);
  const [resending, setResending] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setPending(true);
    setError(null);
    setMessage(null);
    try {
      const response = await fetch(`${API_URL}/api/auth/2fa`, {
        method: "POST",
        credentials: "include",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ code }),
      });
      if (response.ok) {
        const data = (await response.json()) as { authenticated?: boolean; role?: string };
        if (data.authenticated && data.role === "admin") {
          window.location.assign(adminHref);
          return;
        }
        if (data.authenticated && data.role === "broadcaster") {
          window.location.assign(dashboardHref);
          return;
        }
      }
      if (response.status === 429) {
        setError(labels.errorTooMany);
      } else if (response.status === 401) {
        setError(labels.errorInvalid);
      } else {
        setError(labels.errorSessionMissing);
      }
    } catch {
      setError(labels.errorNetwork);
    } finally {
      setPending(false);
    }
  }

  async function handleResend() {
    setResending(true);
    setError(null);
    setMessage(null);
    try {
      const response = await fetch(`${API_URL}/api/auth/2fa/resend`, {
        method: "POST",
        credentials: "include",
      });
      if (response.ok) {
        setMessage(labels.resendSent);
      } else if (response.status === 429) {
        setError(labels.errorTooMany);
      } else if (response.status === 401) {
        setError(labels.errorSessionMissing);
      } else {
        setError(labels.errorNetwork);
      }
    } catch {
      setError(labels.errorNetwork);
    } finally {
      setResending(false);
    }
  }

  return (
    <form className="mt-6 space-y-4" onSubmit={handleSubmit}>
      <div>
        <label className="text-sm font-semibold text-ink" htmlFor="code">{labels.codeLabel}</label>
        <input
          autoComplete="one-time-code"
          className="mt-1.5 h-12 w-full rounded-lg border border-border bg-surface px-3 text-center font-mono text-lg tracking-[0.4em] text-ink outline-none transition focus:border-blue"
          id="code"
          inputMode="numeric"
          maxLength={6}
          name="code"
          onChange={(event) => setCode(event.target.value.replace(/[^0-9]/g, ""))}
          pattern="[0-9]*"
          placeholder={labels.codePlaceholder}
          required
          type="text"
          value={code}
        />
      </div>
      {message ? (
        <p className="rounded-lg border border-success/25 bg-success/10 px-3 py-2 text-sm text-ink" role="status">
          {message}
        </p>
      ) : null}
      {error ? (
        <p className="rounded-lg border border-danger/25 bg-danger/10 px-3 py-2 text-sm text-ink" role="alert">
          {error}
        </p>
      ) : null}
      <button
        className="inline-flex h-11 w-full items-center justify-center rounded-full bg-primary px-5 text-sm font-semibold text-on-primary transition hover:bg-primary-hover disabled:opacity-60"
        disabled={pending || code.length !== 6}
        type="submit"
      >
        {pending ? labels.submittingLabel : labels.submitLabel}
      </button>
      <button
        className="inline-flex h-11 w-full items-center justify-center rounded-full border border-border bg-surface px-5 text-sm font-semibold text-ink transition hover:border-blue disabled:opacity-60"
        disabled={resending}
        onClick={() => void handleResend()}
        type="button"
      >
        {resending ? labels.resendingLabel : labels.resendLabel}
      </button>
    </form>
  );
}
