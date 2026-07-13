import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { BrandLogo } from "@/src/components/brand-logo";
import { VerifyCodeForm } from "@/src/components/verify-code-form";
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
  return { title: config.admin.verify.title };
}

export default async function AdminVerifyPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { verify } = config.admin;

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
          <h1 className="text-2xl font-semibold text-ink">{verify.title}</h1>
          <p className="mt-2 text-sm leading-6 text-muted">{verify.subtitle}</p>

          <VerifyCodeForm
            adminHref={localizePath(locale, "/admin")}
            dashboardHref={localizePath(locale, "/dashboard")}
            labels={{
              codeLabel: verify.codeLabel,
              codePlaceholder: verify.codePlaceholder,
              submitLabel: verify.submitLabel,
              submittingLabel: verify.submittingLabel,
              resendLabel: verify.resendLabel,
              resendingLabel: verify.resendingLabel,
              resendSent: verify.resendSent,
              errorInvalid: config.auth.errors.codeInvalid,
              errorTooMany: config.auth.errors.codeTooMany,
              errorNetwork: config.auth.errors.network,
              errorSessionMissing: config.auth.errors.sessionMissing,
            }}
          />

          <div className="mt-4 flex items-center justify-end text-sm">
            <Link className="font-semibold text-muted hover:text-ink" href={localizePath(locale, "/admin/login")}>
              {verify.backLabel}
            </Link>
          </div>
        </div>
      </div>
    </main>
  );
}
