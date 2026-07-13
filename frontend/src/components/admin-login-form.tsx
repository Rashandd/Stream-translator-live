"use client";

import { useState } from "react";
import { API_URL } from "@/src/lib/api";

type Labels = {
  emailLabel: string;
  emailPlaceholder: string;
  passwordLabel: string;
  passwordPlaceholder: string;
  submitLabel: string;
  submittingLabel: string;
  errorInvalid: string;
  errorUnavailable: string;
  errorNetwork: string;
};

type Props = {
  labels: Labels;
  /** Başarılı girişte gidilecek locale'li yol (örn. /en/admin). */
  redirectTo: string;
  /** 2FA gerekiyorsa gidilecek locale'li admin verify yolu. */
  verifyHref: string;
};

export function AdminLoginForm({ labels, redirectTo, verifyHref }: Props) {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setPending(true);
    setError(null);
    try {
      const response = await fetch(`${API_URL}/api/auth/login`, {
        method: "POST",
        credentials: "include",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ email, password }),
      });
      if (response.ok) {
        const data = (await response.json()) as {
          authenticated?: boolean;
          role?: string;
          two_factor_required?: boolean;
        };
        if (data.two_factor_required && data.role === "admin") {
          window.location.assign(verifyHref);
          return;
        }
        if (data.authenticated && data.role === "admin") {
          window.location.assign(redirectTo);
          return;
        }
        setError(labels.errorInvalid);
        return;
      }
      setError(response.status === 503 ? labels.errorUnavailable : labels.errorInvalid);
    } catch {
      setError(labels.errorNetwork);
    } finally {
      setPending(false);
    }
  }

  return (
    <form className="mt-6 space-y-4" onSubmit={handleSubmit}>
      <div>
        <label className="text-sm font-semibold text-ink" htmlFor="email">
          {labels.emailLabel}
        </label>
        <input
          autoComplete="email"
          className="mt-1.5 h-11 w-full rounded-lg border border-border bg-surface px-3 text-sm text-ink outline-none transition focus:border-blue"
          id="email"
          name="email"
          onChange={(event) => setEmail(event.target.value)}
          placeholder={labels.emailPlaceholder}
          required
          type="email"
          value={email}
        />
      </div>
      <div>
        <label className="text-sm font-semibold text-ink" htmlFor="password">
          {labels.passwordLabel}
        </label>
        <input
          autoComplete="current-password"
          className="mt-1.5 h-11 w-full rounded-lg border border-border bg-surface px-3 text-sm text-ink outline-none transition focus:border-blue"
          id="password"
          name="password"
          onChange={(event) => setPassword(event.target.value)}
          placeholder={labels.passwordPlaceholder}
          required
          type="password"
          value={password}
        />
      </div>
      {error ? (
        <p className="rounded-lg border border-danger/25 bg-danger/10 px-3 py-2 text-sm text-ink" role="alert">
          {error}
        </p>
      ) : null}
      <button
        className="inline-flex h-11 w-full items-center justify-center rounded-full bg-primary px-5 text-sm font-semibold text-on-primary transition hover:bg-primary-hover disabled:opacity-60"
        disabled={pending}
        type="submit"
      >
        {pending ? labels.submittingLabel : labels.submitLabel}
      </button>
    </form>
  );
}
