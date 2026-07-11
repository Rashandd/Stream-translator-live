import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { BrandLogo } from "@/src/components/brand-logo";
import { Icon } from "@/src/components/icon";
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

          <div className="mt-4 flex items-start gap-3 rounded-lg border border-warning/30 bg-warning/10 p-3">
            <Icon className="mt-0.5 h-4 w-4 shrink-0 text-warning" name="Mail" />
            <p className="text-xs leading-5 text-text">{verify.devNotice}</p>
          </div>

          <form action={localizePath(locale, "/admin")} className="mt-6 space-y-4" method="get">
            <div>
              <label className="text-sm font-semibold text-ink" htmlFor="code">{verify.codeLabel}</label>
              <input
                autoComplete="one-time-code"
                className="mt-1.5 h-12 w-full rounded-lg border border-border bg-surface px-3 text-center font-mono text-lg tracking-[0.4em] text-ink outline-none transition focus:border-blue"
                id="code"
                inputMode="numeric"
                maxLength={6}
                name="code"
                pattern="[0-9]*"
                placeholder={verify.codePlaceholder}
                type="text"
              />
            </div>
            <button
              className="inline-flex h-11 w-full items-center justify-center rounded-full bg-primary px-5 text-sm font-semibold text-on-primary transition hover:bg-primary-hover"
              type="submit"
            >
              {verify.submitLabel}
            </button>
          </form>

          <div className="mt-4 flex items-center justify-between text-sm">
            <Link className="font-semibold text-blue hover:text-ink" href={localizePath(locale, "/admin/login/verify")}>
              {verify.resendLabel}
            </Link>
            <Link className="font-semibold text-muted hover:text-ink" href={localizePath(locale, "/admin/login")}>
              {verify.backLabel}
            </Link>
          </div>
        </div>
      </div>
    </main>
  );
}
