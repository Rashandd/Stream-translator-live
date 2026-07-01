import Link from "next/link";
import { notFound } from "next/navigation";
import { Icon } from "@/src/components/icon";
import { PageHeader, SectionCard } from "@/src/components/dashboard-ui";
import { getPublicConfig } from "@/src/lib/config";
import { isLocale, localizePath, locales, type Locale } from "@/src/i18n/routing";

type Props = { params: Promise<{ locale: string }> };

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

function CopyRow({ value, copyLabel }: { value: string; copyLabel: string }) {
  return (
    <div className="flex items-center gap-2 rounded-lg border border-border bg-canvas px-3 py-2">
      <code className="min-w-0 flex-1 truncate font-mono text-sm text-ink">{value}</code>
      <span className="inline-flex shrink-0 items-center gap-1.5 rounded-md border border-border bg-white px-2.5 py-1 text-xs font-semibold text-ink">
        <Icon className="h-3.5 w-3.5" name="Copy" />
        {copyLabel}
      </span>
    </div>
  );
}

export default async function GoLivePage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { goLive } = config.dashboard;
  const isCloud = goLive.activeMode === "cloud";
  const mode = isCloud ? goLive.modes.cloud : goLive.modes.relay;
  const live = mode;
  const ready = mode.steps.every((step) => step.done);

  return (
    <div className="space-y-6">
      <PageHeader subtitle={goLive.subtitle} title={goLive.title} />

      <div className="flex flex-col gap-4 rounded-xl border border-border bg-white p-5 sm:flex-row sm:items-start sm:justify-between">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <p className="text-xs font-semibold uppercase tracking-wide text-blue">{goLive.modeLabel}</p>
            <span className="rounded-full border border-cyan/25 bg-cyan/10 px-2.5 py-0.5 text-xs font-semibold text-ink">{mode.badge}</span>
          </div>
          <h2 className="mt-2 text-lg font-semibold text-ink">{mode.name}</h2>
          <p className="mt-1 max-w-2xl text-sm leading-6 text-muted">{mode.summary}</p>
        </div>
        <Link
          className="inline-flex h-9 shrink-0 items-center gap-1.5 rounded-full border border-border bg-white px-4 text-sm font-semibold text-ink transition hover:border-cyan hover:bg-cyan/10"
          href={localizePath(locale, "/dashboard/distribution")}
        >
          <Icon className="h-4 w-4" name="Route" />
          {goLive.changeModeLabel}
        </Link>
      </div>

      <div className="grid gap-6 lg:grid-cols-[1fr_0.85fr]">
        <SectionCard title={goLive.stepsTitle}>
          <ol className="space-y-4">
            {mode.steps.map((step, index) => (
              <li className="flex gap-4" key={step.title}>
                <span
                  className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-sm font-semibold ${
                    step.done ? "bg-success text-white" : "bg-canvas text-ink"
                  }`}
                >
                  {step.done ? <Icon className="h-4 w-4" name="CheckCircle2" /> : index + 1}
                </span>
                <div className="min-w-0 pt-0.5">
                  <p className="flex items-center gap-2 text-sm font-semibold text-ink">
                    <Icon className="h-4 w-4 text-blue" name={step.icon} />
                    {step.title}
                  </p>
                  <p className="mt-1 text-sm leading-6 text-muted">{step.body}</p>
                </div>
              </li>
            ))}
          </ol>
        </SectionCard>

        <div className="space-y-6">
          <SectionCard title={goLive.startTitle}>
            <p className="text-xs font-medium text-muted">{live.connectLabel}</p>
            <div className="mt-1 space-y-2">
              <CopyRow copyLabel={goLive.copyLabel} value={live.connectValue} />
              {isCloud ? <CopyRow copyLabel={goLive.copyLabel} value={goLive.modes.cloud.streamKey} /> : null}
            </div>

            <div className="mt-4 flex items-start gap-2.5 rounded-lg border border-hairline bg-canvas p-3">
              <Icon className="mt-0.5 h-4 w-4 shrink-0 text-blue" name="Video" />
              <p className="text-sm leading-6 text-muted">{live.videoNote}</p>
            </div>

            {isCloud ? (
              <div className="mt-5">
                <p className="mb-2 text-xs font-medium text-muted">{goLive.targetsLabel}</p>
                <ul className="space-y-2">
                  {goLive.targets.map((target) => (
                    <li className="flex items-center justify-between rounded-lg border border-hairline bg-canvas px-4 py-2.5" key={target.name}>
                      <span className="text-sm font-semibold text-ink">{target.name}</span>
                      <span
                        className={`inline-flex h-6 w-11 items-center rounded-full px-0.5 ${target.enabled ? "justify-end bg-success" : "justify-start bg-border"}`}
                      >
                        <span className="h-5 w-5 rounded-full bg-white" />
                      </span>
                    </li>
                  ))}
                </ul>
              </div>
            ) : null}

            <button
              aria-disabled={!ready}
              className={`mt-6 inline-flex h-11 w-full items-center justify-center gap-2 rounded-full px-5 text-sm font-semibold text-white transition ${
                ready ? "bg-ink hover:bg-text" : "cursor-not-allowed bg-faint"
              }`}
              disabled={!ready}
              type="button"
            >
              <Icon className="h-4 w-4" name="Play" />
              {live.startLabel}
            </button>
            <p className="mt-3 text-sm leading-6 text-muted">{live.startNote}</p>
          </SectionCard>
        </div>
      </div>
    </div>
  );
}
