"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { SectionCard } from "@/src/components/dashboard-ui";
import { adminFetch } from "@/src/lib/api";

type LegalSection = {
  title: string;
  body: string;
};

type LegalPage = {
  slug: string;
  title: string;
  summary: string;
  sections: LegalSection[];
};

type Labels = {
  editorTitle: string;
  editorSubtitle: string;
  selectLabel: string;
  htmlLabel: string;
  previewTitle: string;
  saveLabel: string;
  savingLabel: string;
  savedLabel: string;
  clearLabel: string;
  loading: string;
  empty: string;
  fallbackBadge: string;
  customBadge: string;
  helper: string;
  errors: {
    unauthorized: string;
    network: string;
    saveFailed: string;
  };
  signInLabel: string;
};

type Props = {
  labels: Labels;
  loginHref: string;
  pages: LegalPage[];
};

function htmlEscape(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function fallbackHtml(page: LegalPage): string {
  const sections = page.sections
    .map(
      (section) =>
        `<h2>${htmlEscape(section.title)}</h2>\n<p>${htmlEscape(section.body)}</p>`,
    )
    .join("\n\n");
  return `<h1>${htmlEscape(page.title)}</h1>\n<p>${htmlEscape(page.summary)}</p>\n\n${sections}`;
}

function configKey(slug: string): string {
  return `legal.${slug}.html`;
}

export function AdminLegalPanel({ labels, loginHref, pages }: Props) {
  const [selectedSlug, setSelectedSlug] = useState(pages[0]?.slug ?? "");
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [runtimeConfig, setRuntimeConfig] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [unauthorized, setUnauthorized] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const selectedPage = useMemo(
    () => pages.find((page) => page.slug === selectedSlug) ?? pages[0],
    [pages, selectedSlug],
  );

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await adminFetch("/config");
      if (!response.ok) {
        setError(labels.errors.network);
        return;
      }
      const config = (await response.json()) as Record<string, string>;
      setRuntimeConfig(config);
      setDrafts(
        Object.fromEntries(
          pages.map((page) => {
            const saved = config[configKey(page.slug)];
            return [page.slug, saved?.trim() ? saved : fallbackHtml(page)];
          }),
        ),
      );
    } catch {
      setError(labels.errors.network);
    } finally {
      setLoading(false);
    }
  }, [labels.errors.network, pages]);

  useEffect(() => {
    void load();
  }, [load]);

  const selectedDraft = selectedPage ? drafts[selectedPage.slug] ?? "" : "";
  const selectedFallback = selectedPage ? fallbackHtml(selectedPage) : "";
  const previewHtml = selectedDraft.trim() ? selectedDraft : selectedFallback;
  const isCustom = Boolean(selectedPage && runtimeConfig[configKey(selectedPage.slug)]?.trim());

  async function save(value: string) {
    if (!selectedPage) return;
    setSaving(true);
    setSaved(false);
    setError(null);
    try {
      const response = await adminFetch("/admin/config", {
        method: "PUT",
        body: JSON.stringify({
          key: `legal.${selectedPage.slug}.html`,
          value,
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
      setDrafts((current) => ({
        ...current,
        [selectedPage.slug]: value.trim() ? value : fallbackHtml(selectedPage),
      }));
      setSaved(true);
    } catch {
      setError(labels.errors.network);
    } finally {
      setSaving(false);
    }
  }

  if (loading) {
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

  if (!selectedPage) {
    return <p className="text-sm text-muted">{labels.empty}</p>;
  }

  return (
    <div className="space-y-6">
      {error ? (
        <p className="rounded-lg border border-danger/25 bg-danger/10 px-3 py-2 text-sm text-ink" role="alert">
          {error}
        </p>
      ) : null}

      <SectionCard title={labels.editorTitle} subtitle={labels.editorSubtitle}>
        <div className="grid gap-5 lg:grid-cols-[260px_1fr]">
          <div>
            <label className="text-sm font-semibold text-ink" htmlFor="legal-page">
              {labels.selectLabel}
            </label>
            <select
              className="mt-2 h-11 w-full rounded-lg border border-border bg-white px-3 text-sm text-ink outline-none transition focus:border-blue"
              id="legal-page"
              onChange={(event) => {
                setSelectedSlug(event.target.value);
                setSaved(false);
              }}
              value={selectedPage.slug}
            >
              {pages.map((page) => (
                <option key={page.slug} value={page.slug}>
                  {page.title}
                </option>
              ))}
            </select>

            <p className="mt-4 text-xs leading-5 text-muted">{labels.helper}</p>
            <span className="mt-4 inline-flex rounded-full border border-border px-2.5 py-0.5 text-xs font-semibold text-ink">
              {isCustom ? labels.customBadge : labels.fallbackBadge}
            </span>
          </div>

          <div>
            <label className="text-sm font-semibold text-ink" htmlFor="legal-html">
              {labels.htmlLabel}
            </label>
            <textarea
              className="mt-2 min-h-[340px] w-full rounded-lg border border-border bg-white p-3 font-mono text-sm leading-6 text-ink outline-none transition focus:border-blue"
              id="legal-html"
              onChange={(event) => {
                setDrafts((current) => ({ ...current, [selectedPage.slug]: event.target.value }));
                setSaved(false);
              }}
              spellCheck={false}
              value={selectedDraft}
            />

            <div className="mt-4 flex flex-wrap gap-2">
              <button
                className="inline-flex h-9 items-center rounded-full bg-ink px-4 text-sm font-semibold text-white transition hover:bg-text disabled:opacity-50"
                disabled={saving}
                onClick={() => void save(selectedDraft)}
                type="button"
              >
                {saving ? labels.savingLabel : saved ? labels.savedLabel : labels.saveLabel}
              </button>
              <button
                className="inline-flex h-9 items-center rounded-full border border-border bg-white px-4 text-sm font-semibold text-ink transition hover:border-cyan hover:bg-cyan/10 disabled:opacity-50"
                disabled={saving}
                onClick={() => void save("")}
                type="button"
              >
                {labels.clearLabel}
              </button>
            </div>
          </div>
        </div>
      </SectionCard>

      <SectionCard title={labels.previewTitle}>
        <div className="legal-html" dangerouslySetInnerHTML={{ __html: previewHtml }} />
      </SectionCard>
    </div>
  );
}
