import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import en from "@/src/i18n/dictionaries/en.json";

/** Icon names registered in the shared <Icon> map. */
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

type LogRow = {
  time: string;
  level: string;
  service: string;
  message: string;
  user: string;
  session: string;
};

describe("admin logs page config (1.12)", () => {
  const logs = en.admin.logs;
  const navItems = en.admin.nav.groups.flatMap((group) => group.items);

  it("exists with title, filters, and table columns", () => {
    expect(logs.title).toBeTruthy();
    expect(logs.subtitle).toBeTruthy();
    expect(logs.levelFilters.length).toBeGreaterThanOrEqual(4);
    expect(logs.serviceFilters.length).toBeGreaterThanOrEqual(5);
    expect(logs.columns.length).toBe(6);
  });

  it("is reachable from the admin nav with a registered icon", () => {
    const entry = navItems.find((item) => item.href === "/admin/logs");
    expect(entry).toBeTruthy();
    expect(registeredIcons.has(entry?.icon ?? ""), `icon "${entry?.icon}" is not registered`).toBe(
      true,
    );
  });

  it("mock rows use known levels and services, and contain no raw secrets", () => {
    const knownLevels = new Set(["error", "warn", "info", "debug"]);
    for (const row of logs.rows as LogRow[]) {
      expect(knownLevels.has(row.level), `unknown level "${row.level}"`).toBe(true);
      expect(logs.serviceFilters).toContain(row.service);
      // Hassas değer örneği ancak maskeli görünebilir (redaction kuralı).
      if (/(key|token|secret|password)\s*[=:]/i.test(row.message)) {
        expect(row.message).toContain("[REDACTED]");
      }
    }
  });

  it("has at least one error row to feed the recent-errors panel", () => {
    const errorRows = (logs.rows as LogRow[]).filter((row) => row.level === "error");
    expect(errorRows.length).toBeGreaterThanOrEqual(1);
    expect(logs.recentErrors.title).toBeTruthy();
  });

  it("documents the config-driven level and redaction guarantees", () => {
    expect(logs.levelNote).toContain("log.level");
    expect(logs.note.toLowerCase()).toContain("redact");
  });
});
