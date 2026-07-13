"use client";

import { useEffect, useState } from "react";
import { adminFetch, API_URL } from "@/src/lib/api";

type Labels = {
  title: string;
  apiLabel: string;
  healthy: string;
  unreachable: string;
  sttLabel: string;
  translateLabel: string;
  mockLabel: string;
  mockOn: string;
  mockOff: string;
  plansLabel: string;
  checking: string;
};

type LiveState = {
  apiOk: boolean;
  stt: string;
  translate: string;
  mockFallback: boolean;
  planCount: number | null;
};

/** Genel bakışın canlı durum şeridi. `/health` ve `/api/plans` public;
 *  sağlayıcı/mock durumu ise admin full-config'ten okunur (BH-01: `stt.provider`,
 *  `translate.provider`, `providers.mock_fallback` internal olduğundan public
 *  `/config`'te dönmez). Bu şerit yalnız admin sayfasında render edilir. */
export function AdminLiveStatus({ labels }: { labels: Labels }) {
  const [state, setState] = useState<LiveState | null | "error">(null);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const [health, config, plans] = await Promise.all([
          fetch(`${API_URL}/health`),
          adminFetch("/admin/config"),
          fetch(`${API_URL}/api/plans`),
        ]);
        const cfg = config.ok ? ((await config.json()) as Record<string, string>) : {};
        const planList = plans.ok ? ((await plans.json()) as unknown[]) : null;
        if (cancelled) {
          return;
        }
        setState({
          apiOk: health.ok,
          stt: cfg["stt.provider"] ?? "?",
          translate: cfg["translate.provider"] ?? "?",
          mockFallback: cfg["providers.mock_fallback"] !== "false",
          planCount: planList ? planList.length : null,
        });
      } catch {
        if (!cancelled) {
          setState("error");
        }
      }
    }
    void load();
    return () => {
      cancelled = true;
    };
  }, []);

  const pill = (tone: "ok" | "warn" | "bad", text: string) => (
    <span
      className={`inline-flex rounded-full border px-2.5 py-0.5 text-xs font-semibold text-ink ${
        tone === "ok"
          ? "border-success/25 bg-success/10"
          : tone === "warn"
            ? "border-warning/25 bg-warning/10"
            : "border-danger/25 bg-danger/10"
      }`}
    >
      {text}
    </span>
  );

  return (
    <section className="rounded-xl border border-border bg-surface p-4">
      <h2 className="text-sm font-semibold text-ink">{labels.title}</h2>
      {state === null ? (
        <p className="mt-2 text-sm text-muted">{labels.checking}</p>
      ) : state === "error" ? (
        <p className="mt-2">{pill("bad", `${labels.apiLabel}: ${labels.unreachable}`)}</p>
      ) : (
        <div className="mt-2 flex flex-wrap items-center gap-2 text-sm text-muted">
          {pill(state.apiOk ? "ok" : "bad", `${labels.apiLabel}: ${state.apiOk ? labels.healthy : labels.unreachable}`)}
          <span className="font-mono text-xs">{labels.sttLabel}: {state.stt}</span>
          <span className="font-mono text-xs">{labels.translateLabel}: {state.translate}</span>
          {pill(state.mockFallback ? "warn" : "ok", `${labels.mockLabel}: ${state.mockFallback ? labels.mockOn : labels.mockOff}`)}
          {state.planCount !== null ? (
            <span className="font-mono text-xs">{labels.plansLabel}: {state.planCount}</span>
          ) : null}
        </div>
      )}
    </section>
  );
}
