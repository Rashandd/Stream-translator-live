"use client";

import { useCallback, useEffect, useState } from "react";
import { adminFetch } from "@/src/lib/api";
import { Icon } from "@/src/components/icon";
import { SectionCard } from "@/src/components/dashboard-ui";
import { parsePlanCsv, serializePlanCsv } from "@/src/lib/distribution";

type Tone = "ok" | "warn" | "bad";

type SetupCheck = {
  id: string;
  label: string;
  tone: Tone;
  detail: string;
};

type ProviderProfile = {
  name: string;
  kind: "api_key" | "oauth";
  secret_names: string[];
  configured: boolean;
  last_tested_at_ms: number | null;
  last_test_ok: boolean | null;
  last_test_detail: string | null;
  connect_path: string | null;
  callback_url: string | null;
};

type SetupStatus = {
  checks: SetupCheck[];
  providers: ProviderProfile[];
};

type PlanDefinition = {
  name: string;
  price_cents: number;
  monthly_hours: number;
  max_languages: number;
  max_platforms: number;
  has_watermark: boolean;
  has_voice_clone: boolean;
};

type Labels = {
  checksTitle: string;
  providersTitle: string;
  providersColumns: string[];
  kindLabels: Record<string, string>;
  connectLabel: string;
  testLabel: string;
  testingLabel: string;
  neverTested: string;
  configuredLabel: string;
  notConfiguredLabel: string;
  stripeTitle: string;
  stripeSubtitle: string;
  stripeFreeNote: string;
  planAccessTitle: string;
  planAccessSubtitle: string;
  planAccessColumns: string[];
  noPlanAccessProviders: string;
  planAccessHint: string;
  saveLabel: string;
  savingLabel: string;
  savedLabel: string;
  placeholder: string;
  loading: string;
  empty: string;
  errors: { unauthorized: string; network: string; saveFailed: string };
  signInLabel: string;
};

type Props = {
  labels: Labels;
  loginHref: string;
};

const toneClass: Record<Tone, string> = {
  ok: "border-success/25 bg-success/10",
  warn: "border-warning/25 bg-warning/10",
  bad: "border-danger/25 bg-danger/10",
};

function Pill({ tone, text }: { tone: Tone; text: string }) {
  return (
    <span
      className={`inline-flex rounded-full border px-2.5 py-0.5 text-xs font-semibold text-ink ${toneClass[tone]}`}
    >
      {text}
    </span>
  );
}

