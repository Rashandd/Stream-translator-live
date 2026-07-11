import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import en from "@/src/i18n/dictionaries/en.json";

const iconSource = readFileSync(fileURLToPath(new URL("../../components/icon.tsx", import.meta.url)), "utf8");
const iconMapBody = iconSource.match(/const icons[^{]*\{([\s\S]*?)\}/)?.[1] ?? "";
const registeredIcons = new Set(
  iconMapBody
    .split(",")
    .map((entry) => entry.trim())
    .filter(Boolean),
);

describe("dashboard sign-in account linking config", () => {
  const account = en.dashboard.account;
  const navItems = en.dashboard.nav.groups.flatMap((group) => group.items);

  it("is reachable from the dashboard nav with a registered icon", () => {
    const entry = navItems.find((item) => item.href === "/dashboard/account");
    expect(entry).toBeTruthy();
    expect(registeredIcons.has(entry?.icon ?? ""), `icon "${entry?.icon}" is not registered`).toBe(true);
  });

  it("exists with title, subtitle, and password-status copy", () => {
    expect(account.title).toBeTruthy();
    expect(account.subtitle).toBeTruthy();
    expect(account.passwordSetHint).toBeTruthy();
    expect(account.passwordNotSetHint).toBeTruthy();
    expect(account.lastMethodNotice).toBeTruthy();
  });

  it("lists exactly the two providers the backend supports for login (google, twitch)", () => {
    const providerIds = account.providers.map((p) => p.provider).sort();
    expect(providerIds).toEqual(["google", "twitch"]);
  });

  it("every provider has a registered icon", () => {
    for (const provider of account.providers) {
      expect(registeredIcons.has(provider.icon), `icon "${provider.icon}" is not registered`).toBe(true);
    }
  });

  it("linkedBanner has a {provider} placeholder for the post-redirect success message", () => {
    expect(account.linkedBanner).toContain("{provider}");
  });
});
