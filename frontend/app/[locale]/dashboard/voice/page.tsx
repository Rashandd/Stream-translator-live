import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { PageHeader } from "@/src/components/dashboard-ui";
import { TtsProcessingPanel } from "@/src/components/tts-processing-panel";
import { VoiceCloneStudio } from "@/src/components/voice-clone-studio";
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
  return { title: config.dashboard.voice.title };
}

export default async function VoicePage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { voice } = config.dashboard;

  return (
    <div className="space-y-6">
      <PageHeader subtitle={voice.subtitle} title={voice.title} />
      <TtsProcessingPanel labels={voice.processing} />
      <VoiceCloneStudio labels={voice} />
    </div>
  );
}
