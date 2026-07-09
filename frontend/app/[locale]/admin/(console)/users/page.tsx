import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { AdminFoundingPanel } from "@/src/components/admin-founding-panel";
import { AdminUsersPanel } from "@/src/components/admin-users-panel";
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
  return { title: config.admin.users.title };
}

export default async function AdminUsersPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { users, founding } = config.admin;

  return (
    <div className="space-y-6">
      <PageHeader subtitle={users.subtitle} title={users.title} />

      <AdminUsersPanel
        labels={{
          searchPlaceholder: users.searchPlaceholder,
          columns: users.columns,
          verifiedLabel: users.verifiedLabel,
          pendingLabel: users.pendingLabel,
          saveLabel: users.saveLabel,
          savingLabel: users.savingLabel,
          loading: users.loading,
          empty: users.empty,
          errors: users.errors,
          signInLabel: users.signInLabel,
        }}
        loginHref={localizePath(locale, "/admin/login")}
      />

      <AdminFoundingPanel labels={founding} loginHref={localizePath(locale, "/admin/login")} />
    </div>
  );
}
