import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { Icon } from "@/src/components/icon";
import { GoLivePanel } from "@/src/components/go-live-panel";
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
  return { title: config.dashboard.goLive.title };
}

export default async function GoLivePage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { goLive } = config.dashboard;
  const isCloud = goLive.activeMode === "cloud";
  const mode = isCloud ? goLive.modes.cloud : goLive.modes.relay;

  return (
    <div className="space-y-6">
      <PageHeader subtitle={goLive.subtitle} title={goLive.title} />

      <div className="flex flex-col gap-4 rounded-xl border border-border bg-surface p-5 sm:flex-row sm:items-start sm:justify-between">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <p className="text-xs font-semibold uppercase tracking-wide text-blue">{goLive.modeLabel}</p>
            <span className="rounded-full border border-cyan/25 bg-cyan/10 px-2.5 py-0.5 text-xs font-semibold text-ink">{mode.badge}</span>
          </div>
          <h2 className="mt-2 text-lg font-semibold text-ink">{mode.name}</h2>
          <p className="mt-1 max-w-2xl text-sm leading-6 text-muted">{mode.summary}</p>
        </div>
        <Link
          className="inline-flex h-9 shrink-0 items-center gap-1.5 rounded-full border border-border bg-surface px-4 text-sm font-semibold text-ink transition hover:border-cyan hover:bg-cyan/10"
          href={localizePath(locale, "/dashboard/distribution")}
        >
          <Icon className="h-4 w-4" name="Route" />
          {goLive.changeModeLabel}
        </Link>
      </div>

      <GoLivePanel
        isCloud={isCloud}
        labels={{
          stepsTitle: goLive.stepsTitle,
          startTitle: goLive.startTitle,
          targetsLabel: goLive.targetsLabel,
          targets: goLive.targets,
          copyLabel: goLive.copyLabel,
        }}
        mode={mode}
      />
    </div>
  );
}
