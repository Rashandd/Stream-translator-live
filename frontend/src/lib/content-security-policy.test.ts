import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { buildContentSecurityPolicy } from "./content-security-policy";

const middlewareSource = readFileSync(new URL("../../middleware.ts", import.meta.url), "utf8");
const rootLayoutSource = readFileSync(new URL("../../app/layout.tsx", import.meta.url), "utf8");

describe("browser content security policy", () => {
  it("restricts scripts, connections, frames, and embedded active content", () => {
    const policy = buildContentSecurityPolicy("safe-nonce", {
      apiUrl: "https://api.strovak.com/v1",
      overlayWebSocketUrl: "wss://overlay.strovak.com/ws",
    });

    expect(policy).toContain("script-src 'self' 'nonce-safe-nonce' 'strict-dynamic'");
    expect(policy).toContain("connect-src 'self' https://api.strovak.com wss://overlay.strovak.com");
    expect(policy).toContain("frame-ancestors 'none'");
    expect(policy).toContain("object-src 'none'");
    expect(policy.match(/script-src[^;]+/)?.[0]).not.toContain("'unsafe-inline'");
    expect(policy).not.toContain("'unsafe-eval'");
  });

  it("adds unsafe-eval only for Next development tooling", () => {
    expect(buildContentSecurityPolicy("nonce", { development: true })).toContain("'unsafe-eval'");
    expect(buildContentSecurityPolicy("nonce", { development: false })).not.toContain("'unsafe-eval'");
  });

  it("does not copy malformed environment values into CSP", () => {
    const policy = buildContentSecurityPolicy("nonce", {
      apiUrl: "https://api.strovak.com; script-src *",
      overlayWebSocketUrl: "javascript:alert(1)",
    });
    expect(policy).not.toContain("api.strovak.com");
    expect(policy).not.toContain("javascript:");
    expect(() => buildContentSecurityPolicy("bad; script-src *")).toThrow("invalid CSP nonce");
  });

  it("sets the nonce-bearing policy on both request and response", () => {
    expect(middlewareSource).toContain('requestHeaders.set("x-nonce", nonce)');
    expect(middlewareSource).toContain('requestHeaders.set("Content-Security-Policy", policy)');
    expect(middlewareSource).toContain('response.headers.set("Content-Security-Policy", policy)');
  });

  it("loads the pre-paint theme initializer as a same-origin external script", () => {
    expect(rootLayoutSource).toContain('src="/theme-init.js"');
    expect(rootLayoutSource).toContain("nonce={nonce}");
    expect(rootLayoutSource).not.toContain("dangerouslySetInnerHTML");
  });
});
