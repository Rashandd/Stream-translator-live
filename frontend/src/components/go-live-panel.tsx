"use client";

import { useEffect, useState } from "react";
import { CopyRow } from "@/src/components/copy-row";
import { SectionCard } from "@/src/components/dashboard-ui";
import { Icon } from "@/src/components/icon";
import { userFetch } from "@/src/lib/api";
import { fetchFoundingStatus } from "@/src/lib/founding";

type Step = { title: string; body: string; icon: string };
type Target = { name: string; enabled: boolean };

type Mode = {
  steps: Step[];
  connectLabel: string;
  connectValue: string;
  streamKey?: string;
  videoNote: string;
  startLabel: string;
  startNote: string;
};

type Labels = {
  stepsTitle: string;
  startTitle: string;
  targetsLabel: string;
  targets: Target[];
  copyLabel: string;
};

type Device = { connected: boolean };
type IntegrationStatus = { connected: boolean };

/** Adım kartlarındaki hangi ikon hangi canlı sinyale karşılık gelir. */
function isStepDone(step: Step, sdkConnected: boolean, platformConnected: boolean): boolean {
  if (step.icon === "MonitorCog") return sdkConnected;
  if (step.icon === "Share2") return platformConnected;
  return false;
}

export function GoLivePanel({ labels, mode, isCloud }: { labels: Labels; mode: Mode; isCloud: boolean }) {
  const [sdkConnected, setSdkConnected] = useState(false);
  const [platformConnected, setPlatformConnected] = useState(false);
  // Gate kapalıyken (varsayılan) herkes hazır sayılır; açıksa onay şart.
  const [foundingReady, setFoundingReady] = useState(true);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const [devicesRes, integrationsRes, founding] = await Promise.all([
          userFetch("/api/sdk/devices"),
          userFetch("/api/integrations"),
          fetchFoundingStatus(),
        ]);
        if (cancelled) return;
        if (devicesRes.ok) {
          const devices = (await devicesRes.json()) as Device[];
          setSdkConnected(devices.some((d) => d.connected));
        }
        if (integrationsRes.ok) {
          const items = (await integrationsRes.json()) as IntegrationStatus[];
          setPlatformConnected(items.some((item) => item.connected));
        }
        setFoundingReady(!founding.enabled || founding.application?.status === "approved");
      } catch {
        // Best-effort: sinyaller alınamazsa buton güvenli varsayılanla (disabled) kalır.
      }
    }
    void load();
    return () => {
      cancelled = true;
    };
  }, []);

  const ready = foundingReady && sdkConnected && (!isCloud || platformConnected);

  return (
    <div className="grid gap-6 lg:grid-cols-[1fr_0.85fr]">
      <SectionCard title={labels.stepsTitle}>
        <ol className="space-y-4">
          {mode.steps.map((step, index) => {
            const done = isStepDone(step, sdkConnected, platformConnected);
            return (
              <li className="flex gap-4" key={step.title}>
                <span
                  className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-sm font-semibold ${
                    done ? "bg-success text-white" : "bg-canvas text-ink"
                  }`}
                >
                  {done ? <Icon className="h-4 w-4" name="CheckCircle2" /> : index + 1}
                </span>
                <div className="min-w-0 pt-0.5">
                  <p className="flex items-center gap-2 text-sm font-semibold text-ink">
                    <Icon className="h-4 w-4 text-blue" name={step.icon} />
                    {step.title}
                  </p>
                  <p className="mt-1 text-sm leading-6 text-muted">{step.body}</p>
                </div>
              </li>
            );
          })}
        </ol>
      </SectionCard>

      <div className="space-y-6">
        <SectionCard title={labels.startTitle}>
          <p className="text-xs font-medium text-muted">{mode.connectLabel}</p>
          <div className="mt-1 space-y-2">
            <CopyRow copyLabel={labels.copyLabel} value={mode.connectValue} />
            {isCloud && mode.streamKey ? <CopyRow copyLabel={labels.copyLabel} value={mode.streamKey} /> : null}
          </div>

          <div className="mt-4 flex items-start gap-2.5 rounded-lg border border-hairline bg-canvas p-3">
            <Icon className="mt-0.5 h-4 w-4 shrink-0 text-blue" name="Video" />
            <p className="text-sm leading-6 text-muted">{mode.videoNote}</p>
          </div>

          {isCloud ? (
            <div className="mt-5">
              <p className="mb-2 text-xs font-medium text-muted">{labels.targetsLabel}</p>
              <ul className="space-y-2">
                {labels.targets.map((target) => (
                  <li
                    className="flex items-center justify-between rounded-lg border border-hairline bg-canvas px-4 py-2.5"
                    key={target.name}
                  >
                    <span className="text-sm font-semibold text-ink">{target.name}</span>
                    <span
                      className={`inline-flex h-6 w-11 items-center rounded-full px-0.5 ${
                        target.enabled ? "justify-end bg-success" : "justify-start bg-border"
                      }`}
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
            {mode.startLabel}
          </button>
          <p className="mt-3 text-sm leading-6 text-muted">{mode.startNote}</p>
        </SectionCard>
      </div>
    </div>
  );
}
