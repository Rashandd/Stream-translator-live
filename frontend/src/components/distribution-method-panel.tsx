"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { Icon } from "@/src/components/icon";
import { userFetch } from "@/src/lib/api";
import {
  findIntegration,
  integrationUiState,
  type IntegrationStatus,
  type IntegrationUiState,
} from "@/src/lib/distribution";

type MethodCopy = {
  id: string;
  provider?: string;
  name: string;
  badge: string;
  summary: string;
  icon: string;
  bullets: string[];
};

type TestOutcome = {
  ok: boolean;
  detail: string;
};

type Labels = {
  methodsTitle: string;
  methodsSubtitle: string;
  defaultMethodId: string;
  recommendedLabel: string;
  selectedLabel: string;
  chooseLabel: string;
  connectLabel: string;
  connectingLabel: string;
  testLabel: string;
  testingLabel: string;
  loadingLabel: string;
  states: Record<IntegrationUiState | "ready", string>;
  uploadWarning: {
    title: string;
    body: string;
    metric: string;
  };
  errors: {
    unauthorized: string;
    network: string;
    actionFailed: string;
  };
  methods: MethodCopy[];
};

type Props = {
  labels: Labels;
};

const stateTone: Record<IntegrationUiState | "ready", string> = {
  ready: "border-success/25 bg-success/10 text-ink",
  connected: "border-success/25 bg-success/10 text-ink",
  available: "border-blue/25 bg-blue/10 text-ink",
  loading: "border-border bg-canvas text-muted",
  not_configured: "border-warning/25 bg-warning/10 text-ink",
  unknown: "border-danger/25 bg-danger/10 text-ink",
};

function StatusBadge({ label, tone }: { label: string; tone: IntegrationUiState | "ready" }) {
  return (
    <span className={`inline-flex rounded-full border px-2.5 py-1 text-xs font-semibold ${stateTone[tone]}`}>
      {label}
    </span>
  );
}

