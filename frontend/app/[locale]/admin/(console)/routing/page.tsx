import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { AdminDeeplLanguagesPanel } from "@/src/components/admin-deepl-languages-panel";
import { AdminRoutingPanel } from "@/src/components/admin-routing-panel";
import { Icon } from "@/src/components/icon";
import { PageHeader, SectionCard } from "@/src/components/dashboard-ui";
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
  return { title: config.admin.routing.title };
}

export default async function AdminRoutingPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { routing } = config.admin;

  return (
    <div className="space-y-6">
      <PageHeader subtitle={routing.subtitle} title={routing.title} />

      <AdminRoutingPanel
        labels={{
          groups: routing.groups,
          loading: routing.loading,
          savingLabel: routing.savingLabel,
          errors: routing.errors,
          signInLabel: routing.signInLabel,
        }}
        loginHref={localizePath(locale, "/admin/login")}
        statusLabels={config.statusLabels}
      />

      <div className="flex items-start gap-3 rounded-xl border border-border bg-white p-4">
        <Icon className="mt-0.5 h-5 w-5 shrink-0 text-blue" name="Route" />
        <div>
          <p className="text-sm font-semibold text-ink">{routing.fallbackRuleLabel}</p>
          <p className="mt-1 text-sm leading-6 text-muted">{routing.fallbackRule}</p>
        </div>
      </div>

      <SectionCard subtitle={routing.deeplLanguages.subtitle} title={routing.deeplLanguages.title}>
        <AdminDeeplLanguagesPanel labels={routing.deeplLanguages} />
      </SectionCard>

      <SectionCard subtitle={routing.emailSenderNote} title={routing.emailSenderTitle}>
        <div className="grid gap-4 sm:grid-cols-2">
          {routing.emailSenderFields.map((field) => (
            <div key={field.label}>
              <label className="text-xs font-medium text-muted">{field.label}</label>
              <input
                className="mt-1 h-10 w-full rounded-lg border border-border bg-white px-3 text-sm text-ink outline-none transition focus:border-blue placeholder:text-faint"
                defaultValue=""
                placeholder={field.placeholder}
                type="text"
              />
            </div>
          ))}
        </div>
        <button
          className="mt-4 inline-flex h-10 items-center justify-center rounded-full bg-ink px-5 text-sm font-semibold text-white transition hover:bg-text"
          type="button"
        >
          {routing.saveLabel}
        </button>
      </SectionCard>

      <SectionCard title={routing.historyTitle}>
        <ul className="divide-y divide-hairline">
          {routing.history.map((entry) => (
            <li className="flex items-start justify-between gap-4 py-3 first:pt-0 last:pb-0" key={`${entry.when}-${entry.what}`}>
              <div className="min-w-0">
                <p className="text-sm text-ink">{entry.what}</p>
                <p className="text-xs text-muted">{entry.who}</p>
              </div>
              <span className="shrink-0 font-mono text-xs text-muted">{entry.when}</span>
            </li>
          ))}
        </ul>
      </SectionCard>
    </div>
  );
}
