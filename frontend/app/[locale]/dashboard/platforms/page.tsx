import { notFound } from "next/navigation";
import { Icon } from "@/src/components/icon";
import { PageHeader } from "@/src/components/dashboard-ui";
import { getPublicConfig } from "@/src/lib/config";
import { isLocale, locales, type Locale } from "@/src/i18n/routing";

type Props = { params: Promise<{ locale: string }> };

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export default async function PlatformsPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { platforms } = config.dashboard;
  const { extension } = platforms;

  return (
    <div className="space-y-6">
      <PageHeader subtitle={platforms.subtitle} title={platforms.title} />

      <div className="flex items-start gap-3 rounded-xl border border-border bg-white p-4">
        <Icon className="mt-0.5 h-5 w-5 shrink-0 text-blue" name="LockKeyhole" />
        <p className="text-sm leading-6 text-muted">{platforms.oauthNotice}</p>
      </div>

      <ul className="grid gap-4 sm:grid-cols-2">
        {platforms.accounts.map((account) => (
          <li className="flex flex-col gap-4 rounded-xl border border-border bg-white p-5" key={account.name}>
            <div className="flex items-start justify-between gap-3">
              <div className="flex min-w-0 items-center gap-3">
                <span className="flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-canvas text-ink">
                  <Icon className="h-5 w-5" name={account.icon} />
                </span>
                <div className="min-w-0">
                  <p className="truncate text-sm font-semibold text-ink">{account.name}</p>
                  <p className="truncate text-sm text-muted">{account.detail}</p>
                </div>
              </div>
              <span
                className={`inline-flex shrink-0 items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs font-semibold ${
                  account.connected ? "border-success/25 bg-success/10 text-ink" : "border-border bg-canvas text-muted"
                }`}
              >
                <span className={`h-1.5 w-1.5 rounded-full ${account.connected ? "bg-success" : "bg-faint"}`} />
                {account.connected ? platforms.connectedLabel : platforms.notConnectedLabel}
              </span>
            </div>

            <div className="rounded-lg border border-hairline bg-canvas px-3 py-2">
              <p className="text-xs font-medium text-muted">{platforms.scopesLabel}</p>
              <p className="mt-0.5 text-sm text-ink">{account.connected && account.account ? account.account : account.scopes}</p>
            </div>

            <button
              className={`inline-flex h-10 items-center justify-center gap-1.5 rounded-full px-4 text-sm font-semibold transition ${
                account.connected
                  ? "border border-border bg-white text-ink hover:border-danger hover:text-danger"
                  : "bg-ink text-white hover:bg-text"
              }`}
              type="button"
            >
              {account.connected ? (
                platforms.disconnectLabel
              ) : (
                <>
                  <Icon className="h-4 w-4" name="Plus" />
                  {platforms.connectLabel}
                </>
              )}
            </button>
          </li>
        ))}
      </ul>

      <section className="flex flex-col gap-4 rounded-xl border border-border bg-white p-5">
        <div className="flex items-start justify-between gap-3">
          <div className="flex min-w-0 items-center gap-3">
            <span className="flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-canvas text-ink">
              <Icon className="h-5 w-5" name={extension.icon} />
            </span>
            <div className="min-w-0">
              <p className="text-sm font-semibold text-ink">{extension.title}</p>
              <p className="text-sm text-muted">{extension.summary}</p>
            </div>
          </div>
          <span className="inline-flex shrink-0 items-center gap-1.5 rounded-full border border-border bg-canvas px-2.5 py-1 text-xs font-semibold text-muted">
            <span className="h-1.5 w-1.5 rounded-full bg-faint" />
            {extension.statusLabel}
          </span>
        </div>

        <div className="rounded-lg border border-hairline bg-canvas px-3 py-2">
          <p className="text-xs font-medium text-muted">{extension.stepsLabel}</p>
          <ol className="mt-1 list-decimal space-y-0.5 pl-4 text-sm text-ink">
            {extension.steps.map((step) => (
              <li key={step}>{step}</li>
            ))}
          </ol>
        </div>

        <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
          <button
            className="inline-flex h-10 items-center justify-center gap-1.5 rounded-full bg-ink px-4 text-sm font-semibold text-white opacity-50"
            disabled
            type="button"
          >
            <Icon className="h-4 w-4" name="KeyRound" />
            {extension.codeButton}
          </button>
          <p className="text-xs text-muted sm:max-w-md">{extension.codeNote}</p>
        </div>
      </section>
    </div>
  );
}
