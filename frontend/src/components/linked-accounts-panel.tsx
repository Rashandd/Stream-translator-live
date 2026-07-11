"use client";

import { useCallback, useEffect, useState } from "react";
import { useSearchParams } from "next/navigation";
import { BrandMark, brandForPlatform } from "@/src/components/brand-mark";
import { Icon } from "@/src/components/icon";
import { userFetch } from "@/src/lib/api";

type ProviderConfig = {
  name: string;
  provider: string;
  icon: string;
};

type Labels = {
  passwordLabel: string;
  passwordSetHint: string;
  passwordNotSetHint: string;
  linkLabel: string;
  linkingLabel: string;
  unlinkLabel: string;
  unlinkingLabel: string;
  linkedLabel: string;
  notLinkedLabel: string;
  lastMethodNotice: string;
  linkedBanner: string;
  providers: ProviderConfig[];
  errors: {
    unauthorized: string;
    network: string;
    actionFailed: string;
  };
};

type IdentitiesResponse = {
  has_password: boolean;
  identities: { provider: string; linked_at_ms: number | null }[];
};

export function LinkedAccountsPanel({ labels }: { labels: Labels }) {
  const searchParams = useSearchParams();
  const [data, setData] = useState<IdentitiesResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<{ provider: string; action: "link" | "unlink" } | null>(null);
  const [linkedBanner, setLinkedBanner] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const res = await userFetch("/api/auth/oauth/identities");
      if (res.status === 401 || res.status === 403) {
        setError(labels.errors.unauthorized);
        return;
      }
      if (!res.ok) {
        setError(labels.errors.actionFailed);
        return;
      }
      setError(null);
      setData((await res.json()) as IdentitiesResponse);
    } catch {
      setError(labels.errors.network);
    } finally {
      setLoading(false);
    }
  }, [labels.errors]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    const linked = searchParams.get("linked");
    if (linked) {
      setLinkedBanner(labels.linkedBanner.replace("{provider}", linked));
    }
  }, [searchParams, labels.linkedBanner]);

  async function link(provider: string) {
    setBusy({ provider, action: "link" });
    setError(null);
    try {
      const res = await userFetch(`/api/auth/oauth/${provider}/link/start`);
      if (res.status === 401 || res.status === 403) {
        setError(labels.errors.unauthorized);
        return;
      }
      if (!res.ok) {
        setError(labels.errors.actionFailed);
        return;
      }
      const body = (await res.json()) as { authorize_url: string };
      window.location.href = body.authorize_url;
    } catch {
      setError(labels.errors.network);
    } finally {
      setBusy(null);
    }
  }

  async function unlink(provider: string) {
    setBusy({ provider, action: "unlink" });
    setError(null);
    try {
      const res = await userFetch(`/api/auth/oauth/${provider}`, { method: "DELETE" });
      if (res.status === 401 || res.status === 403) {
        setError(labels.errors.unauthorized);
        return;
      }
      if (res.status === 409) {
        setError(labels.lastMethodNotice);
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
      setBusy(null);
    }
  }

  const linkedCount = (data?.identities.length ?? 0) + (data?.has_password ? 1 : 0);

  return (
    <>
      {linkedBanner ? (
        <p className="rounded-lg border border-success/25 bg-success/10 px-3 py-2 text-sm text-ink" role="status">
          {linkedBanner}
        </p>
      ) : null}

      {error ? (
        <p className="mt-4 rounded-lg border border-danger/25 bg-danger/10 px-3 py-2 text-sm text-ink" role="alert">
          {error}
        </p>
      ) : null}

      <div className="mt-4 flex items-center justify-between gap-3 rounded-xl border border-border bg-surface p-4">
        <div className="flex items-center gap-3">
          <span className="flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-canvas text-ink">
            <Icon className="h-5 w-5" name="LockKeyhole" />
          </span>
          <div>
            <p className="text-sm font-semibold text-ink">{labels.passwordLabel}</p>
            <p className="text-sm text-muted">
              {data?.has_password ? labels.passwordSetHint : labels.passwordNotSetHint}
            </p>
          </div>
        </div>
        <span
          className={`inline-flex shrink-0 items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs font-semibold ${
            data?.has_password ? "border-success/25 bg-success/10 text-ink" : "border-border bg-canvas text-muted"
          }`}
        >
          <span className={`h-1.5 w-1.5 rounded-full ${data?.has_password ? "bg-success" : "bg-faint"}`} />
          {data?.has_password ? labels.linkedLabel : labels.notLinkedLabel}
        </span>
      </div>

      <ul className="mt-4 grid gap-4 sm:grid-cols-2">
        {labels.providers.map((config) => {
          const brand = brandForPlatform(config.provider);
          const identity = data?.identities.find((i) => i.provider === config.provider);
          const linked = Boolean(identity);
          const isLastMethod = linked && linkedCount <= 1;

          return (
            <li className="flex flex-col gap-4 rounded-xl border border-border bg-surface p-5" key={config.provider}>
              <div className="flex items-start justify-between gap-3">
                <div className="flex min-w-0 items-center gap-3">
                  <span className="flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-canvas text-ink">
                    {brand ? (
                      <BrandMark brand={brand} className="h-5 w-5" />
                    ) : (
                      <Icon className="h-5 w-5" name={config.icon} />
                    )}
                  </span>
                  <p className="truncate text-sm font-semibold text-ink">{config.name}</p>
                </div>
                <span
                  className={`inline-flex shrink-0 items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs font-semibold ${
                    linked ? "border-success/25 bg-success/10 text-ink" : "border-border bg-canvas text-muted"
                  }`}
                >
                  <span className={`h-1.5 w-1.5 rounded-full ${linked ? "bg-success" : "bg-faint"}`} />
                  {linked ? labels.linkedLabel : labels.notLinkedLabel}
                </span>
              </div>

              <button
                className={`inline-flex h-10 items-center justify-center gap-1.5 rounded-full px-4 text-sm font-semibold transition disabled:cursor-not-allowed disabled:opacity-50 ${
                  linked
                    ? "border border-border bg-surface text-ink hover:border-danger hover:text-danger"
                    : "bg-primary text-on-primary hover:bg-primary-hover"
                }`}
                disabled={loading || (isLastMethod && !linked) || busy?.provider === config.provider}
                onClick={() => void (linked ? unlink(config.provider) : link(config.provider))}
                title={isLastMethod ? labels.lastMethodNotice : undefined}
                type="button"
              >
                {linked ? (
                  busy?.provider === config.provider && busy.action === "unlink" ? (
                    labels.unlinkingLabel
                  ) : (
                    labels.unlinkLabel
                  )
                ) : busy?.provider === config.provider && busy.action === "link" ? (
                  labels.linkingLabel
                ) : (
                  <>
                    <Icon className="h-4 w-4" name="Plus" />
                    {labels.linkLabel}
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
