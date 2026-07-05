import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { CaptionOverlayBuilder } from "@/src/components/caption-overlay-builder";
import { Icon } from "@/src/components/icon";
import { PageHeader, SectionCard } from "@/src/components/dashboard-ui";
import { SttProcessingPanel } from "@/src/components/stt-processing-panel";
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
  return { title: config.dashboard.captions.title };
}

export default async function CaptionsPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { captions } = config.dashboard;

  return (
    <div className="space-y-6">
      <PageHeader subtitle={captions.subtitle} title={captions.title} />

      <SttProcessingPanel labels={captions.processing} />

      <div className="grid gap-6 lg:grid-cols-2">
        <SectionCard title={captions.targetsLabel}>
          <div className="mb-5">
            <p className="text-xs font-medium text-muted">{captions.sourceLabel}</p>
            <div className="mt-1 inline-flex items-center gap-2 rounded-lg border border-border bg-canvas px-3 py-2 text-sm font-semibold text-ink">
              <Icon className="h-4 w-4 text-blue" name="Mic" />
              {captions.sourceValue}
            </div>
          </div>
          <ul className="space-y-2">
            {captions.targets.map((target) => (
              <li
                className="flex items-center justify-between rounded-lg border border-hairline bg-canvas px-4 py-3"
                key={target.code}
              >
                <span className="flex items-center gap-3 text-sm font-semibold text-ink">
                  <span className="font-mono text-xs text-muted">{target.code}</span>
                  {target.language}
                </span>
                <span
                  className={`inline-flex h-6 w-11 items-center rounded-full px-0.5 ${target.enabled ? "justify-end bg-success" : "justify-start bg-border"}`}
                >
                  <span className="h-5 w-5 rounded-full bg-white" />
                </span>
              </li>
            ))}
          </ul>
        </SectionCard>

        <SectionCard title={captions.appearanceTitle}>
          <CaptionOverlayBuilder
            labels={captions.builder}
            overlayUrlTemplate={captions.overlayUrl}
            sourceLabel={captions.sourceLabel}
            sourceValue={captions.sourceValue}
            targets={captions.targets}
          />
        </SectionCard>
      </div>
    </div>
  );
}
