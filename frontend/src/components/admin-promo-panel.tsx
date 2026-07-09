"use client";

import { useCallback, useEffect, useState } from "react";
import { SectionCard } from "@/src/components/dashboard-ui";
import { adminFetch } from "@/src/lib/api";

type Labels = {
  title: string;
  subtitle: string;
  gateLabel: string;
  gateHint: string;
  enabledLabel: string;
  messageLabel: string;
  saveLabel: string;
  savingLabel: string;
  saved: string;
  errors: { unauthorized: string; network: string; saveFailed: string };
  signInLabel: string;
};

type Props = { labels: Labels; loginHref: string };

const GATE_KEY = "founding.gate.enabled";
const PROMO_ENABLED_KEY = "promo.founding.enabled";
const PROMO_MESSAGE_KEY = "promo.founding.message";

export function AdminPromoPanel({ labels, loginHref }: Props) {
  const [gateEnabled, setGateEnabled] = useState(false);
  const [promoEnabled, setPromoEnabled] = useState(false);
  const [message, setMessage] = useState("");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [unauthorized, setUnauthorized] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
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
      const config = (await res.json()) as Record<string, string>;
      setGateEnabled(config[GATE_KEY] === "true");
      setPromoEnabled(config[PROMO_ENABLED_KEY] === "true");
      setMessage(config[PROMO_MESSAGE_KEY] ?? "");
    } catch {
      setError(labels.errors.network);
    } finally {
      setLoading(false);
    }
  }, [labels.errors.network]);

  useEffect(() => {
    void load();
  }, [load]);

  async function save() {
    setSaving(true);
    setSaved(false);
    setError(null);
    try {
      const updates: [string, string][] = [
        [GATE_KEY, gateEnabled ? "true" : "false"],
        [PROMO_ENABLED_KEY, promoEnabled ? "true" : "false"],
        [PROMO_MESSAGE_KEY, message],
      ];
      for (const [key, value] of updates) {
        const res = await adminFetch("/admin/config", {
          method: "PUT",
          body: JSON.stringify({ key, value }),
        });
        if (res.status === 401 || res.status === 403) {
          setUnauthorized(true);
          return;
        }
        if (!res.ok) {
          setError(labels.errors.saveFailed);
          return;
        }
      }
      setSaved(true);
    } catch {
      setError(labels.errors.network);
    } finally {
      setSaving(false);
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
    <SectionCard subtitle={labels.subtitle} title={labels.title}>
      {loading ? null : (
        <div className="space-y-4">
          {error ? (
            <p className="rounded-lg border border-danger/25 bg-danger/10 px-3 py-2 text-sm text-ink" role="alert">
              {error}
            </p>
          ) : null}

          <label className="flex items-start gap-3">
            <input
              checked={gateEnabled}
              className="mt-0.5 h-4 w-4"
              onChange={(e) => setGateEnabled(e.target.checked)}
              type="checkbox"
            />
            <span>
              <span className="block text-sm font-semibold text-ink">{labels.gateLabel}</span>
              <span className="mt-0.5 block text-xs text-muted">{labels.gateHint}</span>
            </span>
          </label>

          <label className="flex items-center gap-3">
            <input
              checked={promoEnabled}
              className="h-4 w-4"
              onChange={(e) => setPromoEnabled(e.target.checked)}
              type="checkbox"
            />
            <span className="text-sm font-semibold text-ink">{labels.enabledLabel}</span>
          </label>

          <div>
            <label className="block text-xs font-medium text-muted" htmlFor="promo-message">
              {labels.messageLabel}
            </label>
            <textarea
              className="mt-1 w-full rounded-lg border border-border bg-white px-3 py-2 text-sm text-ink"
              id="promo-message"
              onChange={(e) => setMessage(e.target.value)}
              rows={2}
              value={message}
            />
          </div>

          <button
            className="inline-flex h-10 items-center justify-center rounded-full bg-ink px-5 text-sm font-semibold text-white transition hover:bg-text disabled:opacity-60"
            disabled={saving}
            onClick={() => void save()}
            type="button"
          >
            {saving ? labels.savingLabel : saved ? labels.saved : labels.saveLabel}
          </button>
        </div>
      )}
    </SectionCard>
  );
}
