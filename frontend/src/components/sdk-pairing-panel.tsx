"use client";

import { useCallback, useState } from "react";
import { Icon } from "@/src/components/icon";
import { userFetch } from "@/src/lib/api";

type Labels = {
  placeholder: string;
  generateLabel: string;
  generatingLabel: string;
  expiresInTemplate: string;
  errors: {
    unauthorized: string;
    network: string;
    failed: string;
  };
};

/** `POST /api/pairing/code` yanıtı (backend pairing::PairingCodeResponse). */
type PairingCodeResponse = {
  code: string;
  expires_in_secs: number;
};

function fill(template: string, values: Record<string, string>): string {
  return Object.entries(values).reduce(
    (acc, [key, value]) => acc.replaceAll(`{${key}}`, value),
    template,
  );
}

export function SdkPairingPanel({ labels }: { labels: Labels }) {
  const [code, setCode] = useState<string | null>(null);
  const [expiresInSecs, setExpiresInSecs] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const generate = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      const res = await userFetch("/api/pairing/code", { method: "POST" });
      if (res.status === 401) {
        setError(labels.errors.unauthorized);
        return;
      }
      if (!res.ok) {
        setError(labels.errors.failed);
        return;
      }
      const body = (await res.json()) as PairingCodeResponse;
      setCode(body.code);
      setExpiresInSecs(body.expires_in_secs);
    } catch {
      setError(labels.errors.network);
    } finally {
      setBusy(false);
    }
  }, [labels.errors]);

  const minutes = expiresInSecs ? Math.round(expiresInSecs / 60) : null;

  return (
    <>
      <div className="mt-1 rounded-lg border border-border bg-canvas px-4 py-5 text-center">
        <code className="font-mono text-3xl font-semibold tracking-[0.3em] text-ink">
          {code ?? labels.placeholder}
        </code>
      </div>
      {error ? (
        <p className="mt-2 rounded-lg border border-danger/25 bg-danger/10 px-3 py-2 text-sm text-ink" role="alert">
          {error}
        </p>
      ) : null}
      {minutes !== null ? (
        <p className="mt-2 text-xs text-muted">{fill(labels.expiresInTemplate, { minutes: String(minutes) })}</p>
      ) : null}
      <button
        className="mt-4 inline-flex h-11 w-full items-center justify-center gap-2 rounded-full border border-border bg-surface px-5 text-sm font-semibold text-ink transition hover:border-cyan hover:bg-cyan/10 disabled:opacity-50"
        disabled={busy}
        onClick={() => void generate()}
        type="button"
      >
        <Icon className="h-4 w-4" name="KeyRound" />
        {busy ? labels.generatingLabel : labels.generateLabel}
      </button>
    </>
  );
}
