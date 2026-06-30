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

export default async function SdkPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { sdk } = config.dashboard;

  return (
    <div className="space-y-6">
      <PageHeader subtitle={sdk.subtitle} title={sdk.title} />

      <div className="grid gap-6 lg:grid-cols-2">
        <SectionCard subtitle={sdk.downloadNote} title={sdk.downloadTitle}>
          <ul className="space-y-2">
            {sdk.platforms.map((platform) => {
              const available = platform.status === "enabled";
              return (
                <li className="flex items-center justify-between rounded-lg border border-hairline bg-canvas px-4 py-3" key={platform.name}>
                  <span className="flex items-center gap-3 text-sm font-semibold text-ink">
                    <Icon className="h-5 w-5 text-muted" name={platform.icon} />
                    {platform.name}
                  </span>
                  {available ? (
                    <button
                      className="inline-flex h-9 items-center gap-1.5 rounded-full bg-ink px-4 text-sm font-semibold text-white transition hover:bg-text"
                      type="button"
                    >
                      <Icon className="h-4 w-4" name="Download" />
                      {platform.action}
                    </button>
                  ) : (
                    <StatusPill label={platform.status} text={config.statusLabels[platform.status as keyof typeof config.statusLabels]} />
                  )}
                </li>
              );
            })}
          </ul>
        </SectionCard>

        <SectionCard subtitle={sdk.pairingNote} title={sdk.pairingTitle}>
          <p className="text-xs font-medium text-muted">{sdk.pairingCodeLabel}</p>
          <div className="mt-1 rounded-lg border border-border bg-canvas px-4 py-5 text-center">
            <code className="font-mono text-3xl font-semibold tracking-[0.3em] text-ink">{sdk.pairingCode}</code>
          </div>
          <button
            className="mt-4 inline-flex h-11 w-full items-center justify-center gap-2 rounded-full border border-border bg-white px-5 text-sm font-semibold text-ink transition hover:border-cyan hover:bg-cyan/10"
            type="button"
          >
            <Icon className="h-4 w-4" name="KeyRound" />
            {sdk.generateLabel}
          </button>
        </SectionCard>
      </div>

      <SectionCard
        action={
          <span className="inline-flex items-center gap-1.5 rounded-full border border-warning/25 bg-warning/10 px-3 py-1 text-xs font-semibold text-ink">
            <span className="h-1.5 w-1.5 rounded-full bg-warning" />
            {sdk.syncLabel}: {sdk.syncValue}
          </span>
        }
        subtitle={sdk.streamKeyNote}
        title={sdk.streamKeyTitle}
      >
        <p className="text-xs font-medium text-muted">{sdk.streamKeyLabel}</p>
        <div className="mt-1 flex flex-col gap-2 sm:flex-row sm:items-center">
          <div className="flex min-w-0 flex-1 items-center gap-2 rounded-lg border border-border bg-canvas px-3 py-2">
            <code className="min-w-0 flex-1 truncate font-mono text-sm text-ink">{sdk.streamKey}</code>
            <span className="inline-flex shrink-0 items-center gap-1.5 rounded-md border border-border bg-white px-2.5 py-1 text-xs font-semibold text-ink">
              <Icon className="h-3.5 w-3.5" name="Copy" />
              {sdk.copyLabel}
            </span>
          </div>
          <button
            className="inline-flex h-10 shrink-0 items-center justify-center gap-1.5 rounded-full border border-border bg-white px-4 text-sm font-semibold text-ink transition hover:border-danger hover:text-danger"
            type="button"
          >
            <Icon className="h-4 w-4" name="KeyRound" />
            {sdk.regenerateLabel}
          </button>
        </div>
        <p className="mt-3 text-sm leading-6 text-muted">{sdk.regenerateNote}</p>
      </SectionCard>
    </div>
  );
}
