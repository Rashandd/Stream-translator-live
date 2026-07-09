import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { AdminProductionSetupPanel } from "@/src/components/admin-production-setup-panel";
import { AdminPromoPanel } from "@/src/components/admin-promo-panel";
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
  return { title: config.admin.productionSetup.title };
}

export default async function AdminProductionSetupPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { productionSetup, promo } = config.admin;

  return (
    <div className="space-y-6">
      <PageHeader subtitle={productionSetup.subtitle} title={productionSetup.title} />

      <AdminPromoPanel labels={promo} loginHref={localizePath(locale, "/admin/login")} />

      <AdminProductionSetupPanel
        labels={{
          checksTitle: productionSetup.checksTitle,
          providersTitle: productionSetup.providersTitle,
          providersColumns: productionSetup.providersColumns,
          kindLabels: productionSetup.kindLabels,
          connectLabel: productionSetup.connectLabel,
          testLabel: productionSetup.testLabel,
          testingLabel: productionSetup.testingLabel,
          neverTested: productionSetup.neverTested,
          configuredLabel: productionSetup.configuredLabel,
          notConfiguredLabel: productionSetup.notConfiguredLabel,
          stripeTitle: productionSetup.stripeTitle,
          stripeSubtitle: productionSetup.stripeSubtitle,
          stripeFreeNote: productionSetup.stripeFreeNote,
          planAccessTitle: productionSetup.planAccessTitle,
          planAccessSubtitle: productionSetup.planAccessSubtitle,
          planAccessColumns: productionSetup.planAccessColumns,
          noPlanAccessProviders: productionSetup.noPlanAccessProviders,
          planAccessHint: productionSetup.planAccessHint,
          saveLabel: productionSetup.saveLabel,
          savingLabel: productionSetup.savingLabel,
          savedLabel: productionSetup.savedLabel,
          placeholder: productionSetup.placeholder,
          loading: productionSetup.loading,
          empty: productionSetup.empty,
          errors: productionSetup.errors,
          signInLabel: productionSetup.signInLabel,
        }}
        loginHref={localizePath(locale, "/admin/login")}
      />
    </div>
  );
}
