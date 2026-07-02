import { notFound } from "next/navigation";
import { Icon } from "@/src/components/icon";
import { PageHeader } from "@/src/components/dashboard-ui";
import { getPublicConfig } from "@/src/lib/config";
import { isLocale, locales, type Locale } from "@/src/i18n/routing";

type Props = { params: Promise<{ locale: string }> };

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

const levelTone: Record<string, string> = {
  error: "border-danger/25 bg-danger/10",
  warn: "border-warning/25 bg-warning/10",
  info: "border-border bg-canvas",
  debug: "border-border bg-canvas",
};

export default async function AdminLogsPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { logs } = config.admin;
  const recentErrorRows = logs.rows.filter((row) => row.level === "error");

  return (
    <div className="space-y-6">
      <PageHeader subtitle={logs.subtitle} title={logs.title} />

      <section className="rounded-xl border border-border bg-white p-4">
        <div className="flex items-center gap-2">
          <Icon className="h-4 w-4 text-danger" name="ShieldCheck" />
          <h2 className="text-sm font-semibold text-ink">{logs.recentErrors.title}</h2>
        </div>
        <p className="mt-1 text-xs leading-5 text-muted">{logs.recentErrors.subtitle}</p>
        <ul className="mt-3 space-y-2">
          {recentErrorRows.map((row) => (
            <li className="flex flex-wrap items-center gap-2 text-sm" key={`${row.time}-${row.message}`}>
              <span className="font-mono text-xs text-faint">{row.time}</span>
              <span className="rounded-full border border-danger/25 bg-danger/10 px-2 py-0.5 text-xs font-semibold text-ink">
                {row.service}
              </span>
              <span className="min-w-0 flex-1 truncate text-muted">{row.message}</span>
            </li>
          ))}
          {recentErrorRows.length === 0 ? (
            <li className="text-sm text-muted">{logs.empty}</li>
          ) : null}
        </ul>
      </section>

      <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
        <div className="flex flex-1 items-center gap-2 rounded-lg border border-border bg-white px-3 py-2">
          <Icon className="h-4 w-4 text-muted" name="Settings" />
          <input
            className="min-w-0 flex-1 bg-transparent text-sm text-ink outline-none placeholder:text-faint"
            placeholder={logs.searchPlaceholder}
            type="search"
          />
        </div>
        <div className="flex flex-wrap gap-2">
          {logs.levelFilters.map((filter) => (
            <span className="rounded-full border border-border bg-white px-3 py-1.5 text-xs font-semibold text-muted" key={filter}>
              {filter}
            </span>
          ))}
        </div>
      </div>

      <div className="flex flex-wrap gap-2">
        {logs.serviceFilters.map((filter) => (
          <span className="rounded-full border border-border bg-white px-3 py-1.5 text-xs font-semibold text-muted" key={filter}>
            {filter}
          </span>
        ))}
      </div>

      <div className="overflow-hidden rounded-xl border border-border bg-white">
        <div className="overflow-x-auto">
          <table className="w-full min-w-[720px] text-left text-sm">
            <thead>
              <tr className="border-b border-hairline text-xs uppercase tracking-wide text-faint">
                {logs.columns.map((column) => (
                  <th className="px-4 py-3 font-semibold" key={column}>{column}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {logs.rows.map((row) => (
                <tr className="border-b border-hairline last:border-0" key={`${row.time}-${row.service}-${row.message}`}>
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
            </tbody>
          </table>
        </div>
      </div>

      <p className="text-sm leading-6 text-muted">{logs.levelNote}</p>
      <p className="text-sm leading-6 text-muted">{logs.note}</p>
    </div>
  );
}
