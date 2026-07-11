"use client";

import Link from "next/link";
import { useState } from "react";
import { login } from "@/src/lib/auth-client";

type Labels = {
  emailLabel: string;
  passwordLabel: string;
  forgotLabel: string;
  submitLabel: string;
  submittingLabel: string;
  errorInvalid: string;
  errorUnavailable: string;
  errorNetwork: string;
};

type Props = {
  labels: Labels;
  /** Şifremi unuttum sayfası (locale'li). */
  forgotHref: string;
  /** Yayıncı girişinde gidilecek yol (locale'li dashboard). */
  dashboardHref: string;
  /** Admin kimliğiyle girilirse gidilecek yol (locale'li admin). */
  adminHref: string;
  /** 2FA kodu maillendiyse gidilecek doğrulama sayfası (locale'li). */
  verifyHref: string;
};

export function LoginForm({ labels, forgotHref, dashboardHref, adminHref, verifyHref }: Props) {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setPending(true);
    setError(null);
    const outcome = await login(email, password);
    switch (outcome.kind) {
      case "broadcaster":
        window.location.assign(dashboardHref);
        return;
      case "admin":
        window.location.assign(adminHref);
        return;
      case "twofactor":
        window.location.assign(verifyHref);
        return;
      case "unavailable":
        setError(labels.errorUnavailable);
        break;
      case "network":
        setError(labels.errorNetwork);
        break;
      default:
        setError(labels.errorInvalid);
    }
    setPending(false);
  }

  return (
    <form className="mt-6 space-y-4" onSubmit={handleSubmit}>
      <div>
        <label className="text-sm font-semibold text-ink" htmlFor="email">{labels.emailLabel}</label>
        <input
          autoComplete="email"
          className="mt-1.5 h-11 w-full rounded-lg border border-border bg-surface px-3 text-sm text-ink outline-none transition focus:border-blue"
          id="email"
          name="email"
          onChange={(event) => setEmail(event.target.value)}
          required
          type="email"
          value={email}
        />
      </div>
      <div>
        <div className="flex items-center justify-between">
          <label className="text-sm font-semibold text-ink" htmlFor="password">{labels.passwordLabel}</label>
          <Link className="text-sm font-semibold text-blue hover:text-ink" href={forgotHref}>
            {labels.forgotLabel}
          </Link>
        </div>
        <input
          autoComplete="current-password"
          className="mt-1.5 h-11 w-full rounded-lg border border-border bg-surface px-3 text-sm text-ink outline-none transition focus:border-blue"
          id="password"
          onChange={(event) => setPassword(event.target.value)}
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
