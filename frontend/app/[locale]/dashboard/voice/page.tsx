import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { PageHeader, SectionCard } from "@/src/components/dashboard-ui";
import { StatusPill } from "@/src/components/status-pill";
import { WaitlistCta } from "@/src/components/waitlist-cta";
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
      <SectionCard
        action={<StatusPill label="coming_soon" text={config.statusLabels.coming_soon} />}
        title={voice.comingSoon.title}
      >
        <p className="text-sm leading-6 text-muted">{voice.comingSoon.summary}</p>
        <WaitlistCta interest="voice-clone" labels={voice.comingSoon.waitlist} />
      </SectionCard>
    </div>
  );
}
