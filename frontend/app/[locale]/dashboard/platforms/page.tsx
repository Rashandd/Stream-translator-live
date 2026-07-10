import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { Icon } from "@/src/components/icon";
import { PageHeader } from "@/src/components/dashboard-ui";
import { PlatformAccountsPanel } from "@/src/components/platform-accounts-panel";
import { TwitchExtensionLink } from "@/src/components/twitch-extension-link";
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
  return { title: config.dashboard.platforms.title };
}

export default async function PlatformsPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { platforms } = config.dashboard;
  const { extension } = platforms;

  return (
    <div className="space-y-6">
      <PageHeader subtitle={platforms.subtitle} title={platforms.title} />

      <PlatformAccountsPanel labels={platforms} />

      <section className="flex flex-col gap-4 rounded-xl border border-border bg-white p-5">
        <div className="flex items-start justify-between gap-3">
          <div className="flex min-w-0 items-center gap-3">
            <span className="flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-canvas text-ink">
              <Icon className="h-5 w-5" name={extension.icon} />
            </span>
            <div className="min-w-0">
              <p className="text-sm font-semibold text-ink">{extension.title}</p>
              <p className="text-sm text-muted">{extension.summary}</p>
            </div>
          </div>
          <span className="inline-flex shrink-0 items-center gap-1.5 rounded-full border border-border bg-canvas px-2.5 py-1 text-xs font-semibold text-muted">
            <span className="h-1.5 w-1.5 rounded-full bg-faint" />
            {extension.statusLabel}
          </span>
        </div>

        <div className="rounded-lg border border-hairline bg-canvas px-3 py-2">
          <p className="text-xs font-medium text-muted">{extension.stepsLabel}</p>
          <ol className="mt-1 list-decimal space-y-0.5 pl-4 text-sm text-ink">
            {extension.steps.map((step) => (
              <li key={step}>{step}</li>
            ))}
          </ol>
        </div>

        <TwitchExtensionLink labels={extension} />
      </section>
    </div>
  );
}
