"use client";

import Link from "next/link";
import { useState } from "react";
import { API_URL } from "@/src/lib/api";

type Labels = {
  emailLabel: string;
  emailPlaceholder: string;
  submitLabel: string;
  submittingLabel: string;
  codeSentNote: string;
  codeLabel: string;
  codePlaceholder: string;
  newPasswordLabel: string;
  newPasswordPlaceholder: string;
  confirmLabel: string;
  doneMessage: string;
  doneLoginLabel: string;
  errorInvalidCode: string;
  errorWeakPassword: string;
  errorNetwork: string;
};

type Props = {
  labels: Labels;
  /** Giriş sayfası (locale'li) — sıfırlama bitince buraya dönülür. */
  loginHref: string;
};

/**
 * İki adımlı sıfırlama: e-posta → kod maillenir → kod + yeni şifre.
 * Backend hesap varlığını sızdırmaz — istek her durumda 200 döner,
 * bu yüzden ilk adım her zaman ikinci adıma geçer.
 */
export function ResetPasswordForm({ labels, loginHref }: Props) {
  const [step, setStep] = useState<"request" | "confirm" | "done">("request");
  const [email, setEmail] = useState("");
  const [code, setCode] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleRequest(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setPending(true);
    setError(null);
    try {
      await fetch(`${API_URL}/api/auth/reset/request`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ email }),
      });
      setStep("confirm");
    } catch {
      setError(labels.errorNetwork);
    }
    setPending(false);
  }

  async function handleConfirm(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setPending(true);
    setError(null);
    try {
      const response = await fetch(`${API_URL}/api/auth/reset/confirm`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ email, code, new_password: newPassword }),
      });
      if (response.ok) {
        setStep("done");
        setPending(false);
        return;
      }
      setError(response.status === 422 ? labels.errorWeakPassword : labels.errorInvalidCode);
    } catch {
      setError(labels.errorNetwork);
    }
    setPending(false);
  }

  if (step === "done") {
    return (
      <div className="mt-6 space-y-4">
        <p className="rounded-lg border border-success/30 bg-success/10 px-3 py-2 text-sm text-ink">
          {labels.doneMessage}
        </p>
        <Link
          className="inline-flex h-11 w-full items-center justify-center rounded-full bg-primary px-5 text-sm font-semibold text-on-primary transition hover:bg-primary-hover"
          href={loginHref}
        >
          {labels.doneLoginLabel}
        </Link>
      </div>
    );
  }

  if (step === "confirm") {
    return (
      <form className="mt-6 space-y-4" onSubmit={handleConfirm}>
        <p className="rounded-lg border border-blue/25 bg-blue/10 px-3 py-2 text-sm text-ink">
          {labels.codeSentNote}
        </p>
        <div>
          <label className="text-sm font-semibold text-ink" htmlFor="code">{labels.codeLabel}</label>
          <input
            autoComplete="one-time-code"
            className="mt-1.5 h-12 w-full rounded-lg border border-border bg-surface px-3 text-center font-mono text-lg tracking-[0.4em] text-ink outline-none transition focus:border-blue"
            id="code"
            inputMode="numeric"
            maxLength={6}
            onChange={(event) => setCode(event.target.value.replace(/[^0-9]/g, ""))}
            pattern="[0-9]*"
            placeholder={labels.codePlaceholder}
            required
            type="text"
            value={code}
          />
        </div>
        <div>
          <label className="text-sm font-semibold text-ink" htmlFor="new-password">{labels.newPasswordLabel}</label>
          <input
            autoComplete="new-password"
            className="mt-1.5 h-11 w-full rounded-lg border border-border bg-surface px-3 text-sm text-ink outline-none transition focus:border-blue"
            id="new-password"
            minLength={8}
            onChange={(event) => setNewPassword(event.target.value)}
            placeholder={labels.newPasswordPlaceholder}
            required
            type="password"
            value={newPassword}
          />
        </div>
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
          {pending ? labels.submittingLabel : labels.confirmLabel}
        </button>
      </form>
    );
  }

  return (
    <form className="mt-6 space-y-4" onSubmit={handleRequest}>
      <div>
        <label className="text-sm font-semibold text-ink" htmlFor="email">{labels.emailLabel}</label>
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
