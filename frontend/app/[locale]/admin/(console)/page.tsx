import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { AdminLiveStatus } from "@/src/components/admin-live-status";
import { MetricCard, PageHeader } from "@/src/components/dashboard-ui";
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
  return { title: config.admin.overview.title };
}

export default async function AdminOverviewPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { overview } = config.admin;

  return (
    <div className="space-y-6">
      <PageHeader subtitle={overview.subtitle} title={overview.title} />
      <AdminLiveStatus labels={overview.live} />
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {overview.metrics.map((metric) => (
          <MetricCard hint={metric.hint} icon={metric.icon} key={metric.label} label={metric.label} value={metric.value} />
        ))}
      </div>
    </div>
  );
}
