import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { AdminCostsPanel } from "@/src/components/admin-costs-panel";
import { PageHeader } from "@/src/components/dashboard-ui";
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
  return { title: config.admin.costs.title };
}

export default async function AdminCostsPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { costs } = config.admin;

  return (
    <div className="space-y-6">
      <PageHeader subtitle={costs.subtitle} title={costs.title} />

      <AdminCostsPanel
        labels={{
          usageLabel: costs.usageLabel,
          costLabel: costs.costLabel,
          costHint: costs.costHint,
          revenueLabel: costs.revenueLabel,
          revenueHint: costs.revenueHint,
          usersLabel: costs.usersLabel,
          usersHint: costs.usersHint,
          loading: costs.loading,
          errors: costs.errors,
          signInLabel: costs.signInLabel,
        }}
        loginHref={localizePath(locale, "/admin/login")}
      />

      <p className="text-sm leading-6 text-muted">{costs.note}</p>
    </div>
  );
}
