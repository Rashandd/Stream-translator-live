import Link from "next/link";
import { notFound } from "next/navigation";
import { Icon } from "@/src/components/icon";
import { SiteHeader } from "@/src/components/site-header";
// Edge bölümü koyu zeminde (bg-ink) — StatusPill'in açık-yüzey renkleri yerine
// koyu zemine uygun rozet tonları.
const edgeStatusTone: Record<string, string> = {
  enabled: "border-success/45 bg-success/20 text-white",
  beta: "border-cyan/45 bg-cyan/20 text-white",
  waitlist: "border-violet/45 bg-violet/20 text-white",
  coming_soon: "border-white/25 bg-white/10 text-white/85",
  disabled: "border-white/15 bg-white/5 text-white/60",
};
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
          <p className="mt-4 text-sm leading-6 text-muted">{config.home.hero.note}</p>
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

      <section className="mx-auto w-full max-w-7xl px-4 py-16 sm:px-6 lg:px-8 lg:py-20">
        <p className="text-sm font-semibold uppercase tracking-wide text-blue">{config.home.now.eyebrow}</p>
        <h2 className="mt-3 max-w-2xl text-3xl font-semibold text-ink sm:text-4xl">{config.home.now.title}</h2>
        <div className="mt-10 grid gap-5 sm:grid-cols-2 lg:grid-cols-3">
          {config.home.now.items.map((item) => (
            <div className="rounded-xl border border-border bg-white p-6" key={item.title}>
              <Icon className="h-6 w-6 text-blue" name={item.icon} />
              <h3 className="mt-4 text-xl font-semibold text-ink">{item.title}</h3>
              <p className="mt-2 text-base leading-7 text-muted">{item.body}</p>
            </div>
          ))}
        </div>
      </section>

      <section className="bg-ink text-white">
        <div className="mx-auto w-full max-w-7xl px-4 py-16 sm:px-6 lg:px-8 lg:py-20">
          <p className="text-sm font-semibold uppercase tracking-wide text-cyan">{config.home.edge.eyebrow}</p>
          <h2 className="mt-3 max-w-3xl text-3xl font-semibold text-white sm:text-4xl">{config.home.edge.title}</h2>
          <div className="mt-10 grid gap-5 sm:grid-cols-2 lg:grid-cols-4">
            {config.home.edge.items.map((item) => (
              <div className="rounded-xl border border-white/12 bg-white/[0.06] p-6" key={item.title}>
                <div className="flex items-center justify-between gap-2">
                  <Icon className="h-6 w-6 text-cyan" name={item.icon} />
                  <span className={`inline-flex rounded-full border px-3 py-1 text-xs font-semibold ${edgeStatusTone[item.status] ?? edgeStatusTone.disabled}`}>
                    {config.statusLabels[item.status as keyof typeof config.statusLabels] ?? item.status}
                  </span>
                </div>
                <h3 className="mt-4 text-lg font-semibold text-white">{item.title}</h3>
                <p className="mt-2 text-sm leading-6 text-white/70">{item.body}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      <section className="mx-auto w-full max-w-7xl px-4 py-16 sm:px-6 lg:px-8 lg:py-20">
        <p className="text-sm font-semibold uppercase tracking-wide text-coral">{config.home.audience.eyebrow}</p>
        <h2 className="mt-3 max-w-2xl text-3xl font-semibold text-ink sm:text-4xl">{config.home.audience.title}</h2>
        <div className="mt-10 grid gap-5 sm:grid-cols-2 lg:grid-cols-4">
          {config.home.audience.items.map((item) => (
            <div className="rounded-xl border border-border bg-white p-6" key={item.title}>
              <Icon className="h-6 w-6 text-coral" name={item.icon} />
              <h3 className="mt-4 text-lg font-semibold text-ink">{item.title}</h3>
              <p className="mt-2 text-sm leading-6 text-muted">{item.body}</p>
            </div>
          ))}
        </div>
      </section>

      <section className="mx-auto w-full max-w-7xl px-4 pb-20 sm:px-6 lg:px-8">
        <div className="flex flex-col gap-6 rounded-[28px] border border-border bg-canvas p-8 sm:p-10 lg:flex-row lg:items-center lg:justify-between">
          <div className="max-w-2xl">
            <p className="text-sm font-semibold uppercase tracking-wide text-blue">{config.home.businessCta.eyebrow}</p>
            <h2 className="mt-3 text-2xl font-semibold text-ink sm:text-3xl">{config.home.businessCta.title}</h2>
            <p className="mt-3 text-base leading-7 text-muted">{config.home.businessCta.body}</p>
          </div>
          <Link
            className="inline-flex h-12 shrink-0 items-center justify-center rounded-full bg-ink px-6 text-sm font-semibold text-white transition hover:bg-text"
            href={localizePath(locale, config.home.businessCta.cta.href)}
          >
            {config.home.businessCta.cta.label}
          </Link>
        </div>
      </section>
    </main>
  );
}