export function DistributionMethodPanel({ labels }: Props) {
  const [selected, setSelected] = useState(labels.defaultMethodId);
  const [integrations, setIntegrations] = useState<IntegrationStatus[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [unauthorized, setUnauthorized] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [connectBusy, setConnectBusy] = useState<string | null>(null);
  const [testing, setTesting] = useState<string | null>(null);
  const [testResults, setTestResults] = useState<Record<string, TestOutcome>>({});

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const response = await userFetch("/api/integrations");
      if (response.status === 401 || response.status === 403) {
        setUnauthorized(true);
        setIntegrations([]);
        return;
      }
      if (!response.ok) {
        setError(labels.errors.actionFailed);
        setIntegrations([]);
        return;
      }
      setUnauthorized(false);
      setError(null);
      setIntegrations((await response.json()) as IntegrationStatus[]);
    } catch {
      setError(labels.errors.network);
      setIntegrations([]);
    } finally {
      setLoading(false);
    }
  }, [labels.errors.actionFailed, labels.errors.network]);

  useEffect(() => {
    void load();
  }, [load]);

  async function connect(provider: string) {
    setConnectBusy(provider);
    setError(null);
    try {
      const response = await userFetch(`/api/integrations/${provider}/authorize`);
      if (response.status === 401 || response.status === 403) {
        setUnauthorized(true);
        return;
      }
      if (!response.ok) {
        const detail = await response.text();
        setError(detail || labels.errors.actionFailed);
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

  async function testConnection(provider: string) {
    setTesting(provider);
    setError(null);
    try {
      const response = await userFetch(`/api/integrations/${provider}/test`, { method: "POST" });
      if (response.status === 401 || response.status === 403) {
        setUnauthorized(true);
        return;
      }
      if (response.ok) {
        const outcome = (await response.json()) as TestOutcome;
        setTestResults((current) => ({ ...current, [provider]: outcome }));
        return;
      }
      const detail = await response.text();
      setTestResults((current) => ({
        ...current,
        [provider]: { ok: false, detail: detail || labels.errors.actionFailed },
      }));
    } catch {
      setError(labels.errors.network);
    } finally {
      setTesting(null);
    }
  }

  const integrationByProvider = useMemo(
    () =>
      new Map(
        labels.methods
          .filter((method) => method.provider)
          .map((method) => [method.provider as string, findIntegration(integrations, method.provider as string)]),
      ),
    [integrations, labels.methods],
  );

  return (
    <section className="space-y-4">
      <div>
        <h2 className="text-lg font-semibold text-ink">{labels.methodsTitle}</h2>
        <p className="mt-1 text-sm leading-6 text-muted">{labels.methodsSubtitle}</p>
      </div>

      {unauthorized ? (
        <p className="rounded-lg border border-warning/25 bg-warning/10 px-3 py-2 text-sm text-ink" role="alert">
          {labels.errors.unauthorized}
        </p>
      ) : null}
      {error ? (
        <p className="rounded-lg border border-danger/25 bg-danger/10 px-3 py-2 text-sm text-ink" role="alert">
          {error}
        </p>
      ) : null}

      <div className="grid gap-4 lg:grid-cols-2">
        {labels.methods.map((method) => {
          const isSdk = !method.provider;
          const providerStatus = method.provider ? integrationByProvider.get(method.provider) ?? null : null;
          const uiState = isSdk ? "ready" : integrationUiState(providerStatus, loading);
          const selectedMethod = selected === method.id;
          const canConnect =
            method.provider &&
            providerStatus?.configured &&
            !providerStatus.connected;
          const canTest = method.provider && providerStatus?.connected;
          const testResult = method.provider ? testResults[method.provider] : null;

          return (
            <article
              className={`rounded-xl border bg-white p-5 transition ${
                selectedMethod ? "border-blue shadow-panel" : "border-border"
              }`}
              key={method.id}
            >
              <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
                <div className="flex min-w-0 items-start gap-3">
                  <span className="flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-canvas text-ink">
                    <Icon className="h-5 w-5" name={method.icon} />
                  </span>
                  <div className="min-w-0">
                    <div className="flex flex-wrap items-center gap-2">
                      <h3 className="text-base font-semibold text-ink">{method.name}</h3>
                      {method.id === labels.defaultMethodId ? (
                        <StatusBadge label={labels.recommendedLabel} tone="ready" />
                      ) : null}
                      {selectedMethod ? <StatusBadge label={labels.selectedLabel} tone="available" /> : null}
                    </div>
                    <p className="mt-1 text-xs font-semibold uppercase tracking-wide text-muted">{method.badge}</p>
                  </div>
                </div>
                <StatusBadge label={labels.states[uiState]} tone={uiState} />
              </div>

              <p className="mt-4 text-sm leading-6 text-muted">{method.summary}</p>
              <ul className="mt-4 space-y-2">
                {method.bullets.map((bullet) => (
                  <li className="flex gap-2 text-sm leading-6 text-text" key={bullet}>
                    <Icon className="mt-1 h-4 w-4 shrink-0 text-blue" name="CheckCircle2" />
                    <span>{bullet}</span>
                  </li>
                ))}
              </ul>

              <div className="mt-5 flex flex-wrap items-center gap-2">
                <button
                  className="inline-flex h-10 items-center justify-center gap-1.5 rounded-full border border-border bg-white px-4 text-sm font-semibold text-ink transition hover:border-blue hover:text-blue disabled:opacity-50"
                  disabled={selectedMethod}
                  onClick={() => setSelected(method.id)}
                  type="button"
                >
                  <Icon className="h-4 w-4" name={selectedMethod ? "CheckCircle2" : "Circle"} />
                  {selectedMethod ? labels.selectedLabel : labels.chooseLabel}
                </button>

                {method.provider ? (
                  <button
                    className="inline-flex h-10 items-center justify-center gap-1.5 rounded-full bg-ink px-4 text-sm font-semibold text-white transition hover:bg-text disabled:opacity-50"
                    disabled={!canConnect || connectBusy === method.provider}
                    onClick={() => void connect(method.provider as string)}
                    type="button"
                  >
                    <Icon className="h-4 w-4" name="Wifi" />
                    {connectBusy === method.provider ? labels.connectingLabel : labels.connectLabel}
                  </button>
                ) : null}

                {method.provider ? (
                  <button
                    className="inline-flex h-10 items-center justify-center gap-1.5 rounded-full border border-blue/40 px-4 text-sm font-semibold text-ink transition hover:bg-blue/10 disabled:opacity-50"
                    disabled={!canTest || testing === method.provider}
                    onClick={() => void testConnection(method.provider as string)}
                    type="button"
                  >
                    <Icon className="h-4 w-4" name="RefreshCw" />
                    {testing === method.provider ? labels.testingLabel : labels.testLabel}
                  </button>
                ) : null}

                {testResult ? (
                  <span
                    className={`inline-flex rounded-full border px-2.5 py-1 text-xs font-semibold text-ink ${
                      testResult.ok ? "border-success/25 bg-success/10" : "border-danger/25 bg-danger/10"
                    }`}
                    title={testResult.detail}
                  >
                    {testResult.detail}
                  </span>
                ) : null}
              </div>
            </article>
          );
        })}
      </div>

      <div className="flex flex-col gap-3 rounded-xl border border-warning/25 bg-warning/10 p-4 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex min-w-0 items-start gap-3">
          <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-white text-ink">
            <Icon className="h-5 w-5" name="Gauge" />
          </span>
          <div>
            <p className="text-sm font-semibold text-ink">{labels.uploadWarning.title}</p>
            <p className="mt-1 text-sm leading-6 text-muted">{labels.uploadWarning.body}</p>
          </div>
        </div>
        <span className="inline-flex shrink-0 rounded-full border border-warning/30 bg-white px-3 py-1.5 font-mono text-xs font-semibold text-ink">
          {labels.uploadWarning.metric}
        </span>
      </div>
    </section>
  );
}
