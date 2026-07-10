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
  const localizeLinks = (links: { label: string; href: string }[]) =>
    links.map((link) => ({
      ...link,
      href: localizePath(locale, link.href),
    }));

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

        <div className="rounded-2xl border border-border bg-surface p-6 sm:p-8">
          <h1 className="text-2xl font-semibold text-ink">{signup.title}</h1>

          <SignupForm
            dashboardHref={localizePath(locale, "/dashboard")}
            labels={{
              emailLabel: signup.emailLabel,
              passwordLabel: signup.passwordLabel,
              submitLabel: signup.submitLabel,
              submittingLabel: signup.submittingLabel,
              errorTaken: config.auth.errors.emailTaken,
              errorInvalidFields: config.auth.errors.invalidFields,
              errorNetwork: config.auth.errors.network,
              requiredConsent: {
                ...signup.requiredConsent,
                links: localizeLinks(signup.requiredConsent.links),
              },
              optionalConsent: {
                ...signup.optionalConsent,
                links: localizeLinks(signup.optionalConsent.links),
              },
            }}
            verifyHref={localizePath(locale, "/login/verify")}
          />

          <OAuthButtons labels={config.auth.social} />
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
