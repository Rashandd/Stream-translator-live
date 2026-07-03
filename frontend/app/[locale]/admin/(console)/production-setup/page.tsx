import { notFound } from "next/navigation";
import { AdminProductionSetupPanel } from "@/src/components/admin-production-setup-panel";
import { PageHeader } from "@/src/components/dashboard-ui";
import { getPublicConfig } from "@/src/lib/config";
import { isLocale, localizePath, locales, type Locale } from "@/src/i18n/routing";

type Props = { params: Promise<{ locale: string }> };

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export default async function AdminProductionSetupPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { productionSetup } = config.admin;

  return (
    <div className="space-y-6">
      <PageHeader subtitle={productionSetup.subtitle} title={productionSetup.title} />

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
