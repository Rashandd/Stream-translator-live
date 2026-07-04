import type { CSSProperties } from "react";

/**
 * Single source of truth for `/o/[token]`'s render style, shared with the
 * dashboard's Caption Overlay Builder preview so the two can never drift —
 * both call this same function with the same inputs.
 */
export type OverlayStyleInput = {
  font?: string;
  size?: string;
  color?: string;
  bg?: string;
  position?: string;
};

export const OVERLAY_STYLE_DEFAULTS = {
  font: "Inter, system-ui, sans-serif",
  size: "32px",
  color: "#ffffff",
  bg: "rgba(0,0,0,0.6)",
  position: "bottom",
} as const;

export function computeOverlayStyle(input: OverlayStyleInput = {}): {
  container: CSSProperties;
  caption: CSSProperties;
} {
  const font = input.font ?? OVERLAY_STYLE_DEFAULTS.font;
  const size = input.size ?? OVERLAY_STYLE_DEFAULTS.size;
  const color = input.color ?? OVERLAY_STYLE_DEFAULTS.color;
  const bg = input.bg ?? OVERLAY_STYLE_DEFAULTS.bg;
  const alignItems = input.position === "top" ? "flex-start" : "flex-end";

  return {
    container: {
      position: "fixed",
      inset: 0,
      display: "flex",
      alignItems,
      justifyContent: "center",
      padding: "5vh 4vw",
      background: "transparent",
      pointerEvents: "none",
    },
    caption: {
      fontFamily: font,
      fontSize: size,
      color,
      background: bg,
      padding: "0.4em 0.8em",
      borderRadius: "0.5em",
      maxWidth: "90vw",
      textAlign: "center",
      lineHeight: 1.3,
    },
  };
}
