import { notFound } from "next/navigation";
import { Icon } from "@/src/components/icon";
import { PageHeader, SectionCard } from "@/src/components/dashboard-ui";
import { StatusPill } from "@/src/components/status-pill";
import { getPublicConfig } from "@/src/lib/config";
import { isLocale, locales, type Locale } from "@/src/i18n/routing";

type Props = { params: Promise<{ locale: string }> };

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
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

      <div className="grid gap-4 lg:grid-cols-3">
        {distribution.modes.map((mode) => {
          const selected = mode.id === distribution.selectedMode;
          return (
            <article
              className={`relative rounded-xl border bg-white p-5 ${selected ? "border-ink ring-1 ring-ink/20" : "border-border"}`}
              key={mode.id}
            >
              <div className="flex items-center justify-between">
                <Icon className="h-6 w-6 text-blue" name={mode.icon} />
                {selected ? (
                  <span className="inline-flex items-center gap-1 rounded-full bg-ink px-2.5 py-0.5 text-xs font-semibold text-white">
                    <Icon className="h-3 w-3" name="CheckCircle2" />
                    {distribution.selectedLabel}
                  </span>
                ) : (
                  <StatusPill label={mode.status} text={config.statusLabels[mode.status as keyof typeof config.statusLabels]} />
                )}
              </div>
              <h3 className="mt-4 text-lg font-semibold text-ink">{mode.title}</h3>
              <p className="mt-2 text-sm leading-6 text-muted">{mode.body}</p>
            </article>
          );
        })}
      </div>

      <SectionCard subtitle={distribution.bandwidthNotice} title={distribution.servicesTitle}>
        <ul className="divide-y divide-hairline">
          {distribution.services.map((service) => (
            <li className="flex items-center justify-between gap-4 py-3 first:pt-0 last:pb-0" key={service.name}>
              <div className="flex items-center gap-3">
                <span className="flex h-10 w-10 items-center justify-center rounded-full bg-canvas text-ink">
                  <Icon className="h-[18px] w-[18px]" name={service.icon} />
                </span>
                <div>
                  <p className="text-sm font-semibold text-ink">{service.name}</p>
                  <p className="text-sm text-muted">{service.status}</p>
                </div>
              </div>
              <button
                className="inline-flex h-9 items-center rounded-full border border-border bg-white px-4 text-sm font-semibold text-ink transition hover:border-cyan hover:bg-cyan/10"
                type="button"
              >
                {service.action}
              </button>
            </li>
          ))}
        </ul>
      </SectionCard>
    </div>
  );
}
