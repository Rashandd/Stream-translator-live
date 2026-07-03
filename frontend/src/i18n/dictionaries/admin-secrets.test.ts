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

describe("admin secrets page config (V3 — panelden secret yönetimi)", () => {
  const secrets = en.admin.secrets;
  const navItems = en.admin.nav.groups.flatMap((group) => group.items);

  it("exists with title, table columns and row actions", () => {
    expect(secrets.title).toBeTruthy();
    expect(secrets.subtitle).toBeTruthy();
    expect(secrets.columns.length).toBe(4);
    expect(secrets.saveLabel).toBeTruthy();
    expect(secrets.clearLabel).toBeTruthy();
    expect(secrets.confirmClear).toBeTruthy();
  });

  it("is reachable from the admin nav with a registered icon", () => {
    const entry = navItems.find((item) => item.href === "/admin/secrets");
    expect(entry).toBeTruthy();
    expect(registeredIcons.has(entry?.icon ?? ""), `icon "${entry?.icon}" is not registered`).toBe(
      true,
    );
  });

  it("labels every backend source state (panel/env/vault/unset)", () => {
    for (const source of ["panel", "env", "vault", "unset"]) {
      expect(
        secrets.sourceLabels[source as keyof typeof secrets.sourceLabels],
        `missing source label: ${source}`,
      ).toBeTruthy();
    }
  });

  it("documents the masking guarantee and points 401 users to sign-in", () => {
    expect(secrets.maskNote.toLowerCase()).toContain("mask");
    expect(secrets.errors.unauthorized).toBeTruthy();
    expect(secrets.signInLabel).toBeTruthy();
  });

  it("uses no emoji anywhere in the secrets copy", () => {
    const flat = JSON.stringify(secrets);
    expect(EMOJI.test(flat), "emoji found in admin.secrets").toBe(false);
  });
});
