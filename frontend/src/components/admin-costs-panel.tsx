"use client";

import { useCallback, useEffect, useState } from "react";
import { MetricCard } from "@/src/components/dashboard-ui";
import { adminFetch } from "@/src/lib/api";

/** `GET /admin/costs/summary` yanıtı (backend `costs::AdminCostSummary`). */
type CostSummary = {
  period: string;
  user_count: number;
  total_usage_hours: number;
  stt_usd_per_hour: number;
  translate_usd_per_hour: number;
  estimated_cost_cents: number;
  estimated_monthly_revenue_cents: number;
};

type Labels = {
  usageLabel: string;
  costLabel: string;
  costHint: string;
  revenueLabel: string;
  revenueHint: string;
  usersLabel: string;
  usersHint: string;
  loading: string;
  errors: { unauthorized: string; network: string };
  signInLabel: string;
};

type Props = { labels: Labels; loginHref: string };

function formatDollars(cents: number): string {
  return `$${(cents / 100).toFixed(2)}`;
}

export function AdminCostsPanel({ labels, loginHref }: Props) {
  const [summary, setSummary] = useState<CostSummary | null>(null);
  const [unauthorized, setUnauthorized] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const res = await adminFetch("/admin/costs/summary");
      if (res.status === 401 || res.status === 403) {
        setUnauthorized(true);
        return;
      }
      if (!res.ok) {
        setError(labels.errors.network);
        return;
      }
      setUnauthorized(false);
      setSummary((await res.json()) as CostSummary);
    } catch {
      setError(labels.errors.network);
    }
  }, [labels.errors.network]);

  useEffect(() => {
    void load();
  }, [load]);

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
      {error ? (
        <p className="rounded-lg border border-danger/25 bg-danger/10 px-3 py-2 text-sm text-ink" role="alert">
          {error}
        </p>
      ) : null}
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <MetricCard
          hint={summary ? `${summary.period}` : labels.loading}
          icon="Gauge"
          label={labels.usageLabel}
          value={summary ? `${summary.total_usage_hours.toFixed(1)}h` : "—"}
        />
        <MetricCard
          hint={
            summary
              ? `${labels.costHint} · $${summary.stt_usd_per_hour.toFixed(2)}+$${summary.translate_usd_per_hour.toFixed(2)}/h`
              : labels.loading
          }
          icon="CreditCard"
          label={labels.costLabel}
          value={summary ? formatDollars(summary.estimated_cost_cents) : "—"}
        />
        <MetricCard
          hint={labels.revenueHint}
          icon="Route"
          label={labels.revenueLabel}
          value={summary ? formatDollars(summary.estimated_monthly_revenue_cents) : "—"}
        />
        <MetricCard
          hint={labels.usersHint}
          icon="MonitorCog"
          label={labels.usersLabel}
          value={summary ? String(summary.user_count) : "—"}
        />
      </div>
    </div>
  );
}
