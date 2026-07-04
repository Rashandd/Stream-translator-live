import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { PageHeader, SectionCard } from "@/src/components/dashboard-ui";
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
  return { title: config.admin.infrastructure.title };
}

const stateDot: Record<string, string> = {
  healthy: "bg-success",
  degraded: "bg-warning",
  down: "bg-danger",
};

export default async function AdminInfrastructurePage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { infrastructure } = config.admin;

  return (
    <div className="space-y-6">
      <PageHeader subtitle={infrastructure.subtitle} title={infrastructure.title} />

      <div className="grid gap-6 lg:grid-cols-[1fr_0.8fr]">
        <SectionCard title={infrastructure.servicesTitle}>
          <ul className="divide-y divide-hairline">
            {infrastructure.services.map((service) => {
              const running = service.state !== "down";
              return (
                <li className="flex items-center justify-between gap-4 py-3 first:pt-0 last:pb-0" key={service.name}>
                  <div className="flex min-w-0 items-center gap-3">
                    <span className={`h-2 w-2 shrink-0 rounded-full ${stateDot[service.state] ?? "bg-faint"}`} />
                    <div className="min-w-0">
                      <p className="truncate text-sm font-semibold text-ink">{service.name}</p>
                      <p className="truncate text-sm text-muted">{service.detail}</p>
                    </div>
                  </div>
                  <button
                    className={`inline-flex h-8 shrink-0 items-center rounded-full border px-3 text-xs font-semibold transition ${
                      running
                        ? "border-border bg-white text-ink hover:border-danger hover:text-danger"
                        : "border-border bg-white text-ink hover:border-success hover:text-success"
                    }`}
                    type="button"
                  >
                    {running ? infrastructure.stopLabel : infrastructure.startLabel}
                  </button>
                </li>
              );
            })}
          </ul>
        </SectionCard>

        <SectionCard title={infrastructure.resourcesTitle}>
          <ul className="space-y-4">
            {infrastructure.resources.map((resource) => (
              <li key={resource.label}>
                <div className="mb-1 flex items-center justify-between text-sm">
                  <span className="font-semibold text-ink">{resource.label}</span>
                  <span className="font-mono text-muted">{resource.value}%</span>
                </div>
                <div className="h-2 overflow-hidden rounded-full bg-hairline">
                  <div className="h-full rounded-full bg-blue" style={{ width: `${resource.value}%` }} />
                </div>
              </li>
            ))}
          </ul>
        </SectionCard>
      </div>
    </div>
  );
}
