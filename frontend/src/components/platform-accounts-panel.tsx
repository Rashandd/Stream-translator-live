"use client";

import { useCallback, useEffect, useState } from "react";
import { BrandMark, brandForPlatform } from "@/src/components/brand-mark";
import { Icon } from "@/src/components/icon";
import { userFetch } from "@/src/lib/api";
import { findIntegration, type IntegrationStatus } from "@/src/lib/distribution";

type Account = {
  name: string;
  provider: string;
  detail: string;
  scopes: string;
  icon: string;
  disabled?: boolean;
};

type Labels = {
  oauthNotice: string;
  connectLabel: string;
  connectingLabel: string;
  disconnectLabel: string;
  disconnectingLabel: string;
  connectedLabel: string;
  notConnectedLabel: string;
  comingSoonLabel: string;
  scopesLabel: string;
  errors: {
    unauthorized: string;
    network: string;
    actionFailed: string;
  };
  accounts: Account[];
};

export function PlatformAccountsPanel({ labels }: { labels: Labels }) {
  const [integrations, setIntegrations] = useState<IntegrationStatus[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [connectBusy, setConnectBusy] = useState<string | null>(null);
  const [disconnectBusy, setDisconnectBusy] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const res = await userFetch("/api/integrations");
      if (res.status === 401 || res.status === 403) {
        setError(labels.errors.unauthorized);
        return;
      }
      if (!res.ok) {
        setError(labels.errors.actionFailed);
        return;
      }
      setError(null);
      setIntegrations((await res.json()) as IntegrationStatus[]);
    } catch {
      setError(labels.errors.network);
    } finally {
      setLoading(false);
    }
  }, [labels.errors]);

  useEffect(() => {
    void load();
  }, [load]);

  async function connect(provider: string) {
    setConnectBusy(provider);
    setError(null);
    try {
      const res = await userFetch(`/api/integrations/${provider}/authorize`);
      if (res.status === 401 || res.status === 403) {
        setError(labels.errors.unauthorized);
        return;
      }
      if (!res.ok) {
        setError(labels.errors.actionFailed);
        return;
      }
      const data = (await res.json()) as { authorize_url: string };
      window.location.href = data.authorize_url;
    } catch {
      setError(labels.errors.network);
    } finally {
      setConnectBusy(null);
    }
  }

  async function disconnect(provider: string) {
    setDisconnectBusy(provider);
    setError(null);
    try {
      const res = await userFetch(`/api/integrations/${provider}`, { method: "DELETE" });
      if (res.status === 401 || res.status === 403) {
        setError(labels.errors.unauthorized);
        return;
      }
      if (!res.ok) {
        setError(labels.errors.actionFailed);
        return;
      }
      await load();
    } catch {
      setError(labels.errors.network);
    } finally {
      setDisconnectBusy(null);
    }
  }

  return (
    <>
      <div className="flex items-start gap-3 rounded-xl border border-border bg-surface p-4">
        <Icon className="mt-0.5 h-5 w-5 shrink-0 text-blue" name="LockKeyhole" />
        <p className="text-sm leading-6 text-muted">{labels.oauthNotice}</p>
      </div>

      {error ? (
        <p className="mt-4 rounded-lg border border-danger/25 bg-danger/10 px-3 py-2 text-sm text-ink" role="alert">
          {error}
        </p>
      ) : null}

      <ul className="mt-6 grid gap-4 sm:grid-cols-2">
        {labels.accounts.map((account) => {
          const brand = brandForPlatform(account.provider);
          const status = account.disabled ? null : findIntegration(integrations, account.provider);
          const connected = status?.connected ?? false;

          return (
            <li className="flex flex-col gap-4 rounded-xl border border-border bg-surface p-5" key={account.provider}>
              <div className="flex items-start justify-between gap-3">
                <div className="flex min-w-0 items-center gap-3">
                  <span className="flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-canvas text-ink">
                    {brand ? (
                      <BrandMark brand={brand} className="h-5 w-5" />
                    ) : (
                      <Icon className="h-5 w-5" name={account.icon} />
                    )}
                  </span>
                  <div className="min-w-0">
                    <p className="truncate text-sm font-semibold text-ink">{account.name}</p>
                    <p className="truncate text-sm text-muted">{account.detail}</p>
                  </div>
                </div>
                <span
                  className={`inline-flex shrink-0 items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs font-semibold ${
                    connected ? "border-success/25 bg-success/10 text-ink" : "border-border bg-canvas text-muted"
                  }`}
                >
                  <span className={`h-1.5 w-1.5 rounded-full ${connected ? "bg-success" : "bg-faint"}`} />
                  {account.disabled
                    ? labels.comingSoonLabel
                    : connected
                      ? labels.connectedLabel
                      : labels.notConnectedLabel}
                </span>
              </div>

              {!account.disabled ? (
                <div className="rounded-lg border border-hairline bg-canvas px-3 py-2">
                  <p className="text-xs font-medium text-muted">{labels.scopesLabel}</p>
                  <p className="mt-0.5 text-sm text-ink">{account.scopes}</p>
                </div>
              ) : null}

              <button
                className={`inline-flex h-10 items-center justify-center gap-1.5 rounded-full px-4 text-sm font-semibold transition disabled:cursor-not-allowed disabled:opacity-50 ${
                  connected
                    ? "border border-border bg-surface text-ink hover:border-danger hover:text-danger"
                    : "bg-primary text-white hover:bg-primary-hover"
                }`}
                disabled={
                  account.disabled ||
                  loading ||
                  connectBusy === account.provider ||
                  disconnectBusy === account.provider
                }
                onClick={() =>
                  void (connected ? disconnect(account.provider) : connect(account.provider))
                }
                type="button"
              >
                {connected ? (
                  disconnectBusy === account.provider ? (
                    labels.disconnectingLabel
                  ) : (
                    labels.disconnectLabel
                  )
                ) : connectBusy === account.provider ? (
                  labels.connectingLabel
                ) : (
                  <>
                    <Icon className="h-4 w-4" name="Plus" />
                    {labels.connectLabel}
                  </>
                )}
              </button>
            </li>
          );
        })}
      </ul>
    </>
  );
}
