import { notFound } from "next/navigation";
import { MetricCard, PageHeader } from "@/src/components/dashboard-ui";
import { getPublicConfig } from "@/src/lib/config";
import { isLocale, locales, type Locale } from "@/src/i18n/routing";

type Props = { params: Promise<{ locale: string }> };

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export default async function AdminCostsPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { costs } = config.admin;

  return (
    <div className="space-y-6">
      <PageHeader subtitle={costs.subtitle} title={costs.title} />
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {costs.metrics.map((metric) => (
          <MetricCard hint={metric.hint} icon={metric.icon} key={metric.label} label={metric.label} value={metric.value} />
        ))}
      </div>
      <p className="text-sm leading-6 text-muted">{costs.note}</p>
    </div>
  );
}
