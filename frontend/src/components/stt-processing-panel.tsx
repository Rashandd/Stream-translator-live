"use client";

import { useCallback, useEffect, useState } from "react";
import { Icon } from "@/src/components/icon";
import { userFetch } from "@/src/lib/api";

/** GET /api/stt/policy yanıtı (backend/src/stt_policy sözleşmesi). */
type SttPolicy = {
  local_allowed: boolean;
  usage_multiplier: number;
  requirements: { min_vram_mb: number; benchmark_rtf_max: number };
  watchdog: { p95_ms: number; retry_minutes: number };
  max_chars: number;
  mode: "cloud" | "local";
  benchmark?: {
    passed: boolean;
    rtf: number;
    p95_ms: number;
    gpu_name?: string;
    vram_mb?: number;
    reported_at: number;
  };
};

type ModeCopy = {
  name: string;
  badge: string;
  summary: string;
  icon: string;
};

type Labels = {
  title: string;
  subtitle: string;
  loading: string;
  selectedLabel: string;
  chooseLabel: string;
  savingLabel: string;
  cloud: ModeCopy & { quotaNote: string };
  local: ModeCopy & {
    quotaNoteTemplate: string;
    requirementsTitle: string;
    requirements: string[];
    planLocked: string;
    testButton: string;
    testQueuedNote: string;
    checkResultButton: string;
    noBenchmarkNote: string;
    benchmarkPassed: string;
    benchmarkFailed: string;
    sdkHint: string;
  };
  fallbackNote: string;
  errors: {
    unauthorized: string;
    network: string;
    saveFailed: string;
    benchmarkRequired: string;
  };
};

/** "{multiplier}" / "{vram}" yer tutucularını doldurur. */
function fill(template: string, values: Record<string, string>): string {
  return Object.entries(values).reduce(
    (acc, [key, value]) => acc.replaceAll(`{${key}}`, value),
    template,
  );
}

