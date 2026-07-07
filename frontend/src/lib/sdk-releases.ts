/**
 * SDK indirilebilir installer URL'leri — statik hosting tabanı env'den gelir
 * (bkz. NEXT_PUBLIC_API_URL deseni, src/lib/api.ts), koda gömülmez.
 *
 * CI'nin (.github/workflows/sdk-release.yml) her release'te platform başına
 * yüklediği SABİT isimli asset'lerle birebir eşleşir — dosya adı değişirse
 * ikisi birden güncellenmeli.
 */

const ASSET_BY_PLATFORM: Record<string, string> = {
  Windows: "StreamDubber-SDK-Setup-x64.exe",
  macOS: "StreamDubber-SDK-macOS.dmg",
  Linux: "StreamDubber-SDK-Linux-x86_64.AppImage",
};

/** Returns the download URL for a platform, or `null` if releases hosting isn't configured yet. */
export function sdkDownloadUrl(platformName: string): string | null {
  const base = process.env.NEXT_PUBLIC_SDK_RELEASES_BASE?.replace(/\/+$/, "");
  if (!base) {
    return null;
  }
  const asset = ASSET_BY_PLATFORM[platformName];
  return asset ? `${base}/${asset}` : null;
}
