import Link from "next/link";
import { notFound } from "next/navigation";
import { Icon } from "@/src/components/icon";
import { SiteHeader } from "@/src/components/site-header";
import { getPublicConfig } from "@/src/lib/config";
import { isLocale, localizePath, locales, type Locale } from "@/src/i18n/routing";

type HomePageProps = {
  params: Promise<{
    locale: string;
  }>;
};

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export default async function HomePage({ params }: HomePageProps) {
  const { locale: rawLocale } = await params;

  if (!isLocale(rawLocale)) {
    notFound();
  }

  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);

  return (
    <main className="min-h-screen overflow-hidden">
      <SiteHeader config={config} locale={locale} />

      <section className="mx-auto grid w-full max-w-7xl items-center gap-10 px-4 pb-16 pt-12 sm:px-6 lg:grid-cols-[0.92fr_1fr] lg:px-8 lg:pb-24 lg:pt-20">
        <div className="max-w-3xl">
          <div className="mb-6 inline-flex rounded-full border border-cyan/25 bg-white/80 px-4 py-2 text-sm font-semibold text-ink shadow-sm">
            {config.home.hero.eyebrow}
          </div>
          <h1 className="max-w-4xl text-balance text-5xl font-semibold leading-[1.03] tracking-normal text-ink sm:text-6xl lg:text-7xl">
            {config.home.hero.title}
          </h1>
          <p className="mt-6 max-w-2xl text-lg leading-8 text-muted">
            {config.home.hero.summary}
          </p>
          <div className="mt-8 flex flex-col gap-3 sm:flex-row">
            <Link className="inline-flex h-12 items-center justify-center rounded-full bg-ink px-6 text-sm font-semibold text-white transition hover:bg-text" href={localizePath(locale, config.home.hero.primaryCta.href)}>
              {config.home.hero.primaryCta.label}
            </Link>
            <Link className="inline-flex h-12 items-center justify-center rounded-full border border-border bg-white px-6 text-sm font-semibold text-ink transition hover:border-cyan hover:bg-cyan/10" href={localizePath(locale, config.home.hero.secondaryCta.href)}>
              {config.home.hero.secondaryCta.label}
            </Link>
          </div>
        </div>

        <div className="relative min-h-[360px]">
          <div className="absolute inset-0 rounded-[32px] bg-[radial-gradient(circle_at_30%_25%,rgba(54,216,228,0.34),transparent_32%),radial-gradient(circle_at_68%_70%,rgba(154,140,255,0.24),transparent_34%),linear-gradient(135deg,rgba(255,255,255,0.92),rgba(238,246,251,0.86))] shadow-panel" />
          <div className="relative flex min-h-[360px] flex-col justify-between rounded-[32px] border border-border bg-white/58 p-6 backdrop-blur">
            <div className="flex items-center justify-between">
              <span className="rounded-full border border-cyan/25 bg-white/80 px-3 py-1.5 text-sm font-semibold text-ink">
                {config.home.scene.label}
              </span>
              <Icon className="h-6 w-6 text-blue" name="AudioLines" />
            </div>
            <div className="mx-auto flex h-36 w-full max-w-md items-center justify-center rounded-full border border-white/70 bg-white/42">
              <div className="h-20 w-20 rounded-full border border-cyan/35 bg-cyan/20" />
              <div className="-ml-3 h-24 w-24 rounded-full border border-blue/25 bg-blue/15" />
              <div className="-ml-4 h-16 w-16 rounded-full border border-violet/35 bg-violet/20" />
            </div>
            <div>
              <p className="font-mono text-sm font-semibold text-blue">{config.home.scene.subtitle}</p>
              <h2 className="mt-2 text-3xl font-semibold text-ink">{config.home.scene.title}</h2>
              <div className="mt-5 grid gap-2">
                {config.home.scene.items.map((item) => (
                  <div className="flex items-center gap-3 rounded-full border border-white/70 bg-white/78 px-4 py-3 text-sm font-medium text-muted" key={item}>
                    <Icon className="h-4 w-4 shrink-0 text-blue" name="CheckCircle2" />
                    {item}
                  </div>
                ))}
              </div>
            </div>
          </div>
        </div>
      </section>
    </main>
  );
}
