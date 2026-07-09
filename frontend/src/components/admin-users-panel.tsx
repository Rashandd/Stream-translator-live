"use client";

import { useCallback, useEffect, useState } from "react";
import { Icon } from "@/src/components/icon";
import { adminFetch, API_URL } from "@/src/lib/api";

/** `GET /admin/users` satırı (backend `users::AdminUserSummary`). */
type AdminUser = {
  id: string;
  email: string;
  plan: string;
  email_verified: boolean;
  created_at_ms: number;
  oauth_provider: string | null;
};

type Labels = {
  searchPlaceholder: string;
  columns: { broadcaster: string; plan: string; verified: string; joined: string };
  verifiedLabel: string;
  pendingLabel: string;
  saveLabel: string;
  savingLabel: string;
  loading: string;
  empty: string;
  errors: { unauthorized: string; network: string; saveFailed: string };
  signInLabel: string;
};

type Props = { labels: Labels; loginHref: string };

function formatDate(ms: number): string {
  return new Date(ms).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

export function AdminUsersPanel({ labels, loginHref }: Props) {
  const [users, setUsers] = useState<AdminUser[] | null>(null);
  const [plans, setPlans] = useState<string[]>([]);
  const [query, setQuery] = useState("");
  const [unauthorized, setUnauthorized] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pendingPlan, setPendingPlan] = useState<Record<string, string>>({});
  const [savingId, setSavingId] = useState<string | null>(null);

  const load = useCallback(
    async (q: string) => {
      try {
        const path = q.trim() ? `/admin/users?q=${encodeURIComponent(q.trim())}` : "/admin/users";
        const res = await adminFetch(path);
        if (res.status === 401 || res.status === 403) {
          setUnauthorized(true);
          setUsers([]);
          return;
        }
        if (!res.ok) {
          setError(labels.errors.network);
          return;
        }
        setUnauthorized(false);
        setUsers((await res.json()) as AdminUser[]);
      } catch {
        setError(labels.errors.network);
        setUsers([]);
      }
    },
    [labels.errors.network],
  );

  useEffect(() => {
    void load("");
    // Plan seçenekleri public uçtan — admin token gerekmez.
    fetch(`${API_URL}/api/plans`)
      .then((res) => (res.ok ? res.json() : []))
      .then((data: { name: string }[]) => setPlans(data.map((p) => p.name)))
      .catch(() => setPlans([]));
  }, [load]);

  async function savePlan(id: string) {
    const plan = pendingPlan[id];
    if (!plan) return;
    setSavingId(id);
    setError(null);
    try {
      const res = await adminFetch(`/admin/users/${id}/plan`, {
        method: "PUT",
        body: JSON.stringify({ plan }),
      });
      if (res.status === 401 || res.status === 403) {
        setUnauthorized(true);
        return;
      }
      if (!res.ok) {
        setError(labels.errors.saveFailed);
        return;
      }
      const updated = (await res.json()) as AdminUser;
      setUsers((current) => current?.map((u) => (u.id === id ? updated : u)) ?? null);
      setPendingPlan((current) => {
        const next = { ...current };
        delete next[id];
        return next;
      });
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
          className="mt-3 inline-flex h-9 items-center rounded-full bg-ink px-4 text-xs font-semibold text-white transition hover:bg-text"
          href={loginHref}
        >
          {labels.signInLabel}
        </a>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {error ? (
        <p className="rounded-lg border border-danger/25 bg-danger/10 px-3 py-2 text-sm text-ink" role="alert">
          {error}
        </p>
      ) : null}

      <div className="flex items-center gap-2 rounded-lg border border-border bg-white px-3 py-2">
        <Icon className="h-4 w-4 text-muted" name="Settings" />
        <input
          className="min-w-0 flex-1 bg-transparent text-sm text-ink outline-none placeholder:text-faint"
          onChange={(event) => {
            const value = event.target.value;
            setQuery(value);
            void load(value);
          }}
          placeholder={labels.searchPlaceholder}
          type="search"
          value={query}
        />
      </div>

      <div className="overflow-hidden rounded-xl border border-border bg-white">
        <div className="overflow-x-auto">
          <table className="w-full min-w-[640px] text-left text-sm">
            <thead>
              <tr className="border-b border-hairline text-xs uppercase tracking-wide text-faint">
                <th className="px-4 py-3 font-semibold">{labels.columns.broadcaster}</th>
                <th className="px-4 py-3 font-semibold">{labels.columns.plan}</th>
                <th className="px-4 py-3 font-semibold">{labels.columns.verified}</th>
                <th className="px-4 py-3 font-semibold">{labels.columns.joined}</th>
                <th className="px-4 py-3" />
              </tr>
            </thead>
            <tbody>
              {users === null ? (
                <tr>
                  <td className="px-4 py-6 text-sm text-muted" colSpan={5}>
                    {labels.loading}
                  </td>
                </tr>
              ) : null}
              {users?.map((user) => {
                const selected = pendingPlan[user.id] ?? user.plan;
                const dirty = selected !== user.plan;
                return (
                  <tr className="border-b border-hairline last:border-0" key={user.id}>
                    <td className="px-4 py-3 font-semibold text-ink">{user.email}</td>
                    <td className="px-4 py-3">
                      <select
                        className="h-9 rounded-lg border border-border bg-white px-2 text-sm text-ink outline-none transition focus:border-blue"
                        onChange={(event) =>
                          setPendingPlan((current) => ({ ...current, [user.id]: event.target.value }))
                        }
                        value={selected}
                      >
                        {(plans.includes(user.plan) ? plans : [user.plan, ...plans]).map((plan) => (
                          <option key={plan} value={plan}>
                            {plan}
                          </option>
                        ))}
                      </select>
                    </td>
                    <td className="px-4 py-3">
                      <span
                        className={`inline-flex rounded-full border px-2.5 py-0.5 text-xs font-semibold text-ink ${
                          user.email_verified ? "border-success/25 bg-success/10" : "border-border bg-canvas"
                        }`}
                      >
                        {user.email_verified ? labels.verifiedLabel : labels.pendingLabel}
                      </span>
                    </td>
                    <td className="px-4 py-3 font-mono text-xs text-muted">{formatDate(user.created_at_ms)}</td>
                    <td className="px-4 py-3 text-right">
                      <button
                        className="inline-flex h-9 items-center rounded-full border border-blue/40 px-3 text-xs font-semibold text-ink transition hover:bg-blue/10 disabled:opacity-50"
                        disabled={!dirty || savingId === user.id}
                        onClick={() => void savePlan(user.id)}
                        type="button"
                      >
                        {savingId === user.id ? labels.savingLabel : labels.saveLabel}
                      </button>
                    </td>
                  </tr>
                );
              })}
              {users && users.length === 0 ? (
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
