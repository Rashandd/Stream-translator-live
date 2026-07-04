import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { Icon } from "@/src/components/icon";
import { MetricCard, PageHeader, SectionCard } from "@/src/components/dashboard-ui";
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
  return { title: config.dashboard.analytics.title };
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

      <div className="flex items-center gap-2 rounded-lg border border-dashed border-border bg-canvas px-4 py-2.5">
        <Icon className="h-4 w-4 shrink-0 text-faint" name="BarChart3" />
        <p className="text-sm text-muted">{analytics.sampleNote}</p>
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        <SectionCard subtitle={analytics.languageSubtitle} title={analytics.languageTitle}>
          <ul className="space-y-3">
            {analytics.languages.map((item) => {
              const max = Math.max(...analytics.languages.map((l) => l.viewers));
              const pct = max === 0 ? 0 : Math.round((item.viewers / max) * 100);
              return (
                <li key={item.code}>
                  <div className="mb-1 flex items-center justify-between text-sm">
                    <span className="font-semibold text-ink">{item.language}</span>
                    <span className="font-mono text-muted">{item.viewers}</span>
                  </div>
                  <div className="h-2 overflow-hidden rounded-full bg-hairline">
                    <div className="h-full rounded-full bg-blue" style={{ width: `${pct}%` }} />
                  </div>
                </li>
              );
            })}
          </ul>
        </SectionCard>

        <SectionCard subtitle={analytics.platformSubtitle} title={analytics.platformTitle}>
          <ul className="space-y-3">
            {analytics.platforms.map((item) => {
              const max = Math.max(...analytics.platforms.map((p) => p.viewers));
              const pct = max === 0 ? 0 : Math.round((item.viewers / max) * 100);
              return (
                <li key={item.name}>
                  <div className="mb-1 flex items-center justify-between text-sm">
                    <span className="font-semibold text-ink">{item.name}</span>
                    <span className="font-mono text-muted">{item.viewers}</span>
                  </div>
                  <div className="h-2 overflow-hidden rounded-full bg-hairline">
                    <div className="h-full rounded-full bg-violet" style={{ width: `${pct}%` }} />
                  </div>
                </li>
              );
            })}
          </ul>
        </SectionCard>
      </div>

      <div className="flex items-start gap-3 rounded-xl border border-border bg-white p-4">
        <Icon className="mt-0.5 h-5 w-5 shrink-0 text-blue" name="Mail" />
        <p className="text-sm leading-6 text-muted">{analytics.emailNote}</p>
      </div>
    </div>
  );
}
