import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { BrandMark, brandForPlatform } from "@/src/components/brand-mark";
import { DistributionMethodPanel } from "@/src/components/distribution-method-panel";
import { Icon } from "@/src/components/icon";
import { PageHeader } from "@/src/components/dashboard-ui";
import { StatusPill } from "@/src/components/status-pill";
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
  return { title: config.dashboard.distribution.title };
}

export default async function DistributionPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { distribution } = config.dashboard;

  return (
    <div className="space-y-6">
      <PageHeader subtitle={distribution.subtitle} title={distribution.title} />

      <div className="flex flex-col gap-3 rounded-xl border border-border bg-white p-4 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex min-w-0 items-start gap-3">
          <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-canvas text-ink">
            <Icon className="h-5 w-5" name="MonitorCog" />
          </span>
          <div className="min-w-0">
            <p className="flex items-center gap-2 text-sm font-semibold text-ink">
              {distribution.appStatus.label}
              <span className="inline-flex items-center gap-1.5 rounded-full border border-warning/25 bg-warning/10 px-2 py-0.5 text-xs font-semibold text-ink">
                <span className="h-1.5 w-1.5 rounded-full bg-warning" />
                {distribution.appStatus.value}
              </span>
            </p>
            <p className="mt-1 text-sm leading-6 text-muted">{distribution.appStatus.note}</p>
          </div>
        </div>
        <Link
          className="inline-flex h-9 shrink-0 items-center gap-1.5 rounded-full bg-ink px-4 text-sm font-semibold text-white transition hover:bg-text"
          href={localizePath(locale, distribution.appStatus.ctaHref)}
        >
          <Icon className="h-4 w-4" name="Download" />
          {distribution.appStatus.ctaLabel}
        </Link>
      </div>

      <DistributionMethodPanel labels={distribution.methodSelection} />

      <section className="space-y-4">
        <div>
          <h2 className="text-lg font-semibold text-ink">{distribution.platformSupport.title}</h2>
          <p className="mt-1 text-sm leading-6 text-muted">{distribution.platformSupport.subtitle}</p>
        </div>
        <div className="grid gap-4 sm:grid-cols-3">
          {distribution.platformSupport.rows.map((row) => {
            const brand = brandForPlatform(row.platform);

            return (
              <div className="rounded-xl border border-border bg-white p-5" key={row.platform}>
                <div className="flex items-center gap-2">
                  {brand ? (
                    <BrandMark brand={brand} className="h-5 w-5" />
                  ) : (
                    <Icon className="h-5 w-5 text-blue" name={row.icon} />
                  )}
                  <h3 className="text-base font-semibold text-ink">{row.platform}</h3>
                </div>
                <div className="mt-4 space-y-3">
                  <div>
                    <div className="flex items-center justify-between gap-2">
                      <span className="text-xs font-semibold uppercase tracking-wide text-muted">{distribution.platformSupport.captionsLabel}</span>
                      <StatusPill label={row.captionsStatus} text={config.statusLabels[row.captionsStatus as keyof typeof config.statusLabels]} />
                    </div>
                    <p className="mt-1 text-sm leading-6 text-muted">{row.captions}</p>
                  </div>
                  <div className="border-t border-hairline pt-3">
                    <div className="flex items-center justify-between gap-2">
                      <span className="text-xs font-semibold uppercase tracking-wide text-muted">{distribution.platformSupport.dubbingLabel}</span>
                      <StatusPill label={row.dubbingStatus} text={config.statusLabels[row.dubbingStatus as keyof typeof config.statusLabels]} />
                    </div>
                    <p className="mt-1 text-sm leading-6 text-muted">{row.dubbing}</p>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
        <p className="text-sm leading-6 text-muted">{distribution.platformSupport.note}</p>
      </section>
    </div>
  );
}
