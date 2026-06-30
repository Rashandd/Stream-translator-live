import { notFound } from "next/navigation";
import { Icon } from "@/src/components/icon";
import { PageHeader, SectionCard } from "@/src/components/dashboard-ui";
import { getPublicConfig } from "@/src/lib/config";
import { isLocale, locales, type Locale } from "@/src/i18n/routing";

type Props = { params: Promise<{ locale: string }> };

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export default async function BillingPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { billing } = config.dashboard;

  return (
    <div className="space-y-6">
      <PageHeader subtitle={billing.subtitle} title={billing.title} />

      <div className="grid gap-6 lg:grid-cols-[0.9fr_1fr]">
        <section className="rounded-xl border border-border bg-white p-6">
          <div className="flex items-baseline justify-between">
            <h2 className="text-lg font-semibold text-ink">{billing.plan.name}</h2>
            <p className="font-mono text-lg font-semibold text-ink">{billing.plan.price}</p>
          </div>
          <p className="mt-3 text-xs font-medium text-muted">{billing.plan.usageLabel}</p>
          <p className="font-mono text-sm font-semibold text-ink">{billing.plan.usage}</p>
          <ul className="mt-4 space-y-2">
            {billing.plan.features.map((feature) => (
              <li className="flex items-center gap-2 text-sm text-text" key={feature}>
                <Icon className="h-4 w-4 shrink-0 text-success" name="CheckCircle2" />
                {feature}
              </li>
            ))}
          </ul>
          <button
            className="mt-6 inline-flex h-11 w-full items-center justify-center rounded-full bg-ink px-5 text-sm font-semibold text-white transition hover:bg-text"
            type="button"
          >
            {billing.upgradeLabel}
          </button>
        </section>

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
