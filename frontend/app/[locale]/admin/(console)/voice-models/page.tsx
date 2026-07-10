import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { AdminVoiceModelsPanel } from "@/src/components/admin-voice-models-panel";
import { PageHeader } from "@/src/components/dashboard-ui";
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
  return { title: config.admin.voiceModels.title };
}

export default async function AdminVoiceModelsPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { voiceModels } = config.admin;

  return (
    <div className="space-y-6">
      <PageHeader subtitle={voiceModels.subtitle} title={voiceModels.title} />
      <section className="rounded-xl border border-border bg-surface p-5 sm:p-6">
        <AdminVoiceModelsPanel labels={voiceModels} />
      </section>
    </div>
  );
}
