// Archive-level tests for the fixed Twitch Extension packaging profiles.
import { after, test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import {
  PACKAGE_FILES,
  PACKAGE_PROFILES,
  buildPackage,
  readArchiveEntries,
  resolveProfile,
} from "../scripts/package.mjs";

const temporaryDirectories = [];

after(async () => {
  await Promise.all(
    temporaryDirectories.map((directory) => rm(directory, { recursive: true, force: true })),
  );
});

async function packageFor(profile) {
  const outputDir = await mkdtemp(path.join(os.tmpdir(), "streamdubber-extension-package-"));
  temporaryDirectories.push(outputDir);
  return buildPackage(profile, { outputDir });
}

function archiveText(entries) {
  return [...entries.values()].map((value) => value.toString("utf8")).join("\n");
}

test("packaging exposes only the fixed staging and production profiles", () => {
  assert.deepEqual(Object.keys(PACKAGE_PROFILES).sort(), ["production", "staging"]);
  assert.deepEqual(resolveProfile("staging"), {
    ebsBase: "https://ext-staging.strovak.com",
    dashboardUrl: "https://dev.strovak.com/dashboard",
  });
  assert.deepEqual(resolveProfile("production"), {
    ebsBase: "https://ext.strovak.com",
    dashboardUrl: "https://strovak.com/dashboard",
  });
  assert.throws(() => resolveProfile("staging?ebs=https://evil.example"), /Unknown package profile/);
});

test("staging archive contains only review assets pinned to staging origins", async () => {
  const result = await packageFor("staging");
  const entries = readArchiveEntries(await readFile(result.archivePath));

  assert.deepEqual([...entries.keys()].sort(), [...PACKAGE_FILES].sort());

  const shared = entries.get("js/shared.js").toString("utf8");
  assert.match(shared, /EBS_BASE = "https:\/\/ext-staging\.strovak\.com"/);
  assert.match(shared, /DASHBOARD_URL = "https:\/\/dev\.strovak\.com\/dashboard"/);
  assert.doesNotMatch(shared, /https:\/\/ext\.strovak\.com/);
  assert.doesNotMatch(shared, /https:\/\/strovak\.com\/dashboard/);
  assert.doesNotMatch(shared, /\?ebs=|URLSearchParams|localhost|127\.0\.0\.1/);

  const urls = [...archiveText(entries).matchAll(/https?:\/\/[^\s"'<>`]+/g)].map((match) => match[0]);
  assert.deepEqual([...new Set(urls)].sort(), [
    "https://dev.strovak.com/dashboard",
    "https://ext-staging.strovak.com",
    "https://extension-files.twitch.tv/helper/v1/twitch-ext.min.js",
  ]);
});

test("production archive is independently pinned and archive bytes are reproducible", async () => {
  const firstStaging = await packageFor("staging");
  const secondStaging = await packageFor("staging");
  assert.deepEqual(
    await readFile(firstStaging.archivePath),
    await readFile(secondStaging.archivePath),
  );

  const production = await packageFor("production");
  const entries = readArchiveEntries(await readFile(production.archivePath));
  const shared = entries.get("js/shared.js").toString("utf8");
  assert.match(shared, /EBS_BASE = "https:\/\/ext\.strovak\.com"/);
  assert.match(shared, /DASHBOARD_URL = "https:\/\/strovak\.com\/dashboard"/);
  assert.doesNotMatch(shared, /ext-staging|dev\.strovak\.com/);
});
