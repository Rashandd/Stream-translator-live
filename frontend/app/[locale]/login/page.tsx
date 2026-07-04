import type { Metadata } from "next";
import Image from "next/image";
import Link from "next/link";
import { notFound } from "next/navigation";
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

          <form action={localizePath(locale, "/login/verify")} className="mt-6 space-y-4" method="get">
            <div>
              <label className="text-sm font-semibold text-ink" htmlFor="email">{login.emailLabel}</label>
              <input
                autoComplete="email"
                className="mt-1.5 h-11 w-full rounded-lg border border-border bg-white px-3 text-sm text-ink outline-none transition focus:border-blue"
                id="email"
                name="email"
                placeholder={login.emailPlaceholder}
                required
                type="email"
              />
            </div>
            <div>
              <div className="flex items-center justify-between">
                <label className="text-sm font-semibold text-ink" htmlFor="password">{login.passwordLabel}</label>
                <Link className="text-sm font-semibold text-blue hover:text-ink" href={localizePath(locale, "/login/reset")}>
                  {login.forgotLabel}
                </Link>
              </div>
              {/* name yok: form GET ile gezindiği için şifre asla URL'e yazılmamalı. */}
              <input
                autoComplete="current-password"
                className="mt-1.5 h-11 w-full rounded-lg border border-border bg-white px-3 text-sm text-ink outline-none transition focus:border-blue"
                id="password"
                placeholder={login.passwordPlaceholder}
                required
                type="password"
              />
            </div>
            <button
              className="inline-flex h-11 w-full items-center justify-center rounded-full bg-ink px-5 text-sm font-semibold text-white transition hover:bg-text"
              type="submit"
            >
              {login.submitLabel}
            </button>
          </form>

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
