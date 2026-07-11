"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { CopyRow } from "@/src/components/copy-row";
import { userFetch } from "@/src/lib/api";
import {
  buildOverlayUrl,
  hexToRgba,
  OVERLAY_BUILDER_DEFAULTS,
  type OverlayBuilderState,
} from "@/src/lib/overlay-builder-url";
import { computeOverlayStyle } from "@/src/lib/overlay-style";

/** `GET /api/overlay/token` yanıtı (backend `overlay::OverlayTokenResponse`). */
type OverlayTokenState = { active: boolean; token: string | null };

/** Yayın kapalıyken de panel taze kalsın diye — canlıya geçişi birkaç
 * saniye içinde yakalar, `SdkDevicesPanel`'in heartbeat polling'iyle aynı desen. */
const POLL_INTERVAL_MS = 10_000;

type Target = { code: string; language: string; enabled: boolean };

type SelectOption = { value: string; label: string };

type BuilderLabels = {
  languageLabel: string;
  sourceOptionLabel: string;
  fontLabel: string;
  fontOptions: SelectOption[];
  sizeLabel: string;
  sizeOptions: SelectOption[];
  colorLabel: string;
  backgroundLabel: string;
  opacityLabel: string;
  positionLabel: string;
  positionTopLabel: string;
  positionBottomLabel: string;
  previewLabel: string;
  previewSourceCaption: string;
  previewTranslatedCaption: string;
  urlLabel: string;
  copyLabel: string;
  tokenNote: string;
};

type Props = {
  overlayUrlTemplate: string;
  sourceLabel: string;
  sourceValue: string;
  targets: Target[];
  labels: BuilderLabels;
};

const selectClassName =
  "h-11 rounded-lg border border-border bg-surface px-3 text-sm text-ink outline-none transition focus:border-blue";

