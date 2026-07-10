import type { Metadata } from "next";
import Image from "next/image";
import Link from "next/link";
import { notFound } from "next/navigation";
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
  return { title: config.auth.verify.title };
}

export default async function VerifyPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { verify } = config.auth;

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
          <h1 className="text-2xl font-semibold text-ink">{verify.title}</h1>
          <p className="mt-2 text-sm leading-6 text-muted">{verify.subtitle}</p>

          <VerifyCodeForm
            dashboardHref={localizePath(locale, "/dashboard")}
            labels={{
              codeLabel: verify.codeLabel,
              codePlaceholder: verify.codePlaceholder,
              submitLabel: verify.submitLabel,
              submittingLabel: verify.submittingLabel,
              errorInvalid: config.auth.errors.codeInvalid,
              errorTooMany: config.auth.errors.codeTooMany,
              errorNetwork: config.auth.errors.network,
              errorSessionMissing: config.auth.errors.sessionMissing,
            }}
          />

          <div className="mt-4 flex items-center justify-end text-sm">
            <Link className="font-semibold text-muted hover:text-ink" href={localizePath(locale, "/login")}>
              {verify.backLabel}
            </Link>
          </div>
        </div>
      </div>
    </main>
  );
}
