import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { AdminLogsPanel } from "@/src/components/admin-logs-panel";
import { PageHeader } from "@/src/components/dashboard-ui";
import { getPublicConfig } from "@/src/lib/config";
import { isLocale, localizePath, locales, type Locale } from "@/src/i18n/routing";

type Props = { params: Promise<{ locale: string }> };

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) return {};
  const config = await getPublicConfig(rawLocale);
  return { title: config.admin.logs.title };
}

export default async function AdminLogsPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { logs } = config.admin;

  return (
    <div className="space-y-6">
      <PageHeader subtitle={logs.subtitle} title={logs.title} />

      <AdminLogsPanel
        fallbackRows={logs.rows}
        labels={{
          columns: logs.columns,
          levelFilters: logs.levelFilters,
          serviceFilters: logs.serviceFilters,
          searchPlaceholder: logs.searchPlaceholder,
          recentErrors: logs.recentErrors,
          empty: logs.empty,
          liveBadge: logs.liveBadge,
          fallbackBadge: logs.fallbackBadge,
          errors: { unauthorized: logs.unauthorized },
          signInLabel: logs.signInLabel,
        }}
        loginHref={localizePath(locale, "/admin/login")}
      />

      <p className="text-sm leading-6 text-muted">{logs.levelNote}</p>
      <p className="text-sm leading-6 text-muted">{logs.note}</p>
    </div>
  );
}
