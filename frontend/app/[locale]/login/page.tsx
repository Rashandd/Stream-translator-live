import type { Metadata } from "next";
import Image from "next/image";
import Link from "next/link";
import { notFound } from "next/navigation";
import { Icon } from "@/src/components/icon";
import { LoginForm } from "@/src/components/login-form";
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
          <Image
            alt={config.brand.name}
            className="h-14 w-auto object-contain"
            height={545}
            priority
            src={config.brand.logoAsset}
            width={2253}
          />
        </Link>

        <div className="rounded-2xl border border-border bg-white p-6 sm:p-8">
          <h1 className="text-2xl font-semibold text-ink">{login.title}</h1>
          <p className="mt-2 text-sm leading-6 text-muted">{login.subtitle}</p>

          <LoginForm
            adminHref={localizePath(locale, "/admin")}
            dashboardHref={localizePath(locale, "/dashboard")}
            forgotHref={localizePath(locale, "/login/reset")}
            labels={{
              emailLabel: login.emailLabel,
              emailPlaceholder: login.emailPlaceholder,
              passwordLabel: login.passwordLabel,
              passwordPlaceholder: login.passwordPlaceholder,
              forgotLabel: login.forgotLabel,
              submitLabel: login.submitLabel,
              submittingLabel: login.submittingLabel,
              errorInvalid: config.auth.errors.invalid,
              errorUnavailable: config.auth.errors.unavailable,
              errorNetwork: config.auth.errors.network,
            }}
            verifyHref={localizePath(locale, "/login/verify")}
          />

          <p className="mt-4 flex items-center gap-2 text-sm leading-6 text-muted">
            <Icon className="h-4 w-4 shrink-0 text-blue" name="ShieldCheck" />
            {login.twofaNote}
          </p>
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
