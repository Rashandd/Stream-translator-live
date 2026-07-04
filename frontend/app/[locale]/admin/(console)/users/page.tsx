import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { Icon } from "@/src/components/icon";
import { PageHeader } from "@/src/components/dashboard-ui";
import { getPublicConfig } from "@/src/lib/config";
import { isLocale, locales, type Locale } from "@/src/i18n/routing";

type Props = { params: Promise<{ locale: string }> };

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) return {};
  const config = await getPublicConfig(rawLocale);
  return { title: config.admin.users.title };
}

const statusTone: Record<string, string> = {
  active: "border-success/25 bg-success/10",
  suspended: "border-danger/25 bg-danger/10",
};

export default async function AdminUsersPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { users } = config.admin;

  return (
    <div className="space-y-6">
      <PageHeader subtitle={users.subtitle} title={users.title} />

      <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
        <div className="flex flex-1 items-center gap-2 rounded-lg border border-border bg-white px-3 py-2">
          <Icon className="h-4 w-4 text-muted" name="Settings" />
          <input
            className="min-w-0 flex-1 bg-transparent text-sm text-ink outline-none placeholder:text-faint"
            placeholder={users.searchPlaceholder}
            type="search"
          />
        </div>
        <div className="flex flex-wrap gap-2">
          {users.filters.map((filter) => (
            <span className="rounded-full border border-border bg-white px-3 py-1.5 text-xs font-semibold text-muted" key={filter}>
              {filter}
            </span>
          ))}
        </div>
      </div>

      <div className="overflow-hidden rounded-xl border border-border bg-white">
        <div className="overflow-x-auto">
          <table className="w-full min-w-[640px] text-left text-sm">
            <thead>
              <tr className="border-b border-hairline text-xs uppercase tracking-wide text-faint">
                {users.columns.map((column) => (
                  <th className="px-4 py-3 font-semibold" key={column}>{column}</th>
                ))}
                <th className="px-4 py-3" />
              </tr>
            </thead>
            <tbody>
              {users.rows.map((row) => (
                <tr className="border-b border-hairline last:border-0" key={row.name}>
                  <td className="px-4 py-3 font-semibold text-ink">{row.name}</td>
                  <td className="px-4 py-3 text-muted">{row.plan}</td>
                  <td className="px-4 py-3 font-mono text-muted">{row.country}</td>
                  <td className="px-4 py-3">
                    <span className={`inline-flex rounded-full border px-2.5 py-0.5 text-xs font-semibold text-ink ${statusTone[row.status] ?? "border-border bg-canvas"}`}>
                      {row.status}
                    </span>
                  </td>
                  <td className="px-4 py-3 font-mono text-muted">{row.usage}</td>
                  <td className="px-4 py-3 text-right">
                    <button className="text-sm font-semibold text-blue transition hover:text-ink" type="button">
                      {users.manageLabel}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>

      <p className="text-sm leading-6 text-muted">{users.actionsNote}</p>
    </div>
  );
}
