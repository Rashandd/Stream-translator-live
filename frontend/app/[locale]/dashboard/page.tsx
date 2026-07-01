import Link from "next/link";
import { notFound } from "next/navigation";
import { Icon } from "@/src/components/icon";
import { MetricCard, PageHeader, SectionCard } from "@/src/components/dashboard-ui";
import { getPublicConfig } from "@/src/lib/config";
import { isLocale, localizePath, locales, type Locale } from "@/src/i18n/routing";

type DashboardPageProps = {
  params: Promise<{ locale: string }>;
};

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export default async function DashboardOverviewPage({ params }: DashboardPageProps) {
  const { locale: rawLocale } = await params;

  if (!isLocale(rawLocale)) {
    notFound();
  }

  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { overview } = config.dashboard;
  const totalViewers = overview.languageBreakdown.items.reduce((sum, item) => sum + item.viewers, 0);

  return (
    <div className="space-y-6">
      <PageHeader subtitle={overview.subtitle} title={overview.title} />

      <SectionCard subtitle={overview.howItWorks.subtitle} title={overview.howItWorks.title}>
        <ol className="grid gap-4 sm:grid-cols-3">
          {overview.howItWorks.steps.map((step, index) => (
            <li className="rounded-lg border border-hairline bg-canvas p-4" key={step.title}>
              <div className="flex items-center gap-2">
                <span className="flex h-7 w-7 items-center justify-center rounded-full bg-ink text-xs font-semibold text-white">
                  {index + 1}
                </span>
                <Icon className="h-[18px] w-[18px] text-blue" name={step.icon} />
              </div>
              <h4 className="mt-3 text-sm font-semibold text-ink">{step.title}</h4>
              <p className="mt-1 text-sm leading-6 text-muted">{step.body}</p>
            </li>
          ))}
        </ol>
        <p className="mt-4 text-sm leading-6 text-muted">{overview.howItWorks.note}</p>
      </SectionCard>

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {overview.metrics.map((metric) => (
          <MetricCard hint={metric.hint} icon={metric.icon} key={metric.label} label={metric.label} value={metric.value} />
        ))}
      </div>

      <div className="grid gap-6 lg:grid-cols-[1fr_0.9fr]">
        <SectionCard subtitle={overview.languageBreakdown.subtitle} title={overview.languageBreakdown.title}>
          {totalViewers === 0 ? (
            <p className="rounded-lg border border-dashed border-border bg-canvas px-4 py-6 text-center text-sm text-muted">
              {overview.languageBreakdown.empty}
            </p>
          ) : (
            <ul className="space-y-3">
              {overview.languageBreakdown.items.map((item) => {
                const pct = Math.round((item.viewers / totalViewers) * 100);
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
          )}
        </SectionCard>

        <SectionCard title={overview.quickStart.title}>
          <div className="mb-4">
            <p className="text-xs font-medium text-muted">{overview.quickStart.streamKeyLabel}</p>
            <div className="mt-1 flex items-center gap-2 rounded-lg border border-border bg-canvas px-3 py-2">
              <code className="min-w-0 flex-1 truncate font-mono text-sm text-ink">{overview.quickStart.streamKey}</code>
              <span className="inline-flex items-center gap-1.5 rounded-md border border-border bg-white px-2.5 py-1 text-xs font-semibold text-ink">
                <Icon className="h-3.5 w-3.5" name="Copy" />
                {overview.quickStart.copyLabel}
              </span>
            </div>
          </div>
          <div className="space-y-2">
            {overview.quickStart.actions.map((action) => (
              <Link
                className="flex items-center gap-3 rounded-lg border border-hairline bg-canvas p-3 text-left transition hover:border-cyan hover:bg-cyan/10"
                href={localizePath(locale, action.href)}
                key={action.label}
              >
                <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-white text-ink">
                  <Icon className="h-[18px] w-[18px]" name={action.icon} />
                </span>
                <span className="min-w-0">
                  <span className="block text-sm font-semibold text-ink">{action.label}</span>
                  <span className="mt-0.5 block text-sm leading-5 text-muted">{action.description}</span>
                </span>
              </Link>
            ))}
          </div>
          <p className="mt-4 text-sm leading-6 text-muted">{overview.quickStart.note}</p>
        </SectionCard>
      </div>
    </div>
  );
}
