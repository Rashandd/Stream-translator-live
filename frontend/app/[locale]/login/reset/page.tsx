import Image from "next/image";
import Link from "next/link";
import { notFound } from "next/navigation";
import { getPublicConfig } from "@/src/lib/config";
import { isLocale, localizePath, locales, type Locale } from "@/src/i18n/routing";

type Props = { params: Promise<{ locale: string }> };

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export default async function ResetPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { reset } = config.auth;

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
          <h1 className="text-2xl font-semibold text-ink">{reset.title}</h1>
          <p className="mt-2 text-sm leading-6 text-muted">{reset.subtitle}</p>

          <form action={localizePath(locale, "/login")} className="mt-6 space-y-4" method="get">
            <div>
              <label className="text-sm font-semibold text-ink" htmlFor="email">{reset.emailLabel}</label>
              <input
                autoComplete="email"
                className="mt-1.5 h-11 w-full rounded-lg border border-border bg-white px-3 text-sm text-ink outline-none transition focus:border-blue"
                id="email"
                name="email"
                placeholder={reset.emailPlaceholder}
                required
                type="email"
              />
            </div>
            <button
              className="inline-flex h-11 w-full items-center justify-center rounded-full bg-ink px-5 text-sm font-semibold text-white transition hover:bg-text"
              type="submit"
            >
              {reset.submitLabel}
            </button>
          </form>
        </div>

        <p className="mt-6 text-center text-sm">
          <Link className="font-semibold text-muted hover:text-ink" href={localizePath(locale, "/login")}>
            {reset.backLabel}
          </Link>
        </p>
      </div>
    </main>
  );
}
