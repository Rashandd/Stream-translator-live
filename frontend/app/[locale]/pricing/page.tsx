import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { Icon } from "@/src/components/icon";
import { SiteFooter } from "@/src/components/site-footer";
import { SiteHeader } from "@/src/components/site-header";
import { getPublicConfig } from "@/src/lib/config";
import { fetchLivePlans, mergeLivePlans } from "@/src/lib/plans";
import { fetchFoundingPromo } from "@/src/lib/promo";
import { isLocale, localizePath, locales, type Locale } from "@/src/i18n/routing";

type Props = { params: Promise<{ locale: string }> };

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) return {};
  const config = await getPublicConfig(rawLocale);
  return { title: config.pricing.hero.eyebrow };
}

export default async function PricingPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { pricing } = config;
  const livePlans = await fetchLivePlans();
  const plans = mergeLivePlans(pricing.plans, livePlans);
  const promo = await fetchFoundingPromo();
  const promoActive = promo?.enabled ?? false;

  return (
    <main className="min-h-screen">
      <SiteHeader config={config} locale={locale} />

      <section className="mx-auto w-full max-w-7xl px-4 pb-16 pt-12 sm:px-6 lg:px-8 lg:pt-20">
        <p className="text-sm font-semibold uppercase tracking-wide text-blue">{pricing.hero.eyebrow}</p>
        <h1 className="mt-3 max-w-3xl text-balance text-4xl font-semibold leading-[1.08] text-ink sm:text-5xl">
          {pricing.hero.title}
        </h1>
        <p className="mt-4 max-w-2xl text-lg leading-8 text-muted">{pricing.hero.summary}</p>

        {promoActive && promo?.message ? (
          <div className="mt-6 flex flex-wrap items-center gap-2 rounded-xl border border-blue/25 bg-blue/10 px-4 py-3">
            <span className="rounded-full bg-blue px-2.5 py-0.5 text-xs font-semibold text-white">
              {pricing.promo.badge}
            </span>
            <p className="text-sm font-medium text-ink">{promo.message}</p>
          </div>
        ) : null}

        <div className="mt-12 grid gap-5 sm:grid-cols-2 lg:grid-cols-4">
          {plans.map((plan) => (
            <div
              className={`flex flex-col rounded-xl border bg-surface p-6 ${
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
              <p className="mt-3 flex flex-wrap items-baseline gap-1.5">
                {promoActive && plan.price !== "$0" ? (
                  <>
                    <span className="text-lg font-medium text-muted line-through">{plan.price}</span>
                    <span className="rounded-full bg-success/10 px-2 py-0.5 text-xs font-semibold text-success">
                      {pricing.promo.freeChipLabel}
                    </span>
                  </>
                ) : (
                  <>
                    <span className="text-3xl font-semibold text-ink">{plan.price}</span>
                    <span className="text-sm text-muted">{pricing.billingHint}</span>
                  </>
                )}
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
                    ? "bg-primary text-white hover:bg-primary-hover"
                    : "border border-border bg-surface text-ink hover:border-cyan hover:bg-cyan/10"
                }`}
                href={localizePath(locale, pricing.ctaHref)}
              >
                {plan.price === "$0" ? pricing.ctaLabel : pricing.planCtaLabel}
              </Link>
            </div>
          ))}
        </div>

      </section>

      <SiteFooter config={config} locale={locale} />
    </main>
  );
}
