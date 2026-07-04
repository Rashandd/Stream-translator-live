import { describe, expect, it } from "vitest";
import { computeOverlayStyle, OVERLAY_STYLE_DEFAULTS } from "@/src/lib/overlay-style";

describe("computeOverlayStyle", () => {
  it("applies /o/[token]'s current defaults when called with no input", () => {
    const style = computeOverlayStyle();

    expect(style.container.alignItems).toBe("flex-end");
    expect(style.container.justifyContent).toBe("center");
    expect(style.container.background).toBe("transparent");
    expect(style.container.pointerEvents).toBe("none");

    expect(style.caption.fontFamily).toBe(OVERLAY_STYLE_DEFAULTS.font);
    expect(style.caption.fontSize).toBe(OVERLAY_STYLE_DEFAULTS.size);
    expect(style.caption.color).toBe(OVERLAY_STYLE_DEFAULTS.color);
    expect(style.caption.background).toBe(OVERLAY_STYLE_DEFAULTS.bg);
  });

  it("puts the caption at the top only when position is exactly 'top'", () => {
    expect(computeOverlayStyle({ position: "top" }).container.alignItems).toBe("flex-start");
    expect(computeOverlayStyle({ position: "bottom" }).container.alignItems).toBe("flex-end");
    expect(computeOverlayStyle({ position: "sideways" }).container.alignItems).toBe("flex-end");
  });

  it("reflects font/size/color/bg overrides straight into the caption style", () => {
    const style = computeOverlayStyle({
      font: "Georgia, serif",
      size: "44px",
      color: "#00ff00",
      bg: "rgba(10,20,30,0.5)",
    });

    expect(style.caption.fontFamily).toBe("Georgia, serif");
    expect(style.caption.fontSize).toBe("44px");
    expect(style.caption.color).toBe("#00ff00");
    expect(style.caption.background).toBe("rgba(10,20,30,0.5)");
  });

  it("never changes the caption's layout-critical properties across inputs", () => {
    const a = computeOverlayStyle();
    const b = computeOverlayStyle({ font: "Arial, sans-serif" });

    expect(a.caption.maxWidth).toBe("90vw");
    expect(a.caption.textAlign).toBe("center");
    expect(a.caption.borderRadius).toBe("0.5em");
    expect(a.caption.padding).toBe("0.4em 0.8em");
    expect(a.caption.lineHeight).toBe(b.caption.lineHeight);
  });
});
