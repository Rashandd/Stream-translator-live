import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { Icon } from "@/src/components/icon";
import { PageHeader, SectionCard } from "@/src/components/dashboard-ui";
import { SdkDevicesPanel } from "@/src/components/sdk-devices-panel";
import { SdkPairingPanel } from "@/src/components/sdk-pairing-panel";
import { SdkStreamKeyPanel } from "@/src/components/sdk-stream-key-panel";
import { StatusPill } from "@/src/components/status-pill";
import { getPublicConfig } from "@/src/lib/config";
import { isLocale, locales, type Locale } from "@/src/i18n/routing";
import { sdkDownloadUrl } from "@/src/lib/sdk-releases";

type Props = { params: Promise<{ locale: string }> };

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) return {};
  const config = await getPublicConfig(rawLocale);
  return { title: config.dashboard.sdk.title };
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
              const href = available ? sdkDownloadUrl(platform.name) : null;
              return (
                <li className="flex items-center justify-between rounded-lg border border-hairline bg-canvas px-4 py-3" key={platform.name}>
                  <span className="flex items-center gap-3 text-sm font-semibold text-ink">
                    <Icon className="h-5 w-5 text-muted" name={platform.icon} />
                    {platform.name}
                  </span>
                  {href ? (
                    <a
                      className="inline-flex h-9 items-center gap-1.5 rounded-full bg-primary px-4 text-sm font-semibold text-white transition hover:bg-primary-hover"
                      download
                      href={href}
                    >
                      <Icon className="h-4 w-4" name="Download" />
                      {platform.action}
                    </a>
                  ) : available ? (
                    <span
                      className="inline-flex h-9 items-center gap-1.5 rounded-full border border-hairline px-4 text-sm font-semibold text-muted"
                      title={sdk.downloadUnconfigured}
                    >
                      <Icon className="h-4 w-4" name="Download" />
                      {sdk.downloadUnconfigured}
                    </span>
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
          <SdkPairingPanel
            labels={{
              placeholder: sdk.pairingPlaceholder,
              generateLabel: sdk.generateLabel,
              generatingLabel: sdk.generatingLabel,
              expiresInTemplate: sdk.expiresInTemplate,
              errors: sdk.pairingErrors,
            }}
          />
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
        <SdkStreamKeyPanel
          labels={{
            copyLabel: sdk.copyLabel,
            copiedLabel: sdk.copiedLabel,
            regenerateLabel: sdk.regenerateLabel,
            loading: sdk.streamKeyLoading,
            errors: sdk.streamKeyErrors,
          }}
        />
        <p className="mt-3 text-sm leading-6 text-muted">{sdk.regenerateNote}</p>
      </SectionCard>

      <SectionCard subtitle={sdk.devicesNote} title={sdk.devicesTitle}>
        <SdkDevicesPanel
          labels={{
            emptyState: sdk.devices.emptyState,
            connectedLabel: sdk.devices.connectedLabel,
            offlineLabel: sdk.devices.offlineLabel,
            pairedTemplate: sdk.devices.pairedTemplate,
            lastSeenTemplate: sdk.devices.lastSeenTemplate,
            removeLabel: sdk.devices.removeLabel,
            removingLabel: sdk.devices.removingLabel,
            errors: sdk.devices.errors,
          }}
        />
      </SectionCard>
    </div>
  );
}
