import { afterEach, describe, expect, it, vi } from "vitest";
import { sdkDownloadUrl } from "@/src/lib/sdk-releases";

afterEach(() => {
  vi.unstubAllEnvs();
});

describe("sdkDownloadUrl", () => {
  it("returns null when releases hosting isn't configured", () => {
    vi.stubEnv("NEXT_PUBLIC_SDK_RELEASES_BASE", "");
    expect(sdkDownloadUrl("Windows")).toBeNull();
  });

  it("builds a platform-specific asset URL once configured", () => {
    vi.stubEnv("NEXT_PUBLIC_SDK_RELEASES_BASE", "https://releases.example.com/sdk");
    expect(sdkDownloadUrl("Windows")).toBe("https://releases.example.com/sdk/StreamDubber-SDK-Setup-x64.exe");
    expect(sdkDownloadUrl("macOS")).toBe("https://releases.example.com/sdk/StreamDubber-SDK-macOS.dmg");
    expect(sdkDownloadUrl("Linux")).toBe(
      "https://releases.example.com/sdk/StreamDubber-SDK-Linux-x86_64.AppImage",
    );
  });

  it("strips a trailing slash from the configured base", () => {
    vi.stubEnv("NEXT_PUBLIC_SDK_RELEASES_BASE", "https://releases.example.com/sdk/");
    expect(sdkDownloadUrl("Windows")).toBe("https://releases.example.com/sdk/StreamDubber-SDK-Setup-x64.exe");
  });

  it("returns null for an unknown platform name even when configured", () => {
    vi.stubEnv("NEXT_PUBLIC_SDK_RELEASES_BASE", "https://releases.example.com/sdk");
    expect(sdkDownloadUrl("BeOS")).toBeNull();
  });
});
