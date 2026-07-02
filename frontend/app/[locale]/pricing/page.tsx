import Link from "next/link";
import { notFound } from "next/navigation";
import { Icon } from "@/src/components/icon";
import { SiteHeader } from "@/src/components/site-header";
import { getPublicConfig } from "@/src/lib/config";
import { isLocale, localizePath, locales, type Locale } from "@/src/i18n/routing";

type Props = { params: Promise<{ locale: string }> };

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export default async function PricingPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { pricing } = config;

  return (
    <main className="min-h-screen">
      <SiteHeader config={config} locale={locale} />

      <section className="mx-auto w-full max-w-7xl px-4 pb-16 pt-12 sm:px-6 lg:px-8 lg:pt-20">
        <p className="text-sm font-semibold uppercase tracking-wide text-blue">{pricing.hero.eyebrow}</p>
        <h1 className="mt-3 max-w-3xl text-balance text-4xl font-semibold leading-[1.08] text-ink sm:text-5xl">
          {pricing.hero.title}
        </h1>
        <p className="mt-4 max-w-2xl text-lg leading-8 text-muted">{pricing.hero.summary}</p>

        <div className="mt-12 grid gap-5 sm:grid-cols-2 lg:grid-cols-4">
          {pricing.plans.map((plan) => (
            <div
              className={`flex flex-col rounded-xl border bg-white p-6 ${
                plan.popular ? "border-blue shadow-panel" : "border-border"
              }`}
              key={plan.name}
            >
              <div className="flex items-center justify-between">
                <h2 className="text-lg font-semibold text-ink">{plan.name}</h2>
                {plan.popular ? (
                  <span className="rounded-full border border-blue/25 bg-blue/10 px-2.5 py-0.5 text-xs font-semibold text-ink">
                    {pricing.popularLabel}
                  </span>
                ) : null}
              </div>
              <p className="mt-3 flex items-baseline gap-1.5">
                <span className="text-3xl font-semibold text-ink">{plan.price}</span>
                <span className="text-sm text-muted">{pricing.billingHint}</span>
              </p>
              <p className="mt-2 text-sm leading-6 text-muted">{plan.summary}</p>
              <ul className="mt-4 space-y-2.5">
                {plan.features.map((feature) => (
                  <li className="flex items-start gap-2.5 text-sm leading-6 text-text" key={feature}>
                    <Icon className="mt-1 h-4 w-4 shrink-0 text-success" name="CheckCircle2" />
                    {feature}
                  </li>
                ))}
              </ul>
              <Link
                className={`mt-6 inline-flex h-11 items-center justify-center rounded-full px-5 text-sm font-semibold transition ${
                  plan.popular
                    ? "bg-ink text-white hover:bg-text"
                    : "border border-border bg-white text-ink hover:border-cyan hover:bg-cyan/10"
                }`}
                href={localizePath(locale, pricing.ctaHref)}
              >
                {plan.price === "$0" ? pricing.ctaLabel : pricing.planCtaLabel}
              </Link>
            </div>
          ))}
        </div>

        <p className="mt-8 max-w-3xl text-sm leading-6 text-muted">{pricing.note}</p>
      </section>
    </main>
  );
}
