import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import en from "@/src/i18n/dictionaries/en.json";

const oauthButtonsSource = readFileSync(
  fileURLToPath(new URL("../../components/oauth-buttons.tsx", import.meta.url)),
  "utf8",
);
const oauthCompleteSource = readFileSync(
  fileURLToPath(new URL("../../components/oauth-complete.tsx", import.meta.url)),
  "utf8",
);
const loginPageSource = readFileSync(
  fileURLToPath(new URL("../../../app/[locale]/login/page.tsx", import.meta.url)),
  "utf8",
);
const signupPageSource = readFileSync(
  fileURLToPath(new URL("../../../app/[locale]/signup/page.tsx", import.meta.url)),
  "utf8",
);
const oauthCompletePageSource = readFileSync(
  fileURLToPath(new URL("../../../app/[locale]/login/oauth-complete/page.tsx", import.meta.url)),
  "utf8",
);

describe("Google/Twitch login wiring", () => {
  it("has non-empty copy for the social buttons and the oauth-complete step", () => {
    const { social, oauthComplete } = en.auth;
    expect(social.dividerLabel).toBeTruthy();
    expect(social.google).toContain("YouTube");
    expect(social.twitch).toBeTruthy();
    expect(oauthComplete.title).toBeTruthy();
    expect(oauthComplete.backLabel).toBeTruthy();
  });

  it("oauth buttons link straight to the backend's real start endpoints (real nav, not fetch)", () => {
    expect(oauthButtonsSource).toContain("/api/auth/oauth/google/start");
    expect(oauthButtonsSource).toContain("/api/auth/oauth/twitch/start");
    expect(oauthButtonsSource).toContain("${API_URL}");
  });

  it("renders brand marks for the visible social buttons", () => {
    expect(oauthButtonsSource).toContain('brand="youtube"');
    expect(oauthButtonsSource).toContain('brand="twitch"');
  });

  it("login and signup pages both render the OAuthButtons component", () => {
    expect(loginPageSource).toContain("<OAuthButtons");
    expect(signupPageSource).toContain("<OAuthButtons");
  });

  it("oauth-complete handler reads the token from the URL fragment and stores it via setUserToken", () => {
    expect(oauthCompleteSource).toContain("window.location.hash");
    expect(oauthCompleteSource).toContain("setUserToken(token)");
    // Fragment, not query — the token must never be sent to a server (see api.ts docs).
    expect(oauthCompleteSource).not.toContain("searchParams.get(\"token\")");
  });

  it("oauth-complete page is wired to the handler component", () => {
    expect(oauthCompletePageSource).toContain("<OAuthCompleteHandler");
  });
});
