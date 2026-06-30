import { Icon } from "@/src/components/icon";
import { SiteHeader } from "@/src/components/site-header";
import { getPublicConfig } from "@/src/lib/config";

export default async function DashboardPage() {
  const config = await getPublicConfig();

  return (
    <main className="min-h-screen">
      <SiteHeader config={config} />

      <section className="mx-auto w-full max-w-7xl px-4 pb-16 pt-8 sm:px-6 lg:px-8">
        <div className="mb-8 flex flex-col justify-between gap-4 lg:flex-row lg:items-end">
          <div>
            <p className="text-sm font-semibold uppercase text-blue">{config.brand.name}</p>
            <h1 className="mt-3 text-4xl font-semibold tracking-normal text-ink">{config.dashboard.title}</h1>
            <p className="mt-3 max-w-3xl text-base leading-7 text-muted">{config.dashboard.subtitle}</p>
          </div>
          <div className="rounded-full border border-cyan/25 bg-white px-4 py-2 font-mono text-sm font-semibold text-ink shadow-sm">
            {config.dashboard.previewLabel}
          </div>
        </div>

        <div className="grid gap-4 md:grid-cols-4">
          {config.dashboard.statusCards.map((card) => (
            <article className="rounded-lg border border-border bg-white p-5 shadow-sm" key={card.label}>
              <p className="text-sm font-medium text-muted">{card.label}</p>
              <p className="mt-3 font-mono text-2xl font-semibold text-ink">{card.value}</p>
              <p className="mt-2 text-sm leading-6 text-muted">{card.hint}</p>
            </article>
          ))}
        </div>

        <div className="mt-6 grid gap-6 lg:grid-cols-[0.72fr_1fr]">
          <section className="rounded-xl border border-border bg-white p-5 shadow-sm">
            <div className="mb-5 flex items-center justify-between">
              <h2 className="text-xl font-semibold text-ink">{config.dashboard.quickStartTitle}</h2>
              <Icon className="h-5 w-5 text-blue" name="Route" />
            </div>
            <div className="space-y-3">
              {config.dashboard.quickActions.map((action) => (
                <button className="flex w-full items-center gap-4 rounded-lg border border-hairline bg-canvas p-4 text-left transition hover:border-cyan hover:bg-cyan/10" key={action.label} type="button">
                  <span className="flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-white text-ink">
                    <Icon className="h-5 w-5" name={action.icon} />
                  </span>
                  <span>
                    <span className="block text-sm font-semibold text-ink">{action.label}</span>
                    <span className="mt-1 block text-sm leading-6 text-muted">{action.description}</span>
                  </span>
                </button>
              ))}
            </div>
          </section>

          <section className="overflow-hidden rounded-xl border border-border bg-white shadow-sm">
            <div className="flex items-center justify-between border-b border-hairline px-5 py-4">
              <div>
                <h2 className="text-xl font-semibold text-ink">{config.dashboard.pipelineTitle}</h2>
                <p className="mt-1 text-sm text-muted">{config.dashboard.pipelineSubtitle}</p>
              </div>
              <span className="rounded-full border border-warning/25 bg-warning/10 px-3 py-1 text-xs font-semibold text-ink">{config.dashboard.pipelineStatus}</span>
            </div>
            <div className="grid gap-px bg-hairline md:grid-cols-3">
              {config.home.pipeline.map((step) => (
                <div className="bg-white p-6" key={step.title}>
                  <Icon className="h-6 w-6 text-blue" name={step.icon} />
                  <h3 className="mt-5 text-lg font-semibold text-ink">{step.title}</h3>
                  <p className="mt-3 text-sm leading-6 text-muted">{step.body}</p>
                </div>
              ))}
            </div>
          </section>
        </div>
      </section>
    </main>
  );
}
