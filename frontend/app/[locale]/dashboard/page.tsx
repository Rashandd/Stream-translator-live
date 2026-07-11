import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { FoundingApplicationPanel } from "@/src/components/founding-application-panel";
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

export async function generateMetadata({ params }: DashboardPageProps): Promise<Metadata> {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) return {};
  const config = await getPublicConfig(rawLocale);
  return { title: config.dashboard.overview.title };
}

export default async function DashboardOverviewPage({ params }: DashboardPageProps) {
  const { locale: rawLocale } = await params;

  if (!isLocale(rawLocale)) {
    notFound();
  }

  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { overview, founding } = config.dashboard;

  return (
    <div className="space-y-6">
      <PageHeader subtitle={overview.subtitle} title={overview.title} />

      <FoundingApplicationPanel labels={founding} />

      <SectionCard subtitle={overview.howItWorks.subtitle} title={overview.howItWorks.title}>
        <ol className="grid gap-4 sm:grid-cols-3">
          {overview.howItWorks.steps.map((step, index) => (
            <li className="rounded-lg border border-hairline bg-canvas p-4" key={step.title}>
              <div className="flex items-center gap-2">
                <span className="flex h-7 w-7 items-center justify-center rounded-full bg-primary text-xs font-semibold text-on-primary">
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

      <SectionCard title={overview.quickStart.title}>
        <div className="mb-4">
          <p className="text-xs font-medium text-muted">{overview.quickStart.streamKeyLabel}</p>
          <div className="mt-1 flex items-center gap-2 rounded-lg border border-border bg-canvas px-3 py-2">
            <code className="min-w-0 flex-1 truncate font-mono text-sm text-ink">{overview.quickStart.streamKey}</code>
            <span className="inline-flex items-center gap-1.5 rounded-md border border-border bg-surface px-2.5 py-1 text-xs font-semibold text-ink">
              <Icon className="h-3.5 w-3.5" name="Copy" />
              {overview.quickStart.copyLabel}
            </span>
          </div>
        </div>
        <div className="grid gap-2 sm:grid-cols-3">
          {overview.quickStart.actions.map((action) => (
            <Link
              className="flex items-center gap-3 rounded-lg border border-hairline bg-canvas p-3 text-left transition hover:border-cyan hover:bg-cyan/10"
              href={localizePath(locale, action.href)}
              key={action.label}
            >
              <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-surface text-ink">
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
  );
}
