import { notFound } from "next/navigation";
import { Icon } from "@/src/components/icon";
import { MetricCard, PageHeader, SectionCard } from "@/src/components/dashboard-ui";
import { getPublicConfig } from "@/src/lib/config";
import { isLocale, locales, type Locale } from "@/src/i18n/routing";

type Props = { params: Promise<{ locale: string }> };

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export default async function AnalyticsPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { analytics } = config.dashboard;

  return (
    <div className="space-y-6">
      <PageHeader subtitle={analytics.subtitle} title={analytics.title} />

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {analytics.metrics.map((metric) => (
          <MetricCard hint={metric.hint} icon={metric.icon} key={metric.label} label={metric.label} value={metric.value} />
        ))}
      </div>

      <SectionCard title={analytics.byStreamTitle}>
        <p className="rounded-lg border border-dashed border-border bg-canvas px-4 py-10 text-center text-sm text-muted">
          {analytics.byStreamEmpty}
        </p>
      </SectionCard>

      <div className="flex items-start gap-3 rounded-xl border border-border bg-white p-4">
        <Icon className="mt-0.5 h-5 w-5 shrink-0 text-blue" name="Mail" />
        <p className="text-sm leading-6 text-muted">{analytics.emailNote}</p>
      </div>
    </div>
  );
}
