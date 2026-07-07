import type { Metadata } from "next";
import Image from "next/image";
import Link from "next/link";
import { notFound } from "next/navigation";
import { OAuthButtons } from "@/src/components/oauth-buttons";
import { SignupForm } from "@/src/components/signup-form";
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
  return { title: config.auth.signup.title };
}

export default async function SignupPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { signup } = config.auth;

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
          <h1 className="text-2xl font-semibold text-ink">{signup.title}</h1>
          <p className="mt-2 text-sm leading-6 text-muted">{signup.subtitle}</p>

          <SignupForm
            dashboardHref={localizePath(locale, "/dashboard")}
            labels={{
              emailLabel: signup.emailLabel,
              emailPlaceholder: signup.emailPlaceholder,
              passwordLabel: signup.passwordLabel,
              passwordPlaceholder: signup.passwordPlaceholder,
              submitLabel: signup.submitLabel,
              submittingLabel: signup.submittingLabel,
              errorTaken: config.auth.errors.emailTaken,
              errorInvalidFields: config.auth.errors.invalidFields,
              errorNetwork: config.auth.errors.network,
            }}
            verifyHref={localizePath(locale, "/login/verify")}
          />

          <OAuthButtons labels={config.auth.social} />

          <p className="mt-4 text-xs leading-5 text-muted">{signup.termsNote}</p>
        </div>

        <p className="mt-6 text-center text-sm text-muted">
          {signup.loginPrompt}{" "}
          <Link className="font-semibold text-blue hover:text-ink" href={localizePath(locale, "/login")}>
            {signup.loginLabel}
          </Link>
        </p>
      </div>
    </main>
  );
}
