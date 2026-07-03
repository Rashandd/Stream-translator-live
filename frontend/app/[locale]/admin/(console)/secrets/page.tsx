import { notFound } from "next/navigation";
import { AdminSecretsPanel } from "@/src/components/admin-secrets-panel";
import { Icon } from "@/src/components/icon";
import { PageHeader } from "@/src/components/dashboard-ui";
import { getPublicConfig } from "@/src/lib/config";
import { isLocale, localizePath, locales, type Locale } from "@/src/i18n/routing";

type Props = { params: Promise<{ locale: string }> };

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export default async function AdminSecretsPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { secrets } = config.admin;

  return (
    <div className="space-y-6">
      <PageHeader subtitle={secrets.subtitle} title={secrets.title} />

      <AdminSecretsPanel
        labels={{
          columns: secrets.columns,
          sourceLabels: secrets.sourceLabels,
          placeholder: secrets.placeholder,
          saveLabel: secrets.saveLabel,
          savingLabel: secrets.savingLabel,
          clearLabel: secrets.clearLabel,
          confirmClear: secrets.confirmClear,
          testLabel: secrets.testLabel,
          testingLabel: secrets.testingLabel,
          loading: secrets.loading,
          empty: secrets.empty,
          errors: secrets.errors,
          signInLabel: secrets.signInLabel,
        }}
        loginHref={localizePath(locale, "/admin/login")}
      />

      <p className="flex items-start gap-2 text-sm leading-6 text-muted">
        <Icon className="mt-0.5 h-4 w-4 shrink-0 text-blue" name="LockKeyhole" />
        {secrets.maskNote}
      </p>
      <p className="text-sm leading-6 text-muted">{secrets.note}</p>
    </div>
  );
}
