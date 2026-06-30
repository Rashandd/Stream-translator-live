import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import en from "@/src/i18n/dictionaries/en.json";

/** Reads intrinsic width/height from a PNG IHDR chunk. */
function readPngSize(path: string) {
  const buf = readFileSync(path);
  // PNG signature is 8 bytes, IHDR length+type is 8 bytes, then width/height.
  return { width: buf.readUInt32BE(16), height: buf.readUInt32BE(20) };
}

describe("site-header logo", () => {
  const headerSrc = readFileSync(
    fileURLToPath(new URL("./site-header.tsx", import.meta.url)),
    "utf8",
  );

  it("declares Image width/height matching the asset aspect ratio", () => {
    const width = Number(headerSrc.match(/width=\{(\d+)\}/)?.[1]);
    const height = Number(headerSrc.match(/height=\{(\d+)\}/)?.[1]);
    expect(width).toBeGreaterThan(0);
    expect(height).toBeGreaterThan(0);

    const assetPath = fileURLToPath(
      new URL(`../../public${en.brand.logoAsset}`, import.meta.url),
    );
    const asset = readPngSize(assetPath);

    const declaredRatio = width / height;
    const assetRatio = asset.width / asset.height;
    // Distortion guard: keeps the logo from being squished into a wrong box.
    expect(Math.abs(declaredRatio - assetRatio)).toBeLessThan(0.02);
  });
});
