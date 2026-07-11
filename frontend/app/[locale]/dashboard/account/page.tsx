import type { Metadata } from "next";
import { Suspense } from "react";
import { notFound } from "next/navigation";
import { PageHeader } from "@/src/components/dashboard-ui";
import { LinkedAccountsPanel } from "@/src/components/linked-accounts-panel";
import { getPublicConfig } from "@/src/lib/config";
import { isLocale, locales, type Locale } from "@/src/i18n/routing";

type Props = { params: Promise<{ locale: string }> };

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) return {};
  const config = await getPublicConfig(rawLocale);
  return { title: config.dashboard.account.title };
}

export default async function AccountPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { account } = config.dashboard;

  return (
    <div className="space-y-6">
      <PageHeader subtitle={account.subtitle} title={account.title} />
      <Suspense fallback={null}>
        <LinkedAccountsPanel labels={account} />
      </Suspense>
    </div>
  );
}
