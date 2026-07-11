import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { AdminLoginForm } from "@/src/components/admin-login-form";
import { BrandLogo } from "@/src/components/brand-logo";
import { Icon } from "@/src/components/icon";
import { getPublicConfig } from "@/src/lib/config";
import { isLocale, localizePath, locales, type Locale } from "@/src/i18n/routing";

type Props = {
  params: Promise<{ locale: string }>;
  searchParams: Promise<{ auth_error?: string }>;
};

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) return {};
  const config = await getPublicConfig(rawLocale);
  return { title: config.admin.login.title };
}

export default async function AdminLoginPage({ params, searchParams }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { login } = config.admin;
  const { auth_error: authError } = await searchParams;
  const returnMessage =
    authError === "session"
      ? config.auth.errors.sessionExpired
      : authError === "network"
        ? config.auth.errors.sessionUnavailable
        : null;

  return (
    <main className="flex min-h-screen items-center justify-center bg-canvas px-4 py-12">
      <div className="w-full max-w-md">
        <Link className="mb-8 flex justify-center" href={localizePath(locale, "/")}>
          <BrandLogo
            className="h-14 w-auto object-contain"
            config={config}
            height={545}
            priority
            width={2253}
          />
        </Link>

        <div className="rounded-2xl border border-border bg-surface p-6 sm:p-8">
          <span className="inline-flex rounded-full border border-violet/30 bg-violet/10 px-2.5 py-0.5 text-xs font-semibold text-ink">
            {config.admin.topbar.roleLabel}
          </span>
          <h1 className="mt-3 text-2xl font-semibold text-ink">{login.title}</h1>
          <p className="mt-2 text-sm leading-6 text-muted">{login.subtitle}</p>

          {returnMessage ? (
            <p
              className="mt-4 rounded-lg border border-danger/25 bg-danger/10 px-3 py-2 text-sm text-ink"
              role="alert"
            >
              {returnMessage}
            </p>
          ) : null}

          <AdminLoginForm
            labels={{
              emailLabel: login.emailLabel,
              emailPlaceholder: login.emailPlaceholder,
              passwordLabel: login.passwordLabel,
              passwordPlaceholder: login.passwordPlaceholder,
              submitLabel: login.submitLabel,
              submittingLabel: login.submittingLabel,
              errorInvalid: login.errorInvalid,
              errorUnavailable: login.errorUnavailable,
              errorNetwork: login.errorNetwork,
            }}
            redirectTo={localizePath(locale, "/admin")}
          />

          <p className="mt-4 flex items-center gap-2 text-sm leading-6 text-muted">
            <Icon className="h-4 w-4 shrink-0 text-blue" name="ShieldCheck" />
            {login.twofaNote}
          </p>
        </div>
      </div>
    </main>
  );
}
