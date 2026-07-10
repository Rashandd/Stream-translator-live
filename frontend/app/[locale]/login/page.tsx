import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { BrandLogo } from "@/src/components/brand-logo";
import { LoginForm } from "@/src/components/login-form";
import { OAuthButtons } from "@/src/components/oauth-buttons";
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
  return { title: config.auth.login.title };
}

export default async function LoginPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { login } = config.auth;

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
          <h1 className="text-2xl font-semibold text-ink">{login.title}</h1>
          <p className="mt-2 text-sm leading-6 text-muted">{login.subtitle}</p>

          <LoginForm
            adminHref={localizePath(locale, "/admin")}
            dashboardHref={localizePath(locale, "/dashboard")}
            forgotHref={localizePath(locale, "/login/reset")}
            labels={{
              emailLabel: login.emailLabel,
              passwordLabel: login.passwordLabel,
              forgotLabel: login.forgotLabel,
              submitLabel: login.submitLabel,
              submittingLabel: login.submittingLabel,
              errorInvalid: config.auth.errors.invalid,
              errorUnavailable: config.auth.errors.unavailable,
              errorNetwork: config.auth.errors.network,
            }}
            verifyHref={localizePath(locale, "/login/verify")}
          />

          <OAuthButtons labels={config.auth.social} />
        </div>

        <p className="mt-6 text-center text-sm text-muted">
          {login.signupPrompt}{" "}
          <Link className="font-semibold text-blue hover:text-ink" href={localizePath(locale, "/signup")}>
            {login.signupLabel}
          </Link>
        </p>
      </div>
    </main>
  );
}
