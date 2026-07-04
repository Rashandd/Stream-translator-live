import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { Icon } from "@/src/components/icon";
import { SiteHeader } from "@/src/components/site-header";
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
  return { title: config.security.hero.eyebrow };
}

export default async function SecurityPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { security } = config;

  return (
    <main className="min-h-screen">
      <SiteHeader config={config} locale={locale} />

      <section className="mx-auto w-full max-w-7xl px-4 pb-16 pt-12 sm:px-6 lg:px-8 lg:pt-20">
        <p className="text-sm font-semibold uppercase tracking-wide text-blue">{security.hero.eyebrow}</p>
        <h1 className="mt-3 max-w-3xl text-balance text-4xl font-semibold leading-[1.08] text-ink sm:text-5xl">
          {security.hero.title}
        </h1>
        <p className="mt-4 max-w-2xl text-lg leading-8 text-muted">{security.hero.summary}</p>

        <div className="mt-12 grid gap-5 sm:grid-cols-2">
          {security.claims.map((claim) => (
            <div className="rounded-xl border border-border bg-white p-6" key={claim.title}>
              <Icon className="h-6 w-6 text-blue" name={claim.icon} />
              <h2 className="mt-4 text-lg font-semibold text-ink">{claim.title}</h2>
              <p className="mt-2 text-sm leading-6 text-muted">{claim.body}</p>
            </div>
          ))}
        </div>

        <p className="mt-8 max-w-3xl text-sm leading-6 text-muted">{security.note}</p>
      </section>
    </main>
  );
}
