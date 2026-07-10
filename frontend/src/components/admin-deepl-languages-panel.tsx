"use client";

import { useCallback, useEffect, useState } from "react";
import { adminFetch } from "@/src/lib/api";

/** GET /api/translate/catalog kaydı (auth gerekmez, gizli veri yok). */
type CatalogEntry = {
  code: string;
  name: string;
  supports_formality: boolean;
};

type Labels = {
  loading: string;
  saveLabel: string;
  savingLabel: string;
  savedLabel: string;
  errorNetwork: string;
  errorSave: string;
};

const ENABLED_TARGETS_KEY = "translate.deepl.enabled_targets";

export function AdminDeeplLanguagesPanel({ labels }: { labels: Labels }) {
  const [catalog, setCatalog] = useState<CatalogEntry[] | null>(null);
  const [enabled, setEnabled] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  const load = useCallback(async () => {
    setError(null);
    try {
      const [catalogRes, configRes] = await Promise.all([
        adminFetch("/api/translate/catalog"),
        adminFetch("/config"),
      ]);
      if (!catalogRes.ok || !configRes.ok) {
        setError(labels.errorNetwork);
        return;
      }
      const catalogBody = (await catalogRes.json()) as CatalogEntry[];
      const config = (await configRes.json()) as Record<string, string>;
      const csv = config[ENABLED_TARGETS_KEY] ?? "";
      setCatalog(catalogBody);
      setEnabled(
        new Set(
          csv
            .split(",")
            .map((code) => code.trim())
            .filter(Boolean),
        ),
      );
    } catch {
      setError(labels.errorNetwork);
    }
  }, [labels.errorNetwork]);

  useEffect(() => {
    void load();
  }, [load]);

  const toggle = useCallback((code: string) => {
    setSaved(false);
    setEnabled((prev) => {
      const next = new Set(prev);
      if (next.has(code)) {
        next.delete(code);
      } else {
        next.add(code);
      }
      return next;
    });
  }, []);

  const save = useCallback(async () => {
    setError(null);
    setSaved(false);
    setSaving(true);
    try {
      const res = await adminFetch("/admin/config", {
        method: "PUT",
        body: JSON.stringify({
          key: ENABLED_TARGETS_KEY,
          value: Array.from(enabled).join(","),
        }),
      });
      if (!res.ok) {
        setError(labels.errorSave);
      } else {
        setSaved(true);
      }
    } catch {
      setError(labels.errorSave);
    } finally {
      setSaving(false);
    }
  }, [enabled, labels.errorSave]);

  if (!catalog && !error) {
    return <p className="text-sm text-muted">{labels.loading}</p>;
  }

  return (
    <div className="space-y-4">
      {error ? (
        <p className="rounded-lg border border-danger/25 bg-danger/10 px-4 py-3 text-sm text-ink" role="alert">
          {error}
        </p>
      ) : null}
      <div className="flex flex-wrap gap-2">
        {(catalog ?? []).map((lang) => {
          const active = enabled.has(lang.code);
          return (
            <button
              aria-pressed={active}
              className={`inline-flex items-center gap-1.5 rounded-full border px-3 py-1.5 text-xs font-semibold transition ${
                active ? "border-primary bg-primary text-white" : "border-border text-text hover:bg-canvas"
              }`}
              key={lang.code}
              onClick={() => toggle(lang.code)}
              type="button"
            >
              <span className="font-mono">{lang.code.toUpperCase()}</span>
              {lang.name}
            </button>
          );
        })}
      </div>
      <button
        className="inline-flex h-9 items-center rounded-full bg-primary px-4 text-sm font-semibold text-white transition hover:bg-primary-hover disabled:opacity-50"
        disabled={saving}
        onClick={() => void save()}
        type="button"
      >
        {saving ? labels.savingLabel : saved ? labels.savedLabel : labels.saveLabel}
      </button>
    </div>
  );
}
