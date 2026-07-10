import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { Icon } from "@/src/components/icon";
import { SiteFooter } from "@/src/components/site-footer";
import { SiteHeader } from "@/src/components/site-header";
import { getPublicConfig } from "@/src/lib/config";
import { isLocale, localizePath, locales, type Locale } from "@/src/i18n/routing";

type SolutionsPageProps = {
  params: Promise<{
    locale: string;
  }>;
};

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export async function generateMetadata({ params }: SolutionsPageProps): Promise<Metadata> {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) return {};
  const config = await getPublicConfig(rawLocale);
  return { title: config.solutions.hero.eyebrow };
}

export default async function SolutionsPage({ params }: SolutionsPageProps) {
  const { locale: rawLocale } = await params;

  if (!isLocale(rawLocale)) {
    notFound();
  }

  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { solutions } = config;

  return (
    <main className="min-h-screen overflow-hidden">
      <SiteHeader config={config} locale={locale} />

      <section className="mx-auto w-full max-w-7xl px-4 pb-12 pt-12 sm:px-6 lg:px-8 lg:pt-20">
        <p className="text-sm font-semibold uppercase tracking-wide text-blue">{solutions.hero.eyebrow}</p>
        <h1 className="mt-3 max-w-3xl text-balance text-4xl font-semibold leading-[1.05] text-ink sm:text-5xl lg:text-6xl">
          {solutions.hero.title}
        </h1>
        <p className="mt-6 max-w-2xl text-lg leading-8 text-muted">{solutions.hero.summary}</p>
      </section>

      <section className="mx-auto w-full max-w-7xl px-4 pb-16 sm:px-6 lg:px-8 lg:pb-24">
        <div className="grid gap-5 sm:grid-cols-2 lg:grid-cols-3">
          {solutions.items.map((item) => (
            <div className="flex flex-col rounded-xl border border-border bg-surface p-6" key={item.title}>
              <Icon className="h-6 w-6 text-blue" name={item.icon} />
              <h3 className="mt-4 text-xl font-semibold text-ink">{item.title}</h3>
              <p className="mt-2 text-base leading-7 text-muted">{item.body}</p>
              <span className="mt-4 text-xs font-semibold uppercase tracking-wide text-muted">
                {solutions.audienceLabels[item.audience as keyof typeof solutions.audienceLabels]}
              </span>
            </div>
          ))}
        </div>
      </section>

      <section className="mx-auto w-full max-w-7xl px-4 pb-24 sm:px-6 lg:px-8">
        <div className="flex flex-col gap-6 rounded-[28px] border border-border bg-canvas p-8 text-center sm:p-12">
          <div>
            <p className="text-sm font-semibold uppercase tracking-wide text-blue">{solutions.cta.eyebrow}</p>
            <h2 className="mt-3 text-2xl font-semibold text-ink sm:text-3xl">{solutions.cta.title}</h2>
          </div>
          <div className="flex flex-col justify-center gap-3 sm:flex-row">
            <Link
              className="inline-flex h-12 items-center justify-center rounded-full bg-primary px-6 text-sm font-semibold text-white transition hover:bg-primary-hover"
              href={localizePath(locale, solutions.cta.primary.href)}
            >
              {solutions.cta.primary.label}
            </Link>
            <Link
              className="inline-flex h-12 items-center justify-center rounded-full border border-border bg-surface px-6 text-sm font-semibold text-ink transition hover:border-cyan hover:bg-cyan/10"
              href={localizePath(locale, solutions.cta.secondary.href)}
            >
              {solutions.cta.secondary.label}
            </Link>
          </div>
        </div>
      </section>

      <SiteFooter config={config} locale={locale} />
    </main>
  );
}
