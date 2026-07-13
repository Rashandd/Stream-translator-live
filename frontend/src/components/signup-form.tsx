"use client";

import Link from "next/link";
import { useState } from "react";
import { API_URL } from "@/src/lib/api";

type ConsentLink = {
  label: string;
  href: string;
};

type ConsentLabels = {
  label: string;
  links: ConsentLink[];
};

type Labels = {
  emailLabel: string;
  passwordLabel: string;
  submitLabel: string;
  submittingLabel: string;
  errorTaken: string;
  errorInvalidFields: string;
  errorNetwork: string;
  requiredConsent: ConsentLabels;
  optionalConsent: ConsentLabels;
};

type Props = {
  labels: Labels;
  /** Başarılı kayıt + girişte gidilecek yol (locale'li dashboard). */
  dashboardHref: string;
  /** 2FA kodu maillendiyse gidilecek doğrulama sayfası (locale'li). */
  verifyHref: string;
};

export function SignupForm({ labels, dashboardHref, verifyHref }: Props) {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [marketingConsent, setMarketingConsent] = useState(false);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setPending(true);
    setError(null);
    try {
      const response = await fetch(`${API_URL}/api/auth/signup`, {
        method: "POST",
        credentials: "include",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          email,
          password,
          legal_terms_accepted: true,
          marketing_consent: marketingConsent,
        }),
      });
      if (!response.ok) {
        setError(response.status === 409 ? labels.errorTaken : labels.errorInvalidFields);
        setPending(false);
        return;
      }
      const outcome = (await response.json()) as {
        authenticated?: boolean;
        role?: string;
        two_factor_required?: boolean;
      };
      if (outcome.two_factor_required) {
        window.location.assign(verifyHref);
        return;
      }
      if (outcome.authenticated && outcome.role === "broadcaster") {
        window.location.assign(dashboardHref);
        return;
      }
      setError(labels.errorNetwork);
    } catch {
      setError(labels.errorNetwork);
      setPending(false);
      return;
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
        <label className="text-sm font-semibold text-ink" htmlFor="password">{labels.passwordLabel}</label>
        <input
          autoComplete="new-password"
          className="mt-1.5 h-11 w-full rounded-lg border border-border bg-surface px-3 text-sm text-ink outline-none transition focus:border-blue"
          id="password"
          minLength={8}
          onChange={(event) => setPassword(event.target.value)}
          required
          type="password"
          value={password}
        />
      </div>
      <label className="flex gap-3 rounded-lg border border-border bg-surface p-3 text-xs leading-5 text-muted">
        <input
          checked={marketingConsent}
          className="mt-0.5 h-4 w-4 shrink-0 rounded border-border text-blue"
          onChange={(event) => setMarketingConsent(event.target.checked)}
          type="checkbox"
        />
        <span>
          {labels.optionalConsent.label}{" "}
          {labels.optionalConsent.links.map((link, index) => (
            <span key={link.href}>
              <Link className="font-semibold text-blue hover:text-ink" href={link.href}>
                {link.label}
              </Link>
              {index < labels.optionalConsent.links.length - 1 ? ", " : "."}
            </span>
          ))}
        </span>
      </label>
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
      <p className="text-xs leading-5 text-muted">
        {labels.requiredConsent.label}{" "}
        {labels.requiredConsent.links.map((link, index) => (
          <span key={link.href}>
            <Link className="font-semibold text-blue hover:text-ink" href={link.href}>
              {link.label}
            </Link>
            {index < labels.requiredConsent.links.length - 1 ? ", " : "."}
          </span>
        ))}
      </p>
    </form>
  );
}
