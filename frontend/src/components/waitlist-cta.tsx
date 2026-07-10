"use client";

import { useState, type FormEvent } from "react";
import { Icon } from "@/src/components/icon";
import { API_URL } from "@/src/lib/api";

type Labels = {
  label: string;
  emailPlaceholder: string;
  submitLabel: string;
  loadingLabel: string;
  successMessage: string;
  duplicateMessage: string;
  invalidEmailMessage: string;
  errorMessage: string;
};

type Props = {
  /** Which coming-soon/waitlist feature this signup is for — stored as-is. */
  interest: string;
  labels: Labels;
};

type Status = "idle" | "loading" | "success" | "duplicate" | "invalid" | "error";

/** Maps a `POST /api/waitlist` response status to UI state (exported for tests). */
export function statusFromResponse(httpStatus: number): Status {
  if (httpStatus === 201) return "success";
  if (httpStatus === 409) return "duplicate";
  if (httpStatus === 400) return "invalid";
  return "error";
}

export function WaitlistCta({ interest, labels }: Props) {
  const [email, setEmail] = useState("");
  const [status, setStatus] = useState<Status>("idle");

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (status === "loading" || status === "success" || status === "duplicate") return;
    setStatus("loading");
    try {
      const res = await fetch(`${API_URL}/api/waitlist`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ email, interest }),
      });
      setStatus(statusFromResponse(res.status));
    } catch {
      setStatus("error");
    }
  }

  if (status === "success" || status === "duplicate") {
    return (
      <p className="mt-4 flex items-center gap-1.5 text-xs font-medium text-success">
        <Icon className="h-3.5 w-3.5" name="CheckCircle2" />
        {status === "success" ? labels.successMessage : labels.duplicateMessage}
      </p>
    );
  }

  return (
    <form className="mt-4" onSubmit={(event) => void submit(event)}>
      <p className="mb-2 text-xs font-medium text-muted">{labels.label}</p>
      <div className="flex flex-col gap-2 sm:flex-row">
        <input
          className="h-11 min-w-0 flex-1 rounded-lg border border-border bg-surface px-3 text-sm text-ink outline-none transition focus:border-blue"
          onChange={(event) => setEmail(event.target.value)}
          placeholder={labels.emailPlaceholder}
          required
          type="email"
          value={email}
        />
        <button
          className="inline-flex h-11 shrink-0 items-center justify-center gap-1.5 rounded-full bg-primary px-4 text-sm font-semibold text-white transition hover:bg-primary-hover disabled:opacity-60"
          disabled={status === "loading"}
          type="submit"
        >
          {status === "loading" ? labels.loadingLabel : labels.submitLabel}
        </button>
      </div>
      {status === "invalid" || status === "error" ? (
        <p className="mt-2 text-xs font-medium text-danger">
          {status === "invalid" ? labels.invalidEmailMessage : labels.errorMessage}
        </p>
      ) : null}
    </form>
  );
}
