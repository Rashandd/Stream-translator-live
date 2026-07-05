import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import en from "@/src/i18n/dictionaries/en.json";

const iconSource = readFileSync(
  fileURLToPath(new URL("../../components/icon.tsx", import.meta.url)),
  "utf8",
);
const iconMapBody = iconSource.match(/const icons[^{]*\{([\s\S]*?)\}/)?.[1] ?? "";
const registeredIcons = new Set(
  iconMapBody
    .split(",")
    .map((entry) => entry.trim())
    .filter(Boolean),
);

const EMOJI = /[\u{1F300}-\u{1FAFF}\u{2600}-\u{27BF}]/u;

describe("admin production setup page config (V11 — production operator handoff)", () => {
  const productionSetup = en.admin.productionSetup;
  const navItems = en.admin.nav.groups.flatMap((group) => group.items);

  it("exists with title, section titles and provider table columns", () => {
    expect(productionSetup.title).toBeTruthy();
    expect(productionSetup.subtitle).toBeTruthy();
    expect(productionSetup.checksTitle).toBeTruthy();
    expect(productionSetup.providersTitle).toBeTruthy();
    expect(productionSetup.providersColumns.length).toBe(6);
  });

  it("is reachable from the admin nav with a registered icon", () => {
    const entry = navItems.find((item) => item.href === "/admin/production-setup");
    expect(entry).toBeTruthy();
    expect(registeredIcons.has(entry?.icon ?? ""), `icon "${entry?.icon}" is not registered`).toBe(
      true,
    );
  });

  it("labels both provider kinds returned by the backend", () => {
    for (const kind of ["api_key", "oauth"]) {
      expect(
        productionSetup.kindLabels[kind as keyof typeof productionSetup.kindLabels],
        `missing kind label: ${kind}`,
      ).toBeTruthy();
    }
  });

  it("covers connect, test and stripe price actions", () => {
    expect(productionSetup.connectLabel).toBeTruthy();
    expect(productionSetup.testLabel).toBeTruthy();
    expect(productionSetup.stripeTitle).toBeTruthy();
    expect(productionSetup.planAccessTitle).toBeTruthy();
    expect(productionSetup.planAccessColumns.length).toBe(3);
    expect(productionSetup.saveLabel).toBeTruthy();
    expect(productionSetup.placeholder).toBeTruthy();
  });

  it("points 401 users to sign-in", () => {
    expect(productionSetup.errors.unauthorized).toBeTruthy();
    expect(productionSetup.signInLabel).toBeTruthy();
  });

  it("uses no emoji anywhere in the production setup copy", () => {
    const flat = JSON.stringify(productionSetup);
    expect(EMOJI.test(flat), "emoji found in admin.productionSetup").toBe(false);
  });
});
