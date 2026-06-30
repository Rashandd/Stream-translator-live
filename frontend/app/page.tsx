import Link from "next/link";
import { Icon } from "@/src/components/icon";
import { SiteHeader } from "@/src/components/site-header";
import { StatusPill } from "@/src/components/status-pill";
import { getPublicConfig } from "@/src/lib/config";

export default async function HomePage() {
  const config = await getPublicConfig();

  return (
    <main className="min-h-screen overflow-hidden">
      <SiteHeader config={config} />

      <section className="mx-auto grid w-full max-w-7xl items-center gap-10 px-4 pb-14 pt-8 sm:px-6 lg:grid-cols-[1fr_0.86fr] lg:px-8 lg:pb-20 lg:pt-16">
        <div className="max-w-3xl">
          <div className="mb-6 inline-flex rounded-full border border-cyan/25 bg-white/80 px-4 py-2 text-sm font-semibold text-ink shadow-sm">
            {config.home.hero.eyebrow}
          </div>
          <h1 className="max-w-4xl text-balance text-5xl font-semibold leading-[1.02] tracking-normal text-ink sm:text-6xl lg:text-7xl">
            {config.home.hero.title}
          </h1>
          <p className="mt-6 max-w-2xl text-lg leading-8 text-muted">
            {config.home.hero.summary}
          </p>
          <div className="mt-8 flex flex-col gap-3 sm:flex-row">
            <Link className="inline-flex h-12 items-center justify-center rounded-full bg-ink px-6 text-sm font-semibold text-white transition hover:bg-text" href={config.home.hero.primaryCta.href}>
              {config.home.hero.primaryCta.label}
            </Link>
            <Link className="inline-flex h-12 items-center justify-center rounded-full border border-border bg-white px-6 text-sm font-semibold text-ink transition hover:border-cyan hover:bg-cyan/10" href={config.home.hero.secondaryCta.href}>
              {config.home.hero.secondaryCta.label}
            </Link>
          </div>

          <div className="mt-10 grid gap-3 sm:grid-cols-3">
            {config.home.metrics.map((metric) => (
              <div className="rounded-lg border border-hairline bg-white/80 p-4 shadow-sm backdrop-blur" key={metric.label}>
                <p className="text-sm font-medium text-muted">{metric.label}</p>
                <p className="mt-2 font-mono text-xl font-semibold text-ink">{metric.value}</p>
                <p className="mt-1 text-sm text-muted">{metric.hint}</p>
              </div>
            ))}
          </div>
        </div>

        <div className="relative">
          <div className="absolute -inset-10 rounded-full bg-[radial-gradient(circle,rgba(54,216,228,0.34),rgba(154,140,255,0.16)_42%,transparent_70%)] blur-2xl" />
          <div className="relative overflow-hidden rounded-xl border border-border bg-white shadow-panel">
            <div className="flex items-center justify-between border-b border-hairline px-5 py-4">
              <div>
                <p className="text-sm font-semibold text-ink">{config.home.dashboardPreview.title}</p>
                <p className="text-sm text-muted">{config.home.dashboardPreview.status}</p>
              </div>
              <div className="flex h-10 w-10 items-center justify-center rounded-full bg-cyan/15 text-ink">
                <Icon className="h-5 w-5" name="Radio" />
              </div>
            </div>
            <div className="space-y-3 p-5">
              {config.home.dashboardPreview.rows.map((row) => (
                <div className="flex items-center justify-between rounded-lg border border-hairline bg-canvas px-4 py-3" key={row.label}>
                  <span className="text-sm text-muted">{row.label}</span>
                  <span className="font-mono text-sm font-semibold text-ink">{row.value}</span>
                </div>
              ))}
            </div>
            <div className="grid grid-cols-3 gap-px bg-hairline">
              {config.home.pipeline.map((step) => (
                <div className="bg-white p-4" key={step.title}>
                  <Icon className="mb-4 h-5 w-5 text-blue" name={step.icon} />
                  <p className="text-sm font-semibold text-ink">{step.title}</p>
                  <p className="mt-2 text-sm leading-6 text-muted">{step.body}</p>
                </div>
              ))}
            </div>
          </div>
        </div>
      </section>

      <section className="border-y border-hairline bg-white/70 py-14" id={config.home.productSection.id}>
        <div className="mx-auto grid max-w-7xl gap-4 px-4 sm:px-6 md:grid-cols-3 lg:px-8">
          {config.home.pipeline.map((item) => (
            <article className="rounded-lg border border-border bg-white p-6" key={item.title}>
              <Icon className="h-6 w-6 text-blue" name={item.icon} />
              <h2 className="mt-5 text-xl font-semibold text-ink">{item.title}</h2>
              <p className="mt-3 text-sm leading-6 text-muted">{item.body}</p>
            </article>
          ))}
        </div>
      </section>

      <section className="mx-auto max-w-7xl px-4 py-16 sm:px-6 lg:px-8" id={config.home.solutionsSection.id}>
        <div className="mb-8 flex flex-col justify-between gap-4 md:flex-row md:items-end">
          <div>
            <p className="text-sm font-semibold uppercase text-blue">{config.home.solutionsSection.eyebrow}</p>
            <h2 className="mt-3 text-3xl font-semibold text-ink">{config.home.solutionsSection.title}</h2>
          </div>
          <p className="max-w-xl text-sm leading-6 text-muted">
            {config.home.solutionsSection.summary}
          </p>
        </div>
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {config.solutions.map((solution) => (
            <article className="rounded-lg border border-border bg-white p-5 shadow-sm" key={solution.title}>
              <div className="flex items-start justify-between gap-4">
                <div className="flex h-11 w-11 items-center justify-center rounded-full bg-cyan/12 text-ink">
                  <Icon className="h-5 w-5" name={solution.icon} />
                </div>
                <StatusPill label={solution.status} text={config.statusLabels[solution.status as keyof typeof config.statusLabels]} />
              </div>
              <h3 className="mt-5 text-lg font-semibold text-ink">{solution.title}</h3>
              <p className="mt-2 text-sm leading-6 text-muted">{solution.summary}</p>
            </article>
          ))}
        </div>
      </section>

      <section className="border-t border-hairline bg-ink py-14 text-white" id={config.security.id}>
        <div className="mx-auto grid max-w-7xl gap-8 px-4 sm:px-6 md:grid-cols-[0.7fr_1fr] lg:px-8">
          <div>
            <Icon className="h-7 w-7 text-cyan" name="ShieldCheck" />
            <h2 className="mt-5 text-3xl font-semibold">{config.security.title}</h2>
          </div>
          <div className="grid gap-3">
            {config.security.items.map((item) => (
              <div className="flex gap-3 rounded-lg border border-white/10 bg-white/5 p-4" key={item}>
                <Icon className="mt-0.5 h-5 w-5 shrink-0 text-cyan" name="CheckCircle2" />
                <p className="text-sm leading-6 text-white/78">{item}</p>
              </div>
            ))}
          </div>
        </div>
      </section>
    </main>
  );
}
