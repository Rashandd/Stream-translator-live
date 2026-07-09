import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { Icon } from "@/src/components/icon";
import { SiteFooter } from "@/src/components/site-footer";
import { SiteHeader } from "@/src/components/site-header";
import { getPublicConfig } from "@/src/lib/config";
import { isLocale, localizePath, locales, type Locale } from "@/src/i18n/routing";

type Props = { params: Promise<{ locale: string }> };

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) return {};
  const config = await getPublicConfig(rawLocale);
  return { title: config.howItWorks.hero.eyebrow };
}

export default async function HowItWorksPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { howItWorks } = config;

  return (
    <main className="min-h-screen">
      <SiteHeader config={config} locale={locale} />

      <section className="mx-auto w-full max-w-7xl px-4 pb-16 pt-12 sm:px-6 lg:px-8 lg:pt-20">
        <p className="text-sm font-semibold uppercase tracking-wide text-blue">{howItWorks.hero.eyebrow}</p>
        <h1 className="mt-3 max-w-3xl text-balance text-4xl font-semibold leading-[1.08] text-ink sm:text-5xl">
          {howItWorks.hero.title}
        </h1>
        <p className="mt-4 max-w-2xl text-lg leading-8 text-muted">{howItWorks.hero.summary}</p>

        <ol className="mt-12 grid gap-5 sm:grid-cols-2 lg:grid-cols-4">
          {howItWorks.steps.map((step, index) => (
            <li className="rounded-xl border border-border bg-white p-6" key={step.title}>
              <div className="flex items-center gap-3">
                <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-ink text-sm font-semibold text-white">
                  {index + 1}
                </span>
                <Icon className="h-6 w-6 text-blue" name={step.icon} />
              </div>
              <h2 className="mt-4 text-lg font-semibold text-ink">{step.title}</h2>
              <p className="mt-2 text-sm leading-6 text-muted">{step.body}</p>
            </li>
          ))}
        </ol>

        <div className="mt-8 flex items-start gap-3 rounded-xl border border-border bg-canvas p-5">
          <Icon className="mt-0.5 h-5 w-5 shrink-0 text-blue" name="LockKeyhole" />
          <p className="text-sm leading-6 text-muted">{howItWorks.privacyNote}</p>
        </div>

        <div className="mt-16">
          <h2 className="text-2xl font-semibold text-ink">{howItWorks.faqTitle}</h2>
          <div className="mt-6 divide-y divide-hairline rounded-xl border border-border bg-white">
            {howItWorks.faq.map((item) => (
              <div className="p-6" key={item.question}>
                <h3 className="text-base font-semibold text-ink">{item.question}</h3>
                <p className="mt-2 text-sm leading-6 text-muted">{item.answer}</p>
              </div>
            ))}
          </div>
        </div>

        <div className="mt-12 rounded-2xl border border-border bg-white p-6 sm:p-8">
          <h2 className="text-xl font-semibold text-ink">{howItWorks.ctaTitle}</h2>
          <div className="mt-4 flex flex-wrap gap-3">
            <Link
              className="inline-flex h-11 items-center justify-center rounded-full bg-ink px-5 text-sm font-semibold text-white transition hover:bg-text"
              href={localizePath(locale, howItWorks.primaryCta.href)}
            >
              {howItWorks.primaryCta.label}
            </Link>
            <Link
              className="inline-flex h-11 items-center justify-center rounded-full border border-border bg-white px-5 text-sm font-semibold text-ink transition hover:border-cyan hover:bg-cyan/10"
              href={localizePath(locale, howItWorks.secondaryCta.href)}
            >
              {howItWorks.secondaryCta.label}
            </Link>
          </div>
        </div>
      </section>

      <SiteFooter config={config} locale={locale} />
    </main>
  );
}
