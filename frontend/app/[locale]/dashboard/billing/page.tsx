import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { BillingPlanCard } from "@/src/components/billing-plan-card";
import { PageHeader, SectionCard } from "@/src/components/dashboard-ui";
import { getPublicConfig } from "@/src/lib/config";
import { fetchLivePlans, mergeLivePlans } from "@/src/lib/plans";
import { isLocale, locales, type Locale } from "@/src/i18n/routing";

type Props = { params: Promise<{ locale: string }> };

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) return {};
  const config = await getPublicConfig(rawLocale);
  return { title: config.dashboard.billing.title };
}

export default async function BillingPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { billing } = config.dashboard;
  const livePlans = await fetchLivePlans();
  const displayPlans = mergeLivePlans(config.pricing.plans, livePlans);

  return (
    <div className="space-y-6">
      <PageHeader subtitle={billing.subtitle} title={billing.title} />

      <div className="grid gap-6 lg:grid-cols-[0.9fr_1fr]">
        <BillingPlanCard
          displayPlans={displayPlans}
          fallback={{ ...billing.plan }}
          labels={{
            usageLabel: billing.plan.usageLabel,
            upgradeLabel: billing.upgradeLabel,
            upgradingLabel: billing.upgradingLabel,
            topPlanLabel: billing.topPlanLabel,
            checkoutUnavailable: billing.checkoutUnavailable,
            errorNetwork: billing.errorNetwork,
          }}
          livePlans={livePlans}
        />

        <SectionCard title={billing.invoicesTitle}>
          {billing.invoices.length === 0 ? (
            <p className="rounded-lg border border-dashed border-border bg-canvas px-4 py-10 text-center text-sm text-muted">
              {billing.invoicesEmpty}
            </p>
          ) : (
            <ul className="divide-y divide-hairline">
              {billing.invoices.map((invoice: { date: string; amount: string; status: string }) => (
                <li className="flex items-center justify-between py-3 first:pt-0" key={invoice.date}>
                  <span className="font-mono text-sm text-ink">{invoice.date}</span>
                  <span className="text-sm font-semibold text-ink">{invoice.amount}</span>
                  <span className="text-sm text-muted">{invoice.status}</span>
                </li>
              ))}
            </ul>
          )}
        </SectionCard>
      </div>
    </div>
  );
}
