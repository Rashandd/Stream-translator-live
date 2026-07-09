"use client";

import { useCallback, useEffect, useState } from "react";
import { StatusPill } from "@/src/components/status-pill";
import { adminFetch } from "@/src/lib/api";

type ProviderOption = { slug: string; name: string; status: string };

type Group = {
  key: string;
  label: string;
  providerConfigKey: string;
  fallbackConfigKey: string;
  options: ProviderOption[];
  fallbackLabel: string;
  fallbackNone: string;
};

type Labels = {
  groups: Group[];
  loading: string;
  savingLabel: string;
  errors: { unauthorized: string; network: string };
  signInLabel: string;
};

type Props = { labels: Labels; loginHref: string; statusLabels: Record<string, string> };

export function AdminRoutingPanel({ labels, loginHref, statusLabels }: Props) {
  const [live, setLive] = useState<Record<string, string> | null>(null);
  const [unauthorized, setUnauthorized] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savingKey, setSavingKey] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const res = await adminFetch("/config");
      if (res.status === 401 || res.status === 403) {
        setUnauthorized(true);
        return;
      }
      if (!res.ok) {
        setError(labels.errors.network);
        return;
      }
      setUnauthorized(false);
      setLive((await res.json()) as Record<string, string>);
    } catch {
      setError(labels.errors.network);
    }
  }, [labels.errors.network]);

  useEffect(() => {
    void load();
  }, [load]);

  const set = useCallback(
    async (key: string, value: string) => {
      setSavingKey(key);
      setError(null);
      try {
        const res = await adminFetch("/admin/config", {
          method: "PUT",
          body: JSON.stringify({ key, value }),
        });
        if (res.status === 401 || res.status === 403) {
          setUnauthorized(true);
          return;
        }
        if (!res.ok) {
          setError(labels.errors.network);
          return;
        }
        setLive((current) => ({ ...current, [key]: value }));
      } catch {
        setError(labels.errors.network);
      } finally {
        setSavingKey(null);
      }
    },
    [labels.errors.network],
  );

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

      {labels.groups.map((group) => {
        const activeSlug = live?.[group.providerConfigKey];
        const fallbackSlug = live?.[group.fallbackConfigKey] ?? "";
        const realOptions = group.options.filter((o) => o.status === "enabled");
        const fallbackChoices = realOptions.filter((o) => o.slug !== activeSlug);

        return (
          <section className="rounded-xl border border-border bg-white p-5 sm:p-6" key={group.key}>
            <div className="flex flex-wrap items-center justify-between gap-2">
              <h2 className="text-lg font-semibold text-ink">{group.label}</h2>
              <code className="rounded-md bg-canvas px-2 py-1 font-mono text-xs text-muted">
                {group.providerConfigKey}
              </code>
            </div>

            <div className="mt-4 inline-flex flex-wrap gap-1 rounded-xl border border-border bg-canvas p-1">
              {group.options.map((option) => {
                const selectable = option.status === "enabled";
                const active = live !== null && option.slug === activeSlug;
                return (
                  <button
                    aria-pressed={active}
                    className={`inline-flex items-center gap-2 rounded-lg px-4 py-2 text-sm font-semibold transition disabled:cursor-not-allowed ${
                      active
                        ? "bg-ink text-white"
                        : selectable
                          ? "text-text hover:bg-white"
                          : "text-faint"
                    }`}
                    disabled={!selectable || !live || active || savingKey === group.providerConfigKey}
                    key={option.slug}
                    onClick={() => void set(group.providerConfigKey, option.slug)}
                    type="button"
                  >
                    {savingKey === group.providerConfigKey && option.slug === activeSlug
                      ? labels.savingLabel
                      : option.name}
                    {!selectable ? (
                      <StatusPill label={option.status} text={statusLabels[option.status] ?? option.status} />
                    ) : null}
                  </button>
                );
              })}
            </div>

            <div className="mt-3 flex flex-wrap items-center gap-2 text-sm text-muted">
              <span>{group.fallbackLabel}:</span>
              {fallbackChoices.length > 0 ? (
                <select
                  className="h-8 rounded-lg border border-border bg-white px-2 text-sm text-ink outline-none transition focus:border-blue"
                  disabled={!live || savingKey === group.fallbackConfigKey}
                  onChange={(event) => void set(group.fallbackConfigKey, event.target.value)}
                  value={fallbackSlug}
                >
                  <option value="">{group.fallbackNone}</option>
                  {fallbackChoices.map((option) => (
                    <option key={option.slug} value={option.slug}>
                      {option.name}
                    </option>
                  ))}
                </select>
              ) : (
                <span className="font-semibold text-ink">
                  {fallbackSlug
                    ? (realOptions.find((o) => o.slug === fallbackSlug)?.name ?? fallbackSlug)
                    : group.fallbackNone}
                </span>
              )}
            </div>
          </section>
        );
      })}
    </div>
  );
}
