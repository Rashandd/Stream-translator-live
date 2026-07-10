import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { SiteFooter } from "@/src/components/site-footer";
import { SiteHeader } from "@/src/components/site-header";
import en from "@/src/i18n/dictionaries/en.json";
import { isLocale, localizePath, locales, type Locale } from "@/src/i18n/routing";
import { API_URL } from "@/src/lib/api";
import { getPublicConfig } from "@/src/lib/config";
import { sanitizeLegalHtml } from "@/src/lib/legal-html";

type Props = {
  params: Promise<{
    locale: string;
    slug: string;
  }>;
};

export function generateStaticParams() {
  return locales.flatMap((locale) =>
    en.legal.pages.map((page) => ({
      locale,
      slug: page.slug,
    })),
  );
}

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const { locale: rawLocale, slug } = await params;
  if (!isLocale(rawLocale)) return {};

  const config = await getPublicConfig(rawLocale);
  const page = config.legal.pages.find((candidate) => candidate.slug === slug);

  if (!page) return {};

  return {
    title: page.title,
    description: page.summary,
  };
}

async function loadRuntimeConfig(): Promise<Record<string, string> | null> {
  try {
    const response = await fetch(`${API_URL}/config`, { cache: "no-store" });
    if (!response.ok) {
      return null;
    }
    return (await response.json()) as Record<string, string>;
  } catch {
    return null;
  }
}

export default async function LegalPage({ params }: Props) {
  const { locale: rawLocale, slug } = await params;

  if (!isLocale(rawLocale)) {
    notFound();
  }

  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const page = config.legal.pages.find((candidate) => candidate.slug === slug);

  if (!page) {
    notFound();
  }

  const runtimeConfig = await loadRuntimeConfig();
  const runtimeHtml = sanitizeLegalHtml(runtimeConfig?.[`legal.${slug}.html`] ?? "").trim() || null;
  const legalName = runtimeConfig?.["company.legal_name"]?.trim() || "";
  const registrationNumber = runtimeConfig?.["company.registration_number"]?.trim() || "";
  const registeredAddress = runtimeConfig?.["company.registered_address"]?.trim() || "";

  return (
    <main className="min-h-screen bg-surface">
      <SiteHeader config={config} locale={locale} />

      <article className="mx-auto w-full max-w-4xl px-4 py-12 sm:px-6 lg:px-8 lg:py-16">
        <Link
          className="text-sm font-semibold text-blue hover:text-ink"
          href={localizePath(locale, "/")}
        >
          {config.legal.backLabel}
        </Link>

        <p className="mt-8 text-sm font-semibold uppercase tracking-wide text-blue">
          {config.legal.eyebrow}
        </p>
        <h1 className="mt-3 text-4xl font-semibold leading-tight text-ink sm:text-5xl">
          {page.title}
        </h1>
        <p className="mt-5 text-lg leading-8 text-muted">{page.summary}</p>
        <p className="mt-4 text-sm text-faint">
          {config.legal.updatedLabel}: {config.legal.effectiveDate}
        </p>

        {runtimeHtml ? (
          <div className="mt-10 border-t border-hairline pt-10">
            <div className="legal-html" dangerouslySetInnerHTML={{ __html: runtimeHtml }} />
          </div>
        ) : (
          <div className="mt-10 space-y-10 border-t border-hairline pt-10">
            {page.sections.map((section) => (
              <section key={section.title}>
                <h2 className="text-2xl font-semibold text-ink">{section.title}</h2>
                <p className="mt-3 text-base leading-8 text-muted">{section.body}</p>
              </section>
            ))}
          </div>
        )}

        {legalName ? (
          <div className="mt-10 border-t border-hairline pt-8 text-sm text-muted">
            <p className="font-semibold text-ink">{config.legal.entityHeading}</p>
            <p className="mt-2">{legalName}</p>
            {registrationNumber ? (
              <p>
                {config.legal.entityRegistrationLabel}: {registrationNumber}
              </p>
            ) : null}
            {registeredAddress ? <p>{registeredAddress}</p> : null}
          </div>
        ) : null}
      </article>

      <SiteFooter config={config} locale={locale} />
    </main>
  );
}
