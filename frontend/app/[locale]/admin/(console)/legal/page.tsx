import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { AdminLegalPanel } from "@/src/components/admin-legal-panel";
import { PageHeader } from "@/src/components/dashboard-ui";
import { getPublicConfig } from "@/src/lib/config";
import { isLocale, localizePath, locales } from "@/src/i18n/routing";

type Props = { params: Promise<{ locale: string }> };

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) return {};
  const config = await getPublicConfig(rawLocale);
  return { title: config.admin.legal.title };
}

export default async function AdminLegalPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }

  const config = await getPublicConfig(rawLocale);
  const { legal } = config.admin;

  return (
    <div className="space-y-6">
      <PageHeader subtitle={legal.subtitle} title={legal.title} />
      <AdminLegalPanel
        labels={legal}
        loginHref={localizePath(rawLocale, "/admin/login")}
        pages={config.legal.pages}
      />
    </div>
  );
}
