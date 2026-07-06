import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { CaptionOverlayBuilder } from "@/src/components/caption-overlay-builder";
import { PageHeader, SectionCard } from "@/src/components/dashboard-ui";
import { SttProcessingPanel } from "@/src/components/stt-processing-panel";
import { TargetLanguagePicker } from "@/src/components/target-language-picker";
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
        <TargetLanguagePicker labels={captions.languagePicker} />

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