export function SttProcessingPanel({ labels }: { labels: Labels }) {
  const [policy, setPolicy] = useState<SttPolicy | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [testQueued, setTestQueued] = useState(false);

  const loadPolicy = useCallback(async () => {
    setError(null);
    try {
      const res = await userFetch("/api/stt/policy");
      if (res.status === 401) {
        setError(labels.errors.unauthorized);
        return;
      }
      if (!res.ok) {
        setError(labels.errors.network);
        return;
      }
      setPolicy((await res.json()) as SttPolicy);
    } catch {
      setError(labels.errors.network);
    }
  }, [labels.errors.network, labels.errors.unauthorized]);

  useEffect(() => {
    void loadPolicy();
  }, [loadPolicy]);

  const setMode = useCallback(
    async (mode: "cloud" | "local") => {
      setSaving(true);
      setError(null);
      try {
        const res = await userFetch("/api/stt/mode", {
          method: "PUT",
          body: JSON.stringify({ mode }),
        });
        if (res.status === 401) {
          setError(labels.errors.unauthorized);
        } else if (res.status === 409) {
          setError(labels.errors.benchmarkRequired);
        } else if (!res.ok) {
          setError(labels.errors.saveFailed);
        } else {
          setPolicy((prev) => (prev ? { ...prev, mode } : prev));
        }
      } catch {
        setError(labels.errors.network);
      } finally {
        setSaving(false);
      }
    },
    [labels.errors],
  );

  const queueBenchmark = useCallback(async () => {
    setError(null);
    try {
      const res = await userFetch("/api/sdk/commands", {
        method: "POST",
        body: JSON.stringify({ command: "benchmark" }),
      });
      if (res.status === 401) {
        setError(labels.errors.unauthorized);
      } else if (res.ok) {
        setTestQueued(true);
      } else {
        setError(labels.errors.network);
      }
    } catch {
      setError(labels.errors.network);
    }
  }, [labels.errors.network, labels.errors.unauthorized]);

  if (!policy && !error) {
    return (
      <section className="rounded-xl border border-border bg-white p-5">
        <h2 className="text-lg font-semibold text-ink">{labels.title}</h2>
        <p className="mt-2 text-sm text-muted">{labels.loading}</p>
      </section>
    );
  }

  const benchmark = policy?.benchmark ?? null;
  const localUnlocked = Boolean(policy?.local_allowed && benchmark?.passed);
  const vramGb = policy ? Math.round(policy.requirements.min_vram_mb / 1024) : 6;
  const multiplier = policy?.usage_multiplier ?? 1;

  return (
    <section className="space-y-4">
      <div>
        <h2 className="text-lg font-semibold text-ink">{labels.title}</h2>
        <p className="mt-1 text-sm leading-6 text-muted">{labels.subtitle}</p>
      </div>

      {error ? (
        <p className="rounded-lg border border-danger/25 bg-danger/10 px-4 py-3 text-sm text-ink" role="alert">
          {error}
        </p>
      ) : null}

      <div className="grid gap-4 lg:grid-cols-2">
        {/* Bulut modu */}
        <div className="flex flex-col rounded-xl border border-border bg-white p-5">
          <div className="flex items-center gap-2">
            <Icon className="h-5 w-5 text-blue" name={labels.cloud.icon} />
            <h3 className="text-base font-semibold text-ink">{labels.cloud.name}</h3>
            <span className="rounded-full border border-hairline bg-canvas px-2 py-0.5 text-xs font-semibold text-muted">
              {labels.cloud.badge}
            </span>
          </div>
          <p className="mt-3 text-sm leading-6 text-muted">{labels.cloud.summary}</p>
          <p className="mt-2 text-sm font-medium text-ink">{labels.cloud.quotaNote}</p>
          <div className="mt-auto pt-4">
            {policy?.mode === "cloud" ? (
              <span className="inline-flex items-center gap-1.5 rounded-full bg-success/10 px-3 py-1 text-sm font-semibold text-ink">
                <Icon className="h-4 w-4 text-success" name="CheckCircle2" />
                {labels.selectedLabel}
              </span>
            ) : (
              <button
                className="inline-flex h-9 items-center rounded-full border border-border px-4 text-sm font-semibold text-ink transition hover:bg-canvas disabled:opacity-50"
                disabled={saving}
                onClick={() => void setMode("cloud")}
                type="button"
              >
                {saving ? labels.savingLabel : labels.chooseLabel}
              </button>
            )}
          </div>
        </div>

        {/* Cihazında (yerel) modu */}
        <div className="flex flex-col rounded-xl border border-border bg-white p-5">
          <div className="flex items-center gap-2">
            <Icon className="h-5 w-5 text-blue" name={labels.local.icon} />
            <h3 className="text-base font-semibold text-ink">{labels.local.name}</h3>
            <span className="rounded-full border border-warning/25 bg-warning/10 px-2 py-0.5 text-xs font-semibold text-ink">
              {labels.local.badge}
            </span>
          </div>
          <p className="mt-3 text-sm leading-6 text-muted">{labels.local.summary}</p>
          <p className="mt-2 text-sm font-medium text-ink">
            {fill(labels.local.quotaNoteTemplate, { multiplier: String(multiplier) })}
          </p>

          <div className="mt-4">
            <p className="text-xs font-semibold uppercase tracking-wide text-muted">
              {labels.local.requirementsTitle}
            </p>
            <ul className="mt-2 space-y-1.5">
              {labels.local.requirements.map((req) => (
                <li className="flex items-start gap-2 text-sm leading-6 text-muted" key={req}>
                  <Icon className="mt-1 h-3.5 w-3.5 shrink-0 text-muted" name="CheckCircle2" />
                  {fill(req, { vram: String(vramGb) })}
                </li>
              ))}
            </ul>
          </div>

          {/* Donanım testi durumu */}
          <div className="mt-4 rounded-lg border border-hairline bg-canvas px-4 py-3">
            {!policy?.local_allowed ? (
              <p className="text-sm text-muted">{labels.local.planLocked}</p>
            ) : benchmark ? (
              <div className="flex flex-wrap items-center gap-2">
                <span
                  className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-semibold ${benchmark.passed ? "bg-success/10 text-ink" : "bg-danger/10 text-ink"}`}
                >
                  <span className={`h-1.5 w-1.5 rounded-full ${benchmark.passed ? "bg-success" : "bg-danger"}`} />
                  {benchmark.passed ? labels.local.benchmarkPassed : labels.local.benchmarkFailed}
                </span>
                <span className="font-mono text-xs text-muted">
                  {benchmark.gpu_name ? `${benchmark.gpu_name} · ` : ""}
                  rtf {benchmark.rtf.toFixed(2)} · p95 {benchmark.p95_ms}ms
                </span>
              </div>
            ) : testQueued ? (
              <p className="text-sm text-muted">{labels.local.testQueuedNote}</p>
            ) : (
              <p className="text-sm text-muted">{labels.local.noBenchmarkNote}</p>
            )}
            {policy?.local_allowed ? (
              <div className="mt-3 flex flex-wrap items-center gap-2">
                <button
                  className="inline-flex h-8 items-center gap-1.5 rounded-full border border-border px-3 text-xs font-semibold text-ink transition hover:bg-white"
                  onClick={() => void queueBenchmark()}
                  type="button"
                >
                  <Icon className="h-3.5 w-3.5" name="Gauge" />
                  {labels.local.testButton}
                </button>
                {testQueued ? (
                  <button
                    className="inline-flex h-8 items-center gap-1.5 rounded-full border border-border px-3 text-xs font-semibold text-ink transition hover:bg-white"
                    onClick={() => void loadPolicy()}
                    type="button"
                  >
                    <Icon className="h-3.5 w-3.5" name="RefreshCw" />
                    {labels.local.checkResultButton}
                  </button>
                ) : null}
                <span className="text-xs text-muted">{labels.local.sdkHint}</span>
              </div>
            ) : null}
          </div>

          <div className="mt-auto pt-4">
            {policy?.mode === "local" ? (
              <span className="inline-flex items-center gap-1.5 rounded-full bg-success/10 px-3 py-1 text-sm font-semibold text-ink">
                <Icon className="h-4 w-4 text-success" name="CheckCircle2" />
                {labels.selectedLabel}
              </span>
            ) : (
              <button
                className="inline-flex h-9 items-center rounded-full bg-ink px-4 text-sm font-semibold text-white transition hover:bg-text disabled:cursor-not-allowed disabled:opacity-40"
                disabled={saving || !localUnlocked}
                onClick={() => void setMode("local")}
                type="button"
              >
                {saving ? labels.savingLabel : labels.chooseLabel}
              </button>
            )}
          </div>
        </div>
      </div>

      {/* Zorunlu fallback dipnotu (docs/local-stt-plan.md §1) */}
      <p className="text-xs leading-5 text-muted">{labels.fallbackNote}</p>
    </section>
  );
}
