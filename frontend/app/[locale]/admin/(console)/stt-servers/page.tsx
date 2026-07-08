import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { AdminSttServersPanel } from "@/src/components/admin-stt-servers-panel";
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
  return { title: config.admin.sttServers.title };
}

export default async function AdminSttServersPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { sttServers } = config.admin;

  return (
    <div className="space-y-6">
      <PageHeader subtitle={sttServers.subtitle} title={sttServers.title} />

      <AdminSttServersPanel
        labels={{
          addTitle: sttServers.addTitle,
          nameLabel: sttServers.nameLabel,
          namePlaceholder: sttServers.namePlaceholder,
          hostnameLabel: sttServers.hostnameLabel,
          hostnamePlaceholder: sttServers.hostnamePlaceholder,
          sshPortLabel: sttServers.sshPortLabel,
          sshUserLabel: sttServers.sshUserLabel,
          addButton: sttServers.addButton,
          addingButton: sttServers.addingButton,
          publicKeyIntro: sttServers.publicKeyIntro,
          copyLabel: sttServers.copyLabel,
          copiedLabel: sttServers.copiedLabel,
          columns: sttServers.columns,
          statusLabels: sttServers.statusLabels,
          provisionButton: sttServers.provisionButton,
          provisioningButton: sttServers.provisioningButton,
          deleteButton: sttServers.deleteButton,
          confirmDelete: sttServers.confirmDelete,
          loading: sttServers.loading,
          empty: sttServers.empty,
          errors: sttServers.errors,
          signInLabel: sttServers.signInLabel,
        }}
        loginHref={localizePath(locale, "/admin/login")}
      />

      <p className="text-sm leading-6 text-muted">{sttServers.note}</p>
    </div>
  );
}