export function CaptionOverlayBuilder({
  overlayUrlTemplate,
  sourceLabel,
  sourceValue,
  targets,
  labels,
}: Props) {
  const [state, setState] = useState<OverlayBuilderState>(OVERLAY_BUILDER_DEFAULTS);
  const [overlayToken, setOverlayToken] = useState<OverlayTokenState | null>(null);

  function update<K extends keyof OverlayBuilderState>(key: K, value: OverlayBuilderState[K]) {
    setState((current) => ({ ...current, [key]: value }));
  }

  const loadOverlayToken = useCallback(async () => {
    try {
      const res = await userFetch("/api/overlay/token");
      if (!res.ok) {
        return;
      }
      setOverlayToken((await res.json()) as OverlayTokenState);
    } catch {
      // Sessizce yut — placeholder şablon zaten güvenli bir varsayılan.
    }
  }, []);

  useEffect(() => {
    void loadOverlayToken();
    const interval = setInterval(() => void loadOverlayToken(), POLL_INTERVAL_MS);
    return () => clearInterval(interval);
  }, [loadOverlayToken]);

  const previewBg = hexToRgba(state.bgColor, state.bgOpacity);
  const style = useMemo(
    () =>
      computeOverlayStyle({
        font: state.font,
        size: state.size,
        color: state.color,
        bg: previewBg,
        position: state.position,
      }),
    [state.font, state.size, state.color, previewBg, state.position],
  );
  const previewText = state.lang ? labels.previewTranslatedCaption : labels.previewSourceCaption;
  const isLive = overlayToken?.active === true && !!overlayToken.token;
  const resolvedTemplate = useMemo(() => {
    if (isLive && overlayToken?.token && typeof window !== "undefined") {
      return `${window.location.origin}/o/${overlayToken.token}`;
    }
    return overlayUrlTemplate;
  }, [isLive, overlayToken, overlayUrlTemplate]);
  const overlayUrl = useMemo(
    () => buildOverlayUrl(resolvedTemplate, state),
    [resolvedTemplate, state],
  );

  return (
    <div className="space-y-5">
      <div className="grid gap-4 sm:grid-cols-2">
        <label className="block">
          <span className="text-xs font-medium text-muted">{labels.languageLabel}</span>
          <select
            className={`mt-1 w-full ${selectClassName}`}
            onChange={(event) => update("lang", event.target.value)}
            value={state.lang}
          >
            <option value="">{labels.sourceOptionLabel}</option>
            {targets
              .filter((target) => target.enabled)
              .map((target) => (
                <option key={target.code} value={target.code}>
                  {target.language}
                </option>
              ))}
          </select>
          <span className="mt-1 block text-xs text-faint">
            {sourceLabel}: {sourceValue}
          </span>
        </label>

        <label className="block">
          <span className="text-xs font-medium text-muted">{labels.fontLabel}</span>
          <select
            className={`mt-1 w-full ${selectClassName}`}
            onChange={(event) => update("font", event.target.value)}
            value={state.font}
          >
            {labels.fontOptions.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>

        <label className="block">
          <span className="text-xs font-medium text-muted">{labels.sizeLabel}</span>
          <select
            className={`mt-1 w-full ${selectClassName}`}
            onChange={(event) => update("size", event.target.value)}
            value={state.size}
          >
            {labels.sizeOptions.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>

        <div>
          <span className="text-xs font-medium text-muted">{labels.positionLabel}</span>
          <div className="mt-1 inline-flex h-11 w-full overflow-hidden rounded-lg border border-border">
            {(
              [
                ["bottom", labels.positionBottomLabel],
                ["top", labels.positionTopLabel],
              ] as const
            ).map(([value, label]) => (
              <button
                className={`flex-1 text-sm font-semibold transition ${
                  state.position === value ? "bg-primary text-white" : "bg-surface text-ink hover:bg-canvas"
                }`}
                key={value}
                onClick={() => update("position", value)}
                type="button"
              >
                {label}
              </button>
            ))}
          </div>
        </div>

        <label className="block">
          <span className="text-xs font-medium text-muted">{labels.colorLabel}</span>
          <div className="mt-1 flex h-11 items-center gap-2 rounded-lg border border-border bg-surface px-2">
            <input
              aria-label={labels.colorLabel}
              className="h-8 w-8 shrink-0 cursor-pointer rounded border border-border"
              onChange={(event) => update("color", event.target.value)}
              type="color"
              value={state.color}
            />
            <input
              className="w-full min-w-0 font-mono text-sm text-ink outline-none"
              onChange={(event) => update("color", event.target.value)}
              value={state.color}
            />
          </div>
        </label>

        <div>
          <span className="text-xs font-medium text-muted">{labels.backgroundLabel}</span>
          <div className="mt-1 flex h-11 items-center gap-2 rounded-lg border border-border bg-surface px-2">
            <input
              aria-label={labels.backgroundLabel}
              className="h-8 w-8 shrink-0 cursor-pointer rounded border border-border"
              onChange={(event) => update("bgColor", event.target.value)}
              type="color"
              value={state.bgColor}
            />
            <span className="w-14 shrink-0 text-xs text-muted">{labels.opacityLabel}</span>
            <input
              aria-label={labels.opacityLabel}
              className="w-full"
              max={100}
              min={0}
              onChange={(event) => update("bgOpacity", Number(event.target.value))}
              type="range"
              value={state.bgOpacity}
            />
            <span className="w-10 shrink-0 text-right font-mono text-xs text-muted">{state.bgOpacity}%</span>
          </div>
        </div>
      </div>

      <div>
        <p className="mb-2 text-xs font-medium text-muted">{labels.previewLabel}</p>
        <div className="relative min-h-24 overflow-hidden rounded-lg bg-emphasis p-4">
          <div style={{ ...style.container, position: "absolute" }}>
            <span style={style.caption}>{previewText}</span>
          </div>
        </div>
      </div>

      <div>
        <p className="mb-2 text-xs font-medium text-muted">{labels.urlLabel}</p>
        <CopyRow copyLabel={labels.copyLabel} value={overlayUrl} />
        {!isLive ? <p className="mt-2 text-xs leading-5 text-faint">{labels.tokenNote}</p> : null}
      </div>
    </div>
  );
}
