"use client";

import { useEffect, useState } from "react";
import { API_URL } from "@/src/lib/api";
import { clearPending2faEmail, readPending2faEmail } from "@/src/lib/auth-client";

type Labels = {
  codeLabel: string;
  codePlaceholder: string;
  submitLabel: string;
  submittingLabel: string;
  errorInvalid: string;
  errorTooMany: string;
  errorNetwork: string;
  errorSessionMissing: string;
};

type Props = {
  labels: Labels;
  /** Başarılı doğrulamada gidilecek yol (locale'li dashboard). */
  dashboardHref: string;
};

export function VerifyCodeForm({ labels, dashboardHref }: Props) {
  const [code, setCode] = useState("");
  const [email, setEmail] = useState<string | null>(null);
  const [ready, setReady] = useState(false);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // sessionStorage sadece client'ta var — hydration sonrası oku.
  useEffect(() => {
    setEmail(readPending2faEmail());
    setReady(true);
  }, []);

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!email) {
      setError(labels.errorSessionMissing);
      return;
    }
    setPending(true);
    setError(null);
    try {
      const response = await fetch(`${API_URL}/api/auth/2fa`, {
        method: "POST",
        credentials: "include",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ email, code }),
      });
      if (response.ok) {
        const data = (await response.json()) as { authenticated?: boolean };
        if (data.authenticated) {
          clearPending2faEmail();
          window.location.assign(dashboardHref);
          return;
        }
      }
      setError(response.status === 429 ? labels.errorTooMany : labels.errorInvalid);
    } catch {
      setError(labels.errorNetwork);
    }
    setPending(false);
  }

  return (
    <form className="mt-6 space-y-4" onSubmit={handleSubmit}>
      {ready && !email ? (
        <p className="rounded-lg border border-warning/30 bg-warning/10 px-3 py-2 text-sm text-ink" role="alert">
          {labels.errorSessionMissing}
        </p>
      ) : null}
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
      {error ? (
        <p className="rounded-lg border border-danger/25 bg-danger/10 px-3 py-2 text-sm text-ink" role="alert">
          {error}
        </p>
      ) : null}
      <button
        className="inline-flex h-11 w-full items-center justify-center rounded-full bg-primary px-5 text-sm font-semibold text-white transition hover:bg-primary-hover disabled:opacity-60"
        disabled={pending || code.length !== 6}
        type="submit"
      >
        {pending ? labels.submittingLabel : labels.submitLabel}
      </button>
    </form>
  );
}