export function AdminProductionSetupPanel({ labels, loginHref }: Props) {
  const [status, setStatus] = useState<SetupStatus | null>(null);
  const [plans, setPlans] = useState<PlanDefinition[] | null>(null);
  const [runtimeConfig, setRuntimeConfig] = useState<Record<string, string>>({});
  const [planDrafts, setPlanDrafts] = useState<Record<string, Set<string>>>({});
  const [planBusy, setPlanBusy] = useState<string | null>(null);
  const [planSaved, setPlanSaved] = useState<string | null>(null);
  const [priceDrafts, setPriceDrafts] = useState<Record<string, string>>({});
  const [priceBusy, setPriceBusy] = useState<string | null>(null);
  const [priceSaved, setPriceSaved] = useState<string | null>(null);
  const [connectBusy, setConnectBusy] = useState<string | null>(null);
  const [testing, setTesting] = useState<string | null>(null);
  const [testResults, setTestResults] = useState<Record<string, { ok: boolean; detail: string }>>({});
  const [unauthorized, setUnauthorized] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const [statusResp, plansResp] = await Promise.all([
        adminFetch("/admin/setup/status"),
        adminFetch("/api/plans"),
      ]);
      const configResp = await adminFetch("/config");
      if (statusResp.status === 401 || statusResp.status === 403) {
        setUnauthorized(true);
        setStatus({ checks: [], providers: [] });
        return;
      }
      if (!statusResp.ok) {
        setError(labels.errors.saveFailed);
        return;
      }
      setUnauthorized(false);
      setError(null);
      const loadedStatus = (await statusResp.json()) as SetupStatus;
      const loadedPlans = plansResp.ok ? ((await plansResp.json()) as PlanDefinition[]) : [];
      const loadedConfig = configResp.ok ? ((await configResp.json()) as Record<string, string>) : {};
      setStatus(loadedStatus);
      setPlans(loadedPlans);
      setRuntimeConfig(loadedConfig);
      setPlanDrafts((current) => {
        const next = { ...current };
        for (const provider of loadedStatus.providers.filter((item) => item.kind === "oauth")) {
          if (!next[provider.name]) {
            next[provider.name] = new Set(
              parsePlanCsv(loadedConfig[`integration.${provider.name}.plans`] ?? ""),
            );
          }
        }
        return next;
      });
    } catch {
      setError(labels.errors.network);
      setStatus({ checks: [], providers: [] });
    }
  }, [labels.errors.network, labels.errors.saveFailed]);

  useEffect(() => {
    void load();
  }, [load]);

  async function runTest(name: string) {
    setTesting(name);
    try {
      const response = await adminFetch(`/admin/providers/${name}/test`, { method: "POST" });
      if (response.status === 401 || response.status === 403) {
        setUnauthorized(true);
        return;
      }
      if (response.ok) {
        const outcome = (await response.json()) as { ok: boolean; detail: string };
        setTestResults((current) => ({ ...current, [name]: outcome }));
      } else {
        const detail = await response.text();
        setTestResults((current) => ({ ...current, [name]: { ok: false, detail } }));
      }
    } catch {
      setError(labels.errors.network);
    } finally {
      setTesting(null);
    }
  }

  async function connect(name: string, connectPath: string) {
    setConnectBusy(name);
    try {
      const response = await adminFetch(connectPath);
      if (response.status === 401 || response.status === 403) {
        setUnauthorized(true);
        return;
      }
      if (!response.ok) {
        const detail = await response.text();
        setError(detail || labels.errors.saveFailed);
        return;
      }
      const data = (await response.json()) as { authorize_url: string };
      window.location.href = data.authorize_url;
    } catch {
      setError(labels.errors.network);
    } finally {
      setConnectBusy(null);
    }
  }

  async function savePrice(planName: string) {
    const value = priceDrafts[planName]?.trim();
    if (!value) {
      return;
    }
    setPriceBusy(planName);
    setPriceSaved(null);
    try {
      const response = await adminFetch("/admin/config", {
        method: "PUT",
        body: JSON.stringify({ key: `billing.plan.${planName}.stripe_price_id`, value }),
      });
      if (response.status === 401 || response.status === 403) {
        setUnauthorized(true);
        return;
      }
      if (!response.ok) {
        setError(labels.errors.saveFailed);
        return;
      }
      setPriceSaved(planName);
      await load();
    } catch {
      setError(labels.errors.network);
    } finally {
      setPriceBusy(null);
    }
  }

  function toggleProviderPlan(provider: string, plan: string) {
    setPlanSaved(null);
    setPlanDrafts((current) => {
      const next = new Set(current[provider] ?? []);
      if (next.has(plan)) {
        next.delete(plan);
      } else {
        next.add(plan);
      }
      return { ...current, [provider]: next };
    });
  }

  async function saveProviderPlans(provider: string) {
    const selected = planDrafts[provider] ?? new Set<string>();
    setPlanBusy(provider);
    setPlanSaved(null);
    try {
      const response = await adminFetch("/admin/config", {
        method: "PUT",
        body: JSON.stringify({
          key: `integration.${provider}.plans`,
          value: serializePlanCsv(selected),
        }),
      });
      if (response.status === 401 || response.status === 403) {
        setUnauthorized(true);
        return;
      }
      if (!response.ok) {
        setError(labels.errors.saveFailed);
        return;
      }
      const updated = (await response.json()) as Record<string, string>;
      setRuntimeConfig(updated);
      setPlanSaved(provider);
      await load();
    } catch {
      setError(labels.errors.network);
    } finally {
      setPlanBusy(null);
    }
  }

  if (status === null) {
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
    <div className="space-y-6">
      {error ? (
        <p className="rounded-lg border border-danger/25 bg-danger/10 px-3 py-2 text-sm text-ink" role="alert">
          {error}
        </p>
      ) : null}

      <SectionCard title={labels.checksTitle}>
        {status.checks.length === 0 ? (
          <p className="text-sm text-muted">{labels.empty}</p>
        ) : (
          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {status.checks.map((check) => (
              <div className="rounded-lg border border-hairline p-3" key={check.id}>
                <div className="flex items-center justify-between gap-2">
                  <p className="text-sm font-semibold text-ink">{check.label}</p>
                  <Pill text={check.tone} tone={check.tone} />
                </div>
                <p className="mt-1.5 text-xs leading-5 text-muted">{check.detail}</p>
              </div>
            ))}
          </div>
        )}
      </SectionCard>

      <SectionCard title={labels.providersTitle}>
        <div className="overflow-hidden rounded-xl border border-border">
          <div className="overflow-x-auto">
            <table className="w-full min-w-[760px] text-left text-sm">
              <thead>
                <tr className="border-b border-hairline text-xs uppercase tracking-wide text-faint">
                  {labels.providersColumns.map((column) => (
                    <th className="px-4 py-3 font-semibold" key={column}>
                      {column}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {status.providers.map((provider) => (
                  <tr className="border-b border-hairline last:border-0" key={provider.name}>
                    <td className="px-4 py-3 font-mono text-xs text-ink">{provider.name}</td>
                    <td className="px-4 py-3 text-xs text-muted">
                      {labels.kindLabels[provider.kind] ?? provider.kind}
                    </td>
                    <td className="px-4 py-3 font-mono text-xs text-muted">
                      {provider.secret_names.join(", ")}
                    </td>
                    <td className="px-4 py-3">
                      <Pill
                        text={provider.configured ? labels.configuredLabel : labels.notConfiguredLabel}
                        tone={provider.configured ? "ok" : "warn"}
                      />
                    </td>
                    <td className="px-4 py-3 text-xs text-muted">
                      {provider.last_tested_at_ms ? (
                        <span
                          className={provider.last_test_ok ? "text-success" : "text-danger"}
                          title={provider.last_test_detail ?? ""}
                        >
                          {new Date(provider.last_tested_at_ms).toLocaleString()}
                        </span>
                      ) : (
                        labels.neverTested
                      )}
                    </td>
                    <td className="px-4 py-3">
                      <div className="flex flex-wrap items-center gap-2">
                        {provider.kind === "api_key" && provider.configured ? (
                          <button
                            className="inline-flex h-9 items-center gap-1.5 rounded-full border border-blue/40 px-3 text-xs font-semibold text-ink transition hover:bg-blue/10 disabled:opacity-50"
                            disabled={testing === provider.name}
                            onClick={() => void runTest(provider.name)}
                            type="button"
                          >
                            <Icon className="h-3.5 w-3.5" name="Wifi" />
                            {testing === provider.name ? labels.testingLabel : labels.testLabel}
                          </button>
                        ) : null}
                        {provider.kind === "oauth" && provider.configured && provider.connect_path ? (
                          <button
                            className="inline-flex h-9 items-center gap-1.5 rounded-full border border-blue/40 px-3 text-xs font-semibold text-ink transition hover:bg-blue/10 disabled:opacity-50"
                            disabled={connectBusy === provider.name}
                            onClick={() => void connect(provider.name, provider.connect_path as string)}
                            type="button"
                          >
                            <Icon className="h-3.5 w-3.5" name="Wifi" />
                            {labels.connectLabel}
                          </button>
                        ) : null}
                        {testResults[provider.name] ? (
                          <span
                            className={`inline-flex rounded-full border px-2.5 py-0.5 text-xs font-semibold text-ink ${
                              testResults[provider.name].ok
                                ? "border-success/25 bg-success/10"
                                : "border-danger/25 bg-danger/10"
                            }`}
                            title={testResults[provider.name].detail}
                          >
                            {testResults[provider.name].detail}
                          </span>
                        ) : null}
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      </SectionCard>

      <SectionCard subtitle={labels.planAccessSubtitle} title={labels.planAccessTitle}>
        {status.providers.filter((provider) => provider.kind === "oauth").length === 0 ? (
          <p className="text-sm text-muted">{labels.noPlanAccessProviders}</p>
        ) : (
          <div className="overflow-hidden rounded-xl border border-border">
            <div className="grid grid-cols-[minmax(120px,1fr)_minmax(220px,2fr)_auto] gap-3 border-b border-hairline bg-canvas px-4 py-3 text-xs font-semibold uppercase tracking-wide text-faint max-md:hidden">
              {labels.planAccessColumns.map((column) => (
                <span key={column}>{column}</span>
              ))}
            </div>
            <div className="divide-y divide-hairline">
              {status.providers
                .filter((provider) => provider.kind === "oauth")
                .map((provider) => {
                  const selectedPlans = planDrafts[provider.name] ?? new Set<string>();
                  const savedValue = runtimeConfig[`integration.${provider.name}.plans`] ?? "";
                  return (
                    <div className="grid gap-3 px-4 py-4 md:grid-cols-[minmax(120px,1fr)_minmax(220px,2fr)_auto] md:items-center" key={provider.name}>
                      <div>
                        <p className="font-mono text-xs font-semibold text-ink">{provider.name}</p>
                        <p className="mt-1 text-xs text-muted">{labels.planAccessHint}</p>
                      </div>
                      <div className="flex flex-wrap gap-2">
                        {(plans ?? []).map((plan) => {
                          const checked = selectedPlans.has(plan.name);
                          return (
                            <label
                              className={`inline-flex min-h-9 cursor-pointer items-center gap-2 rounded-full border px-3 py-1.5 text-xs font-semibold transition ${
                                checked
                                  ? "border-blue/40 bg-blue/10 text-ink"
                                  : "border-border bg-white text-muted"
                              }`}
                              key={`${provider.name}-${plan.name}`}
                            >
                              <input
                                checked={checked}
                                className="sr-only"
                                onChange={() => toggleProviderPlan(provider.name, plan.name)}
                                type="checkbox"
                              />
                              <Icon className="h-3.5 w-3.5" name={checked ? "CheckCircle2" : "Circle"} />
                              {plan.name}
                            </label>
                          );
                        })}
                      </div>
                      <div className="flex flex-wrap items-center gap-2 md:justify-end">
                        <span className="font-mono text-xs text-muted">{savedValue || "-"}</span>
                        <button
                          className="inline-flex h-9 items-center rounded-full bg-ink px-4 text-xs font-semibold text-white transition hover:bg-text disabled:opacity-50"
                          disabled={planBusy === provider.name}
                          onClick={() => void saveProviderPlans(provider.name)}
                          type="button"
                        >
                          {planBusy === provider.name ? labels.savingLabel : labels.saveLabel}
                        </button>
                        {planSaved === provider.name ? <Pill text={labels.savedLabel} tone="ok" /> : null}
                      </div>
                    </div>
                  );
                })}
            </div>
          </div>
        )}
      </SectionCard>

      <SectionCard subtitle={labels.stripeSubtitle} title={labels.stripeTitle}>
        <div className="grid gap-3 sm:grid-cols-2">
          {(plans ?? []).map((plan) => (
            <div className="rounded-lg border border-hairline p-3" key={plan.name}>
              <p className="text-sm font-semibold capitalize text-ink">{plan.name}</p>
              {plan.price_cents === 0 ? (
                <p className="mt-2 text-xs text-muted">{labels.stripeFreeNote}</p>
              ) : (
                <div className="mt-2 flex flex-wrap items-center gap-2">
                  <input
                    aria-label={`${plan.name} stripe price id`}
                    className="h-9 w-48 rounded-lg border border-border bg-white px-2.5 font-mono text-xs text-ink outline-none transition focus:border-blue"
                    onChange={(event) =>
                      setPriceDrafts((current) => ({ ...current, [plan.name]: event.target.value }))
                    }
                    placeholder={labels.placeholder}
                    value={priceDrafts[plan.name] ?? ""}
                  />
                  <button
                    className="inline-flex h-9 items-center rounded-full bg-ink px-4 text-xs font-semibold text-white transition hover:bg-text disabled:opacity-50"
                    disabled={priceBusy === plan.name || !(priceDrafts[plan.name] ?? "").trim()}
                    onClick={() => void savePrice(plan.name)}
                    type="button"
                  >
                    {priceBusy === plan.name ? labels.savingLabel : labels.saveLabel}
                  </button>
                  {priceSaved === plan.name ? <Pill text={labels.savedLabel} tone="ok" /> : null}
                </div>
              )}
            </div>
          ))}
        </div>
      </SectionCard>
    </div>
  );
}
