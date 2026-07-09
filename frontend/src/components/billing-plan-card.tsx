"use client";

import { useCallback, useEffect, useState } from "react";
import { Icon } from "@/src/components/icon";
import { userFetch } from "@/src/lib/api";
import { formatPriceCents, type LivePlan } from "@/src/lib/plans";

/** `GET /api/usage/summary` yanıtı (backend `usage::UsageSummaryResponse`). */
type UsageSummary = {
  plan: string;
  price_cents: number;
  period: string;
  used_seconds: number;
  monthly_hours: number;
  remaining_seconds: number;
  limit_reached: boolean;
};

type DisplayPlan = { name: string; price: string; features: string[] };

type Labels = {
  usageLabel: string;
  upgradeLabel: string;
  upgradingLabel: string;
  topPlanLabel: string;
  checkoutUnavailable: string;
  errorNetwork: string;
};

type Props = {
  /** Sözlükteki `pricing.plans` — canlı fiyat/limitlerle birleştirilmiş
   * (bkz. `mergeLivePlans`), plan adına göre eşleştirilip gösterilir. */
  displayPlans: DisplayPlan[];
  /** Ham katalog — fiyata göre sıralayıp "bir üstteki plan"ı bulmak için. */
  livePlans: LivePlan[] | null;
  /** Yayıncı hesabı hiç canlı veri dönmezse gösterilecek statik yedek. */
  fallback: DisplayPlan & { usage: string };
  labels: Labels;
};

function formatUsage(usage: UsageSummary): string {
  const usedHours = Math.floor(usage.used_seconds / 3600);
  if (usage.monthly_hours < 0) {
    return `${usedHours}h of unlimited`;
  }
  return `${usedHours}h of ${usage.monthly_hours}h`;
}

export function BillingPlanCard({ displayPlans, livePlans, fallback, labels }: Props) {
  const [usage, setUsage] = useState<UsageSummary | null | "error">(null);
  const [checkoutState, setCheckoutState] = useState<"idle" | "pending" | "unavailable" | "error">(
    "idle",
  );

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const res = await userFetch("/api/usage/summary");
        if (!res.ok) {
          if (!cancelled) setUsage("error");
          return;
        }
        const data = (await res.json()) as UsageSummary;
        if (!cancelled) setUsage(data);
      } catch {
        if (!cancelled) setUsage("error");
      }
    }
    void load();
    return () => {
      cancelled = true;
    };
  }, []);

  const current =
    usage && usage !== "error"
      ? (displayPlans.find((p) => p.name.toLowerCase() === usage.plan.toLowerCase()) ?? {
          name: usage.plan,
          price: formatPriceCents(usage.price_cents),
          features: fallback.features,
        })
      : fallback;
  const usageText = usage && usage !== "error" ? formatUsage(usage) : fallback.usage;

  const nextPlan =
    usage && usage !== "error" && livePlans
      ? [...livePlans]
          .sort((a, b) => a.price_cents - b.price_cents)
          .find((p) => p.price_cents > usage.price_cents)
      : undefined;

  const upgrade = useCallback(async () => {
    if (!nextPlan) return;
    setCheckoutState("pending");
    try {
      const res = await userFetch("/api/billing/checkout", {
        method: "POST",
        body: JSON.stringify({ plan: nextPlan.name }),
      });
      if (res.status === 404 || res.status === 503) {
        setCheckoutState("unavailable");
        return;
      }
      if (!res.ok) {
        setCheckoutState("error");
        return;
      }
      const data = (await res.json()) as { url: string };
      window.location.assign(data.url);
    } catch {
      setCheckoutState("error");
    }
  }, [nextPlan]);

  return (
    <section className="rounded-xl border border-border bg-white p-6">
      <div className="flex items-baseline justify-between">
        <h2 className="text-lg font-semibold text-ink">{current.name}</h2>
        <p className="font-mono text-lg font-semibold text-ink">{current.price}</p>
      </div>
      <p className="mt-3 text-xs font-medium text-muted">{labels.usageLabel}</p>
      <p className="font-mono text-sm font-semibold text-ink">{usageText}</p>
      <ul className="mt-4 space-y-2">
        {current.features.map((feature) => (
          <li className="flex items-center gap-2 text-sm text-text" key={feature}>
            <Icon className="h-4 w-4 shrink-0 text-success" name="CheckCircle2" />
            {feature}
          </li>
        ))}
      </ul>
      {usage && usage !== "error" && !nextPlan ? (
        <p className="mt-6 text-center text-xs font-medium text-muted">{labels.topPlanLabel}</p>
      ) : (
        <button
          className="mt-6 inline-flex h-11 w-full items-center justify-center rounded-full bg-ink px-5 text-sm font-semibold text-white transition hover:bg-text disabled:opacity-60"
          disabled={checkoutState === "pending" || !nextPlan}
          onClick={() => void upgrade()}
          type="button"
        >
          {checkoutState === "pending" ? labels.upgradingLabel : labels.upgradeLabel}
        </button>
      )}
      {checkoutState === "unavailable" ? (
        <p className="mt-2 text-xs text-danger">{labels.checkoutUnavailable}</p>
      ) : null}
      {checkoutState === "error" ? <p className="mt-2 text-xs text-danger">{labels.errorNetwork}</p> : null}
    </section>
  );
}
