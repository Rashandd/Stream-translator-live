import { notFound } from "next/navigation";
import { Icon } from "@/src/components/icon";
import { PageHeader } from "@/src/components/dashboard-ui";
import { StatusPill } from "@/src/components/status-pill";
import { getPublicConfig } from "@/src/lib/config";
import { isLocale, locales, type Locale } from "@/src/i18n/routing";

type Props = { params: Promise<{ locale: string }> };

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export default async function VoiceStudioPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { voice } = config.dashboard;

  return (
    <div className="space-y-6">
      <PageHeader
        action={<StatusPill label={voice.status} text={config.statusLabels[voice.status as keyof typeof config.statusLabels]} />}
        subtitle={voice.subtitle}
        title={voice.title}
      />

      <div className="flex items-start gap-3 rounded-xl border border-cyan/25 bg-cyan/10 p-4">
        <Icon className="mt-0.5 h-5 w-5 shrink-0 text-blue" name="LockKeyhole" />
        <p className="text-sm leading-6 text-text">{voice.notice}</p>
      </div>

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4" aria-disabled="true">
        {voice.features.map((feature) => (
          <article className="rounded-xl border border-border bg-white p-5 opacity-60" key={feature.title}>
            <Icon className="h-6 w-6 text-muted" name={feature.icon} />
            <h3 className="mt-4 text-base font-semibold text-ink">{feature.title}</h3>
            <p className="mt-2 text-sm leading-6 text-muted">{feature.body}</p>
          </article>
        ))}
      </div>
    </div>
  );
}
