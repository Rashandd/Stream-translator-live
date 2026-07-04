import { OVERLAY_STYLE_DEFAULTS } from "@/src/lib/overlay-style";

export type OverlayBuilderState = {
  /** Target language code, or "" for the source language (no translation). */
  lang: string;
  font: string;
  size: string;
  color: string;
  bgColor: string;
  bgOpacity: number;
  position: "top" | "bottom";
};

export const OVERLAY_BUILDER_DEFAULTS: OverlayBuilderState = {
  lang: "",
  font: OVERLAY_STYLE_DEFAULTS.font,
  size: OVERLAY_STYLE_DEFAULTS.size,
  color: OVERLAY_STYLE_DEFAULTS.color,
  bgColor: "#000000",
  bgOpacity: 60,
  position: OVERLAY_STYLE_DEFAULTS.position,
};

export function hexToRgba(hex: string, opacityPercent: number): string {
  const clean = hex.replace("#", "");
  const value = Number.parseInt(clean, 16);
  const r = (value >> 16) & 255;
  const g = (value >> 8) & 255;
  const b = value & 255;
  const alpha = Math.min(100, Math.max(0, opacityPercent)) / 100;
  return `rgba(${r},${g},${b},${alpha})`;
}

export function buildOverlayQueryParams(state: OverlayBuilderState): URLSearchParams {
  const params = new URLSearchParams();
  if (state.lang) params.set("lang", state.lang);
  params.set("font", state.font);
  params.set("size", state.size);
  params.set("color", state.color);
  params.set("bg", hexToRgba(state.bgColor, state.bgOpacity));
  params.set("position", state.position);
  return params;
}

export function buildOverlayUrl(template: string, state: OverlayBuilderState): string {
  return `${template}?${buildOverlayQueryParams(state).toString()}`;
}
