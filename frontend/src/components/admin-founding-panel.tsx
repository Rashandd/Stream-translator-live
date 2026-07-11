"use client";

import { useCallback, useEffect, useState } from "react";
import { adminFetch } from "@/src/lib/api";

type Status = "pending" | "approved" | "revoked";

/** `GET /admin/founding/applications` satırı (backend `founding::FoundingApplication`). */
type Application = {
  user_id: string;
  email: string;
  platform: "kick" | "youtube" | "twitch" | "other";
  channel_url: string;
  message: string;
  status: Status;
  created_at_ms: number;
};

type Labels = {
  title: string;
  subtitle: string;
  columns: { broadcaster: string; platform: string; channel: string; status: string };
  statusLabels: Record<Status, string>;
  approveLabel: string;
  revokeLabel: string;
  savingLabel: string;
  loading: string;
  empty: string;
  errors: { unauthorized: string; network: string };
  signInLabel: string;
};

type Props = { labels: Labels; loginHref: string };

function formatDate(ms: number): string {
  return new Date(ms).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

export function AdminFoundingPanel({ labels, loginHref }: Props) {
  const [applications, setApplications] = useState<Application[] | null>(null);
  const [unauthorized, setUnauthorized] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savingId, setSavingId] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const res = await adminFetch("/admin/founding/applications");
      if (res.status === 401 || res.status === 403) {
        setUnauthorized(true);
        setApplications([]);
        return;
      }
      if (!res.ok) {
        setError(labels.errors.network);
        return;
      }
      setUnauthorized(false);
      setApplications((await res.json()) as Application[]);
    } catch {
      setError(labels.errors.network);
      setApplications([]);
    }
  }, [labels.errors.network]);

  useEffect(() => {
    void load();
  }, [load]);

  async function setStatus(userId: string, action: "approve" | "revoke") {
    setSavingId(userId);
    setError(null);
    try {
      const res = await adminFetch(`/admin/founding/${userId}/${action}`, { method: "POST" });
      if (res.status === 401 || res.status === 403) {
        setUnauthorized(true);
        return;
      }
      if (!res.ok) {
        setError(labels.errors.network);
        return;
      }
      const updated = (await res.json()) as Application;
      setApplications((current) => current?.map((a) => (a.user_id === userId ? updated : a)) ?? null);
    } catch {
      setError(labels.errors.network);
    } finally {
      setSavingId(null);
    }
  }

  if (unauthorized) {
    return (
      <div className="rounded-xl border border-warning/25 bg-warning/10 p-4">
        <p className="text-sm text-ink">{labels.errors.unauthorized}</p>
        <a
          className="mt-3 inline-flex h-9 items-center rounded-full bg-primary px-4 text-xs font-semibold text-on-primary transition hover:bg-primary-hover"
          href={loginHref}
        >
          {labels.signInLabel}
        </a>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-lg font-semibold text-ink">{labels.title}</h2>
        <p className="mt-1 text-sm text-muted">{labels.subtitle}</p>
      </div>

      {error ? (
        <p className="rounded-lg border border-danger/25 bg-danger/10 px-3 py-2 text-sm text-ink" role="alert">
          {error}
        </p>
      ) : null}

      <div className="overflow-hidden rounded-xl border border-border bg-surface">
        <div className="overflow-x-auto">
          <table className="w-full min-w-[640px] text-left text-sm">
            <thead>
              <tr className="border-b border-hairline text-xs uppercase tracking-wide text-faint">
                <th className="px-4 py-3 font-semibold">{labels.columns.broadcaster}</th>
                <th className="px-4 py-3 font-semibold">{labels.columns.platform}</th>
                <th className="px-4 py-3 font-semibold">{labels.columns.channel}</th>
                <th className="px-4 py-3 font-semibold">{labels.columns.status}</th>
                <th className="px-4 py-3" />
              </tr>
            </thead>
            <tbody>
              {applications === null ? (
                <tr>
                  <td className="px-4 py-6 text-sm text-muted" colSpan={5}>
                    {labels.loading}
                  </td>
                </tr>
              ) : null}
              {applications?.map((app) => (
                <tr className="border-b border-hairline last:border-0" key={app.user_id}>
                  <td className="px-4 py-3">
                    <p className="font-semibold text-ink">{app.email}</p>
                    <p className="font-mono text-xs text-muted">{formatDate(app.created_at_ms)}</p>
                  </td>
                  <td className="px-4 py-3 text-ink">{app.platform}</td>
                  <td className="max-w-[220px] px-4 py-3">
                    <a
                      className="truncate text-blue underline"
                      href={app.channel_url}
                      rel="noreferrer"
                      target="_blank"
                    >
                      {app.channel_url}
                    </a>
                  </td>
                  <td className="px-4 py-3">
                    <span
                      className={`inline-flex rounded-full border px-2.5 py-0.5 text-xs font-semibold text-ink ${
                        app.status === "approved"
                          ? "border-success/25 bg-success/10"
                          : app.status === "revoked"
                            ? "border-danger/25 bg-danger/10"
                            : "border-warning/25 bg-warning/10"
                      }`}
                    >
                      {labels.statusLabels[app.status]}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-right">
                    <button
                      className="inline-flex h-9 items-center rounded-full border border-blue/40 px-3 text-xs font-semibold text-ink transition hover:bg-blue/10 disabled:opacity-50"
                      disabled={savingId === app.user_id || app.status === "approved"}
                      onClick={() => void setStatus(app.user_id, "approve")}
                      type="button"
                    >
                      {savingId === app.user_id ? labels.savingLabel : labels.approveLabel}
                    </button>
                    <button
                      className="ml-2 inline-flex h-9 items-center rounded-full border border-danger/40 px-3 text-xs font-semibold text-ink transition hover:bg-danger/10 disabled:opacity-50"
                      disabled={savingId === app.user_id || app.status !== "approved"}
                      onClick={() => void setStatus(app.user_id, "revoke")}
                      type="button"
                    >
                      {labels.revokeLabel}
                    </button>
                  </td>
                </tr>
              ))}
              {applications && applications.length === 0 ? (
                <tr>
                  <td className="px-4 py-6 text-sm text-muted" colSpan={5}>
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
