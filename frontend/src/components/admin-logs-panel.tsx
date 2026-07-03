"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { adminFetch } from "@/src/lib/api";
import { Icon } from "@/src/components/icon";

type LogRow = {
  time: string;
  level: string;
  service: string;
  message: string;
  user: string;
  session: string;
};

type ApiLogEntry = {
  ts_ms: number;
  level: string;
  service: string;
  message: string;
  user_id: string | null;
  session_id: string | null;
};

type Labels = {
  columns: string[];
  levelFilters: string[];
  serviceFilters: string[];
  searchPlaceholder: string;
  recentErrors: { title: string; subtitle: string };
  empty: string;
  liveBadge: string;
  fallbackBadge: string;
  errors: { unauthorized: string };
  signInLabel: string;
};

type Props = {
  labels: Labels;
  /** API'ye ulaşılamadığında gösterilen sözlük örnek satırları. */
  fallbackRows: LogRow[];
  loginHref: string;
};

const levelTone: Record<string, string> = {
  error: "border-danger/25 bg-danger/10",
  warn: "border-warning/25 bg-warning/10",
  info: "border-border bg-canvas",
  debug: "border-border bg-canvas",
};

function toRow(entry: ApiLogEntry): LogRow {
  return {
    time: new Date(entry.ts_ms).toISOString().slice(11, 19),
    level: entry.level,
    service: entry.service,
    message: entry.message,
    user: entry.user_id ?? "—",
    session: entry.session_id ?? "—",
  };
}

export function AdminLogsPanel({ labels, fallbackRows, loginHref }: Props) {
  const [rows, setRows] = useState<LogRow[] | null>(null);
  const [live, setLive] = useState(false);
  const [unauthorized, setUnauthorized] = useState(false);
  const [level, setLevel] = useState<string | null>(null);
  const [service, setService] = useState<string | null>(null);
  const [query, setQuery] = useState("");

  const load = useCallback(async () => {
    const params = new URLSearchParams({ limit: "200" });
    if (level) {
      params.set("level", level);
    }
    if (service) {
      params.set("service", service);
    }
    try {
      const response = await adminFetch(`/admin/logs?${params.toString()}`);
      if (response.status === 401 || response.status === 403) {
        setUnauthorized(true);
        setRows([]);
        return;
      }
      if (!response.ok) {
        throw new Error(String(response.status));
      }
      const entries = (await response.json()) as ApiLogEntry[];
      setUnauthorized(false);
      setLive(true);
      setRows(entries.map(toRow));
    } catch {
      // API yoksa sözlük örnek satırlarına düş (geliştirme/önizleme).
      setLive(false);
      setRows(fallbackRows);
    }
  }, [level, service, fallbackRows]);

  useEffect(() => {
    void load();
  }, [load]);

  const visible = useMemo(() => {
    const base = rows ?? [];
    const needle = query.trim().toLowerCase();
    return base.filter((row) => {
      if (needle && !row.message.toLowerCase().includes(needle)) {
        return false;
      }
      // Canlı modda filtre API'de uygulanır; fallback'te client'ta uygula.
      if (!live && level && row.level !== level) {
        return false;
      }
      if (!live && service && row.service !== service) {
        return false;
      }
      return true;
    });
  }, [rows, query, live, level, service]);

  const recentErrors = useMemo(
    () => (rows ?? []).filter((row) => row.level === "error").slice(0, 5),
    [rows],
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

  const chip = (active: boolean) =>
    `rounded-full border px-3 py-1.5 text-xs font-semibold transition ${
      active ? "border-blue/40 bg-blue/10 text-ink" : "border-border bg-white text-muted hover:text-ink"
    }`;

  return (
    <div className="space-y-6">
      <section className="rounded-xl border border-border bg-white p-4">
        <div className="flex items-center gap-2">
          <Icon className="h-4 w-4 text-danger" name="ShieldCheck" />
          <h2 className="text-sm font-semibold text-ink">{labels.recentErrors.title}</h2>
          <span className="ml-auto rounded-full border border-border bg-canvas px-2.5 py-0.5 text-xs font-semibold text-muted">
            {live ? labels.liveBadge : labels.fallbackBadge}
          </span>
        </div>
        <p className="mt-1 text-xs leading-5 text-muted">{labels.recentErrors.subtitle}</p>
        <ul className="mt-3 space-y-2">
          {recentErrors.map((row) => (
            <li className="flex flex-wrap items-center gap-2 text-sm" key={`${row.time}-${row.message}`}>
              <span className="font-mono text-xs text-faint">{row.time}</span>
              <span className="rounded-full border border-danger/25 bg-danger/10 px-2 py-0.5 text-xs font-semibold text-ink">
                {row.service}
              </span>
              <span className="min-w-0 flex-1 truncate text-muted">{row.message}</span>
            </li>
          ))}
          {recentErrors.length === 0 ? <li className="text-sm text-muted">{labels.empty}</li> : null}
        </ul>
      </section>

      <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
        <div className="flex flex-1 items-center gap-2 rounded-lg border border-border bg-white px-3 py-2">
          <Icon className="h-4 w-4 text-muted" name="Settings" />
          <input
            className="min-w-0 flex-1 bg-transparent text-sm text-ink outline-none placeholder:text-faint"
            onChange={(event) => setQuery(event.target.value)}
            placeholder={labels.searchPlaceholder}
            type="search"
            value={query}
          />
        </div>
        <div className="flex flex-wrap gap-2">
          {labels.levelFilters.map((filter) => (
            <button
              className={chip(level === filter)}
              key={filter}
              onClick={() => setLevel(level === filter ? null : filter)}
              type="button"
            >
              {filter}
            </button>
          ))}
        </div>
      </div>

      <div className="flex flex-wrap gap-2">
        {labels.serviceFilters.map((filter) => (
          <button
            className={chip(service === filter)}
            key={filter}
            onClick={() => setService(service === filter ? null : filter)}
            type="button"
          >
            {filter}
          </button>
        ))}
      </div>

      <div className="overflow-hidden rounded-xl border border-border bg-white">
        <div className="overflow-x-auto">
          <table className="w-full min-w-[720px] text-left text-sm">
            <thead>
              <tr className="border-b border-hairline text-xs uppercase tracking-wide text-faint">
                {labels.columns.map((column) => (
                  <th className="px-4 py-3 font-semibold" key={column}>{column}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {visible.map((row, index) => (
                <tr className="border-b border-hairline last:border-0" key={`${row.time}-${row.service}-${index}`}>
                  <td className="px-4 py-3 font-mono text-muted">{row.time}</td>
                  <td className="px-4 py-3">
                    <span className={`inline-flex rounded-full border px-2.5 py-0.5 text-xs font-semibold text-ink ${levelTone[row.level] ?? "border-border bg-canvas"}`}>
                      {row.level}
                    </span>
                  </td>
                  <td className="px-4 py-3 font-mono text-muted">{row.service}</td>
                  <td className="px-4 py-3 text-muted">{row.message}</td>
                  <td className="px-4 py-3 font-mono text-muted">{row.user}</td>
                  <td className="px-4 py-3 font-mono text-muted">{row.session}</td>
                </tr>
              ))}
              {visible.length === 0 ? (
                <tr>
                  <td className="px-4 py-6 text-sm text-muted" colSpan={labels.columns.length}>
                    {labels.empty}
                  </td>
                </tr>
              ) : null}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
