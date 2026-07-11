import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./linked-accounts-panel.tsx", import.meta.url), "utf8");

describe("linked accounts panel", () => {
  it("loads the current user's linked login identities", () => {
    expect(source).toContain('userFetch("/api/auth/oauth/identities")');
  });

  it("links through the authenticated authorize-url fetch, then navigates the browser", () => {
    expect(source).toContain('userFetch(`/api/auth/oauth/${provider}/link/start`)');
    expect(source).toContain("window.location.href = body.authorize_url");
  });

  it("unlinks through DELETE and surfaces the last-method conflict distinctly", () => {
    expect(source).toContain('userFetch(`/api/auth/oauth/${provider}`, { method: "DELETE" })');
    expect(source).toContain("res.status === 409");
    expect(source).toContain("labels.lastMethodNotice");
  });

  it("reads the post-link redirect banner from the query string", () => {
    expect(source).toContain('searchParams.get("linked")');
  });
});
