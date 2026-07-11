import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { Icon } from "@/src/components/icon";
import { SiteFooter } from "@/src/components/site-footer";
import { SiteHeader } from "@/src/components/site-header";
import { getPublicConfig } from "@/src/lib/config";
import { isLocale, localizePath, locales, type Locale } from "@/src/i18n/routing";

type HomePageProps = {
  params: Promise<{
    locale: string;
  }>;
};

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export async function generateMetadata({ params }: HomePageProps): Promise<Metadata> {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) return {};
  const config = await getPublicConfig(rawLocale);
  return { title: config.home.hero.eyebrow };
}

export default async function HomePage({ params }: HomePageProps) {
  const { locale: rawLocale } = await params;

  if (!isLocale(rawLocale)) {
    notFound();
  }

  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);

  return (
    <main className="min-h-screen overflow-hidden">
      <SiteHeader config={config} locale={locale} />

      <section className="mx-auto w-full max-w-7xl px-4 pb-16 pt-12 sm:px-6 lg:px-8 lg:pb-24 lg:pt-20">
        <div className="max-w-3xl">
          <div className="mb-6 inline-flex rounded-full border border-cyan/25 bg-surface/80 px-4 py-2 text-sm font-semibold text-ink shadow-sm">
            {config.home.hero.eyebrow}
          </div>
          <h1 className="max-w-4xl text-balance text-5xl font-semibold leading-[1.03] tracking-normal text-ink sm:text-6xl lg:text-7xl">
            {config.home.hero.title}
          </h1>
          <p className="mt-6 max-w-2xl text-lg leading-8 text-muted">
            {config.home.hero.summary}
          </p>
          <div className="mt-8 flex flex-col gap-3 sm:flex-row">
            <Link className="inline-flex h-12 items-center justify-center rounded-full bg-primary px-6 text-sm font-semibold text-on-primary transition hover:bg-primary-hover" href={localizePath(locale, config.home.hero.primaryCta.href)}>
              {config.home.hero.primaryCta.label}
            </Link>
            <Link className="inline-flex h-12 items-center justify-center rounded-full border border-border bg-surface px-6 text-sm font-semibold text-ink transition hover:border-cyan hover:bg-cyan/10" href={localizePath(locale, config.home.hero.secondaryCta.href)}>
              {config.home.hero.secondaryCta.label}
            </Link>
          </div>
          <p className="mt-4 text-sm leading-6 text-muted">{config.home.hero.note}</p>
        </div>
      </section>

      <section className="mx-auto w-full max-w-7xl px-4 py-16 sm:px-6 lg:px-8 lg:py-20">
        <p className="text-sm font-semibold uppercase tracking-wide text-blue">{config.home.now.eyebrow}</p>
        <h2 className="mt-3 max-w-2xl text-3xl font-semibold text-ink sm:text-4xl">{config.home.now.title}</h2>
        <div className="mt-10 grid gap-5 sm:grid-cols-2 lg:grid-cols-3">
          {config.home.now.items.map((item) => (
            <div className="rounded-xl border border-border bg-surface p-6" key={item.title}>
              <Icon className="h-6 w-6 text-blue" name={item.icon} />
              <h3 className="mt-4 text-xl font-semibold text-ink">{item.title}</h3>
              <p className="mt-2 text-base leading-7 text-muted">{item.body}</p>
            </div>
          ))}
        </div>
      </section>

      <section className="bg-emphasis-green text-white">
        <div className="mx-auto w-full max-w-7xl px-4 py-16 sm:px-6 lg:px-8 lg:py-20">
          <p className="text-sm font-semibold uppercase tracking-wide text-cyan">{config.home.edge.eyebrow}</p>
          <h2 className="mt-3 max-w-3xl text-3xl font-semibold text-white sm:text-4xl">{config.home.edge.title}</h2>
          <div className="mt-10 grid gap-5 sm:grid-cols-2 lg:grid-cols-4">
            {config.home.edge.items.map((item) => (
              <div className="rounded-xl border border-white/12 bg-surface/[0.06] p-6" key={item.title}>
                <Icon className="h-6 w-6 text-cyan" name={item.icon} />
                <h3 className="mt-4 text-lg font-semibold text-white">{item.title}</h3>
                <p className="mt-2 text-sm leading-6 text-white/70">{item.body}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      <section className="mx-auto w-full max-w-7xl px-4 py-16 sm:px-6 lg:px-8 lg:py-20">
        <p className="text-sm font-semibold uppercase tracking-wide text-blue">{config.home.audience.eyebrow}</p>
        <h2 className="mt-3 max-w-2xl text-3xl font-semibold text-ink sm:text-4xl">{config.home.audience.title}</h2>
        <div className="mt-10 grid gap-5 sm:grid-cols-2 lg:grid-cols-3">
          {config.home.audience.items.map((item) => (
            <div className="rounded-xl border border-border bg-surface p-6" key={item.title}>
              <Icon className="h-6 w-6 text-blue" name={item.icon} />
              <h3 className="mt-4 text-lg font-semibold text-ink">{item.title}</h3>
              <p className="mt-2 text-sm leading-6 text-muted">{item.body}</p>
            </div>
          ))}
        </div>
      </section>

      <SiteFooter config={config} locale={locale} />
    </main>
  );
}
