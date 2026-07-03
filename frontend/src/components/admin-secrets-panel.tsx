"use client";

import { useCallback, useEffect, useState } from "react";
import { adminFetch } from "@/src/lib/api";
import { Icon } from "@/src/components/icon";

type SecretStatus = {
  name: string;
  configured: boolean;
  source: "panel" | "env" | "vault" | "unset" | string;
  masked: string | null;
};

type Labels = {
  columns: string[];
  sourceLabels: Record<string, string>;
  placeholder: string;
  saveLabel: string;
  savingLabel: string;
  clearLabel: string;
  confirmClear: string;
  loading: string;
  empty: string;
  errors: { unauthorized: string; network: string; saveFailed: string };
  signInLabel: string;
};

type Props = {
  labels: Labels;
  /** 401 durumunda gösterilecek locale'li admin login yolu. */
  loginHref: string;
};

const sourceTone: Record<string, string> = {
  panel: "border-blue/25 bg-blue/10",
  env: "border-border bg-canvas",
  vault: "border-violet/25 bg-violet/10",
  unset: "border-warning/25 bg-warning/10",
};

export function AdminSecretsPanel({ labels, loginHref }: Props) {
  const [rows, setRows] = useState<SecretStatus[] | null>(null);
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [unauthorized, setUnauthorized] = useState(false);

  const applyResponse = useCallback(
    async (response: Response) => {
      if (response.status === 401 || response.status === 403) {
        setUnauthorized(true);
        setRows([]);
        return;
      }
      if (!response.ok) {
        setError(labels.errors.saveFailed);
        return;
      }
      setUnauthorized(false);
      setError(null);
      setRows((await response.json()) as SecretStatus[]);
    },
    [labels.errors.saveFailed],
  );

  const load = useCallback(async () => {
    try {
      await applyResponse(await adminFetch("/admin/secrets"));
    } catch {
      setError(labels.errors.network);
      setRows([]);
    }
  }, [applyResponse, labels.errors.network]);

  useEffect(() => {
    void load();
  }, [load]);

  async function save(name: string) {
    const value = drafts[name]?.trim();
    if (!value) {
      return;
    }
    setBusy(name);
    try {
      await applyResponse(
        await adminFetch(`/admin/secrets/${name}`, {
          method: "PUT",
          body: JSON.stringify({ value }),
        }),
      );
      setDrafts((current) => ({ ...current, [name]: "" }));
    } catch {
      setError(labels.errors.network);
    } finally {
      setBusy(null);
    }
  }

  async function clearOverride(name: string) {
    if (!window.confirm(labels.confirmClear)) {
      return;
    }
    setBusy(name);
    try {
      await applyResponse(await adminFetch(`/admin/secrets/${name}`, { method: "DELETE" }));
    } catch {
      setError(labels.errors.network);
    } finally {
      setBusy(null);
    }
  }

  if (rows === null) {
    return <p className="text-sm text-muted">{labels.loading}</p>;
  }

  if (unauthorized) {
    return (
      <div className="rounded-xl border border-warning/25 bg-warning/10 p-4">
        <p className="text-sm text-ink">{labels.errors.unauthorized}</p>
        <a
          className="mt-3 inline-flex h-9 items-center rounded-full bg-ink px-4 text-xs font-semibold text-white transition hover:bg-text"
          href={loginHref}
        >
          {labels.signInLabel}
        </a>
      </div>
    );
  }

  return (
    <div className="space-y-3">
      {error ? (
        <p className="rounded-lg border border-danger/25 bg-danger/10 px-3 py-2 text-sm text-ink" role="alert">
          {error}
        </p>
      ) : null}

      <div className="overflow-hidden rounded-xl border border-border bg-white">
        <div className="overflow-x-auto">
          <table className="w-full min-w-[760px] text-left text-sm">
            <thead>
              <tr className="border-b border-hairline text-xs uppercase tracking-wide text-faint">
                {labels.columns.map((column) => (
                  <th className="px-4 py-3 font-semibold" key={column}>
                    {column}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <tr className="border-b border-hairline last:border-0" key={row.name}>
                  <td className="px-4 py-3 font-mono text-xs text-ink">{row.name}</td>
                  <td className="px-4 py-3">
                    <span
                      className={`inline-flex rounded-full border px-2.5 py-0.5 text-xs font-semibold text-ink ${sourceTone[row.source] ?? "border-border bg-canvas"}`}
                    >
                      {labels.sourceLabels[row.source] ?? row.source}
                    </span>
                  </td>
                  <td className="px-4 py-3 font-mono text-xs text-muted">
                    {row.masked ?? "—"}
                  </td>
                  <td className="px-4 py-3">
                    <div className="flex flex-wrap items-center gap-2">
                      <input
                        aria-label={`${row.name} value`}
                        className="h-9 w-44 rounded-lg border border-border bg-white px-2.5 font-mono text-xs text-ink outline-none transition focus:border-blue"
                        onChange={(event) =>
                          setDrafts((current) => ({ ...current, [row.name]: event.target.value }))
                        }
                        placeholder={labels.placeholder}
                        type="password"
                        value={drafts[row.name] ?? ""}
                      />
                      <button
                        className="inline-flex h-9 items-center rounded-full bg-ink px-4 text-xs font-semibold text-white transition hover:bg-text disabled:opacity-50"
                        disabled={busy === row.name || !(drafts[row.name] ?? "").trim()}
                        onClick={() => void save(row.name)}
                        type="button"
                      >
                        {busy === row.name ? labels.savingLabel : labels.saveLabel}
                      </button>
                      {row.source === "panel" ? (
                        <button
                          className="inline-flex h-9 items-center gap-1.5 rounded-full border border-border px-3 text-xs font-semibold text-muted transition hover:text-ink disabled:opacity-50"
                          disabled={busy === row.name}
                          onClick={() => void clearOverride(row.name)}
                          type="button"
                        >
                          <Icon className="h-3.5 w-3.5" name="X" />
                          {labels.clearLabel}
                        </button>
                      ) : null}
                    </div>
                  </td>
                </tr>
              ))}
              {rows.length === 0 ? (
                <tr>
                  <td className="px-4 py-6 text-sm text-muted" colSpan={labels.columns.length}>
                    {labels.empty}
                  </td>
                </tr>
              ) : null}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
