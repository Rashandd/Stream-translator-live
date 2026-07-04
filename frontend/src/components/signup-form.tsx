"use client";

import { useState } from "react";
import { API_URL } from "@/src/lib/api";
import { login } from "@/src/lib/auth-client";

type Labels = {
  emailLabel: string;
  emailPlaceholder: string;
  passwordLabel: string;
  passwordPlaceholder: string;
  submitLabel: string;
  submittingLabel: string;
  errorTaken: string;
  errorInvalidFields: string;
  errorNetwork: string;
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
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setPending(true);
    setError(null);
    try {
      const response = await fetch(`${API_URL}/api/auth/signup`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ email, password }),
      });
      if (!response.ok) {
        setError(response.status === 409 ? labels.errorTaken : labels.errorInvalidFields);
        setPending(false);
        return;
      }
    } catch {
      setError(labels.errorNetwork);
      setPending(false);
      return;
    }

    // Hesap açıldı — aynı kimlikle normal giriş akışı (2FA kararını login verir).
    const outcome = await login(email, password);
    if (outcome.kind === "twofactor") {
      window.location.assign(verifyHref);
      return;
    }
    if (outcome.kind === "broadcaster") {
      window.location.assign(dashboardHref);
      return;
    }
    setError(labels.errorNetwork);
    setPending(false);
  }

  return (
    <form className="mt-6 space-y-4" onSubmit={handleSubmit}>
      <div>
        <label className="text-sm font-semibold text-ink" htmlFor="email">{labels.emailLabel}</label>
        <input
          autoComplete="email"
          className="mt-1.5 h-11 w-full rounded-lg border border-border bg-white px-3 text-sm text-ink outline-none transition focus:border-blue"
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
        <label className="text-sm font-semibold text-ink" htmlFor="password">{labels.passwordLabel}</label>
        <input
          autoComplete="new-password"
          className="mt-1.5 h-11 w-full rounded-lg border border-border bg-white px-3 text-sm text-ink outline-none transition focus:border-blue"
          id="password"
          minLength={8}
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
        className="inline-flex h-11 w-full items-center justify-center rounded-full bg-ink px-5 text-sm font-semibold text-white transition hover:bg-text disabled:opacity-60"
        disabled={pending}
        type="submit"
      >
        {pending ? labels.submittingLabel : labels.submitLabel}
      </button>
    </form>
  );
}
