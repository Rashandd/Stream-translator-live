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
    // A-02: every opaque error code the backend can return has dictionary copy.
    expect(oauthComplete.errors.accountNotFound).toBeTruthy();
    expect(oauthComplete.errors.emailExists).toBeTruthy();
    expect(oauthComplete.errors.invalidState).toBeTruthy();
    expect(oauthComplete.errors.providerUnavailable).toBeTruthy();
    expect(oauthComplete.errors.oauthFailed).toBeTruthy();
    expect(oauthComplete.errors.generic).toBeTruthy();
  });

  it("oauth buttons link straight to the backend start endpoint with intent + locale (real nav, not fetch)", () => {
    expect(oauthButtonsSource).toContain("/api/auth/oauth/${provider}/start");
    expect(oauthButtonsSource).toContain("${API_URL}");
    // Intent + locale ride the start URL so the callback can be locale-safe (A-03).
    expect(oauthButtonsSource).toContain("intent");
    expect(oauthButtonsSource).toContain("locale");
  });

  it("login sends intent=login (no silent signup) and signup sends intent=signup", () => {
    expect(loginPageSource).toContain('intent="login"');
    expect(signupPageSource).toContain('intent="signup"');
  });

  it("oauth-complete maps opaque codes to dictionary copy, never showing the raw URL value", () => {
    // The user-facing text comes from labels.errors, not from the query string.
    expect(oauthCompleteSource).toContain("labels.errors");
    expect(oauthCompleteSource).toContain("account_not_found");
    expect(oauthCompleteSource).not.toContain("queryError || ");
  });

  it("renders brand marks for the visible social buttons", () => {
    expect(oauthButtonsSource).toContain('brand="youtube"');
    expect(oauthButtonsSource).toContain('brand="twitch"');
  });

  it("login and signup pages both render the OAuthButtons component", () => {
    expect(loginPageSource).toContain("<OAuthButtons");
    expect(signupPageSource).toContain("<OAuthButtons");
  });

  it("oauth-complete verifies the cookie session — no token in the URL or localStorage (Faz 2)", () => {
    // The callback set HttpOnly session cookies on the redirect; the page just
    // confirms the session and routes. Nothing reads a token anywhere.
    expect(oauthCompleteSource).toContain("/api/auth/session");
    expect(oauthCompleteSource).not.toContain("setUserToken");
    expect(oauthCompleteSource).not.toContain("window.location.hash");
  });

  it("oauth-complete page is wired to the handler component", () => {
    expect(oauthCompletePageSource).toContain("<OAuthCompleteHandler");
  });
});
