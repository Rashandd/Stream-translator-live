"use client";

import { useCallback, useEffect, useState } from "react";
import { userFetch } from "@/src/lib/api";

/** GET/PUT /api/translate/languages yanıtı (backend stream_settings sözleşmesi). */
type LanguageOption = {
  code: string;
  name: string;
  supports_formality: boolean;
  enabled: boolean;
};

type LanguagesResponse = {
  languages: LanguageOption[];
  max_languages: number;
  source_lang: string;
  target_langs: string[];
};

type Labels = {
  title: string;
  subtitle: string;
  unlimitedNote: string;
  loading: string;
  sourceTitle: string;
  autoDetectLabel: string;
  autoDetectHint: string;
  manualLabel: string;
  selectedCountTemplate: string;
  noFormalityNote: string;
  saveLabel: string;
  savingLabel: string;
  savedLabel: string;
  limitReachedNote: string;
  errors: {
    unauthorized: string;
    network: string;
    saveFailed: string;
    tooMany: string;
    empty: string;
  };
};

/** "{max}" / "{count}" yer tutucularını doldurur. */
function fill(template: string, values: Record<string, string>): string {
  return Object.entries(values).reduce(
    (acc, [key, value]) => acc.replaceAll(`{${key}}`, value),
    template,
  );
}

export function TargetLanguagePicker({ labels }: { labels: Labels }) {
  const [data, setData] = useState<LanguagesResponse | null>(null);
  const [selected, setSelected] = useState<string[]>([]);
  const [sourceLang, setSourceLang] = useState("auto");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  const load = useCallback(async () => {
    setError(null);
    try {
      const res = await userFetch("/api/translate/languages");
      if (res.status === 401) {
        setError(labels.errors.unauthorized);
        return;
      }
      if (!res.ok) {
        setError(labels.errors.network);
        return;
      }
      const body = (await res.json()) as LanguagesResponse;
      setData(body);
      setSelected(body.target_langs);
      setSourceLang(body.source_lang);
    } catch {
      setError(labels.errors.network);
    }
  }, [labels.errors.network, labels.errors.unauthorized]);

  useEffect(() => {
    void load();
  }, [load]);

  const maxLanguages = data?.max_languages ?? 1;
  const unlimited = maxLanguages < 0;
  const atLimit = !unlimited && selected.length >= maxLanguages;

  const toggle = useCallback(
    (code: string) => {
      setSaved(false);
      setSelected((prev) => {
        if (prev.includes(code)) {
          return prev.filter((c) => c !== code);
        }
        if (!unlimited && prev.length >= maxLanguages) {
          return prev;
        }
        return [...prev, code];
      });
    },
    [maxLanguages, unlimited],
  );

  const selectSource = useCallback((code: string) => {
    setSourceLang(code);
    setSaved(false);
  }, []);

  const save = useCallback(async () => {
    setError(null);
    setSaved(false);
    if (selected.length === 0) {
      setError(labels.errors.empty);
      return;
    }
    setSaving(true);
    try {
      const res = await userFetch("/api/translate/languages", {
        method: "PUT",
        body: JSON.stringify({ source_lang: sourceLang, target_langs: selected }),
      });
      if (res.status === 401) {
        setError(labels.errors.unauthorized);
      } else if (res.status === 403) {
        setError(labels.errors.tooMany);
      } else if (!res.ok) {
        setError(labels.errors.saveFailed);
      } else {
        setSaved(true);
      }
    } catch {
      setError(labels.errors.network);
    } finally {
      setSaving(false);
    }
  }, [labels.errors, selected, sourceLang]);

  if (!data && !error) {
    return (
      <section className="rounded-xl border border-border bg-white p-5 sm:p-6">
        <h2 className="text-lg font-semibold text-ink">{labels.title}</h2>
        <p className="mt-2 text-sm text-muted">{labels.loading}</p>
      </section>
    );
  }

  const enabledLanguages = data?.languages.filter((lang) => lang.enabled) ?? [];

  return (
    <section className="space-y-4 rounded-xl border border-border bg-white p-5 sm:p-6">
      <div>
        <h2 className="text-lg font-semibold text-ink">{labels.title}</h2>
        <p className="mt-1 text-sm leading-6 text-muted">
          {unlimited ? labels.unlimitedNote : fill(labels.subtitle, { max: String(maxLanguages) })}
        </p>
      </div>

      {error ? (
        <p className="rounded-lg border border-danger/25 bg-danger/10 px-4 py-3 text-sm text-ink" role="alert">
          {error}
        </p>
      ) : null}

      <div>
        <p className="text-xs font-semibold uppercase tracking-wide text-muted">{labels.sourceTitle}</p>
        <div className="mt-2 inline-flex flex-wrap gap-1 rounded-xl border border-border bg-canvas p-1">
          <button
            className={`rounded-lg px-4 py-2 text-sm font-semibold transition ${
              sourceLang === "auto" ? "bg-ink text-white" : "text-text hover:bg-white"
            }`}
            onClick={() => selectSource("auto")}
            type="button"
          >
            {labels.autoDetectLabel}
          </button>
          {enabledLanguages.map((lang) => (
            <button
              className={`rounded-lg px-4 py-2 text-sm font-semibold transition ${
                sourceLang === lang.code ? "bg-ink text-white" : "text-text hover:bg-white"
              }`}
              key={lang.code}
              onClick={() => selectSource(lang.code)}
              type="button"
            >
              {lang.name}
            </button>
          ))}
        </div>
        {sourceLang === "auto" ? <p className="mt-2 text-xs text-muted">{labels.autoDetectHint}</p> : null}
      </div>

      <div>
        <div className="flex items-center justify-between">
          <p className="text-xs font-semibold uppercase tracking-wide text-muted">{labels.manualLabel}</p>
          {!unlimited ? (
            <span className="text-xs text-muted">
              {fill(labels.selectedCountTemplate, {
                count: String(selected.length),
                max: String(maxLanguages),
              })}
            </span>
          ) : null}
        </div>
        <ul className="mt-2 space-y-2">
          {enabledLanguages.map((lang) => {
            const checked = selected.includes(lang.code);
            const disabled = !checked && atLimit;
            return (
              <li
                className="flex items-center justify-between rounded-lg border border-hairline bg-canvas px-4 py-3"
                key={lang.code}
              >
                <label
                  className={`flex items-center gap-3 text-sm font-semibold text-ink ${disabled ? "opacity-50" : ""}`}
                >
                  <input
                    checked={checked}
                    disabled={disabled}
                    onChange={() => toggle(lang.code)}
                    type="checkbox"
                  />
                  <span className="font-mono text-xs text-muted">{lang.code.toUpperCase()}</span>
                  {lang.name}
                </label>
                {!lang.supports_formality ? (
                  <span className="text-xs text-muted">{labels.noFormalityNote}</span>
                ) : null}
              </li>
            );
          })}
        </ul>
      </div>

      {atLimit ? <p className="text-xs text-muted">{labels.limitReachedNote}</p> : null}

      <button
        className="inline-flex h-9 items-center rounded-full bg-ink px-4 text-sm font-semibold text-white transition hover:bg-text disabled:opacity-50"
        disabled={saving}
        onClick={() => void save()}
        type="button"
      >
        {saving ? labels.savingLabel : saved ? labels.savedLabel : labels.saveLabel}
      </button>
    </section>
  );
}
