// Build a Twitch-review ZIP from a fixed deployment profile. The archive is
// deliberately dependency-free, deterministic, and contains only runtime
// assets that Twitch needs to host the extension.
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_PATH = fileURLToPath(import.meta.url);
const EXTENSION_ROOT = path.resolve(path.dirname(SCRIPT_PATH), "..");
const TWITCH_HELPER_URL = "https://extension-files.twitch.tv/helper/v1/twitch-ext.min.js";
const FIXED_DOS_DATE = 0x0021; // 1980-01-01, the ZIP epoch

/** Files allowed in a Twitch review archive, in deterministic ZIP order. */
export const PACKAGE_FILES = Object.freeze([
  "config.html",
  "css/extension.css",
  "js/config.js",
  "js/shared.js",
  "js/viewer.js",
  "viewer.html",
]);

/**
 * The complete, closed set of deployable Twitch Extension profiles.
 *
 * Do not add ad-hoc environment variables or CLI URL flags here. Twitch
 * requires every fetched URL to be allowlisted for the submitted version, so
 * a profile must be chosen before the archive is produced.
 */
export const PACKAGE_PROFILES = Object.freeze({
  staging: Object.freeze({
    ebsBase: "https://ext-staging.strovak.com",
    dashboardUrl: "https://dev.strovak.com/dashboard",
  }),
  production: Object.freeze({
    ebsBase: "https://ext.strovak.com",
    dashboardUrl: "https://strovak.com/dashboard",
  }),
});

function fail(message) {
  throw new Error(`Extension package error: ${message}`);
}

/** Resolve one of the fixed profiles; arbitrary names and URL suffixes fail. */
export function resolveProfile(profileName) {
  if (
    typeof profileName !== "string" ||
    !Object.prototype.hasOwnProperty.call(PACKAGE_PROFILES, profileName)
  ) {
    fail(`Unknown package profile ${JSON.stringify(profileName)}.`);
  }
  return PACKAGE_PROFILES[profileName];
}

function replaceExactly(source, expected, replacement, label) {
  const first = source.indexOf(expected);
  if (first === -1 || source.indexOf(expected, first + expected.length) !== -1) {
    fail(`Could not apply the ${label} profile value exactly once.`);
  }
  return `${source.slice(0, first)}${replacement}${source.slice(first + expected.length)}`;
}

function profileSharedSource(source, profile) {
  let configured = replaceExactly(
    source,
    'export const EBS_BASE = "https://ext.strovak.com";',
    `export const EBS_BASE = ${JSON.stringify(profile.ebsBase)};`,
    "EBS",
  );
  configured = replaceExactly(
    configured,
    'export const DASHBOARD_URL = "https://strovak.com/dashboard";',
    `export const DASHBOARD_URL = ${JSON.stringify(profile.dashboardUrl)};`,
    "dashboard",
  );
  return configured;
}

function extractHttpUrls(entries) {
  const urls = new Set();
  for (const content of entries.values()) {
    const text = content.toString("utf8");
    for (const match of text.matchAll(/https?:\/\/[^\s"'<>`]+/g)) {
      urls.add(match[0]);
    }
  }
  return [...urls].sort();
}

function sameStrings(left, right) {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function validatePackageEntries(entries, profile) {
  const names = [...entries.keys()].sort();
  const expectedNames = [...PACKAGE_FILES].sort();
  if (!sameStrings(names, expectedNames)) {
    fail("Archive contains files outside the review allowlist.");
  }

  const shared = entries.get("js/shared.js")?.toString("utf8");
  if (!shared) {
    fail("Archive is missing js/shared.js.");
  }
  if (!shared.includes(`export const EBS_BASE = ${JSON.stringify(profile.ebsBase)};`)) {
    fail("Archive EBS origin does not match the selected profile.");
  }
  if (!shared.includes(`export const DASHBOARD_URL = ${JSON.stringify(profile.dashboardUrl)};`)) {
    fail("Archive dashboard URL does not match the selected profile.");
  }
  if (/\?ebs=|URLSearchParams|localhost|127\.0\.0\.1/.test(shared)) {
    fail("Archive contains a runtime EBS override.");
  }

  const expectedUrls = [profile.dashboardUrl, profile.ebsBase, TWITCH_HELPER_URL].sort();
  const actualUrls = extractHttpUrls(entries);
  if (!sameStrings(actualUrls, expectedUrls)) {
    fail(`Archive URL allowlist mismatch: ${actualUrls.join(", ") || "none"}.`);
  }
}

const CRC32_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let index = 0; index < table.length; index += 1) {
    let value = index;
    for (let bit = 0; bit < 8; bit += 1) {
      value = (value >>> 1) ^ (value & 1 ? 0xedb88320 : 0);
    }
    table[index] = value >>> 0;
  }
  return table;
})();

function crc32(content) {
  let value = 0xffffffff;
  for (const byte of content) {
    value = CRC32_TABLE[(value ^ byte) & 0xff] ^ (value >>> 8);
  }
  return (value ^ 0xffffffff) >>> 0;
}

/**
 * Create a standards-compliant stored ZIP without a system zip dependency.
 * Fixed metadata and ordered inputs make the resulting review artifact
 * byte-for-byte reproducible for identical source files.
 */
function createZip(entries) {
  const localParts = [];
  const centralParts = [];
  let localOffset = 0;

  for (const name of PACKAGE_FILES) {
    const nameBytes = Buffer.from(name, "utf8");
    const content = entries.get(name);
    if (!content) {
      fail(`Missing archive entry ${name}.`);
    }
    const checksum = crc32(content);

    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4); // ZIP 2.0
    local.writeUInt16LE(0x0800, 6); // UTF-8 filenames
    local.writeUInt16LE(0, 8); // stored, never compressed/obfuscated
    local.writeUInt16LE(0, 10);
    local.writeUInt16LE(FIXED_DOS_DATE, 12);
    local.writeUInt32LE(checksum, 14);
    local.writeUInt32LE(content.length, 18);
    local.writeUInt32LE(content.length, 22);
    local.writeUInt16LE(nameBytes.length, 26);
    local.writeUInt16LE(0, 28);
    localParts.push(local, nameBytes, content);

    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE(0x0314, 4); // Unix host, ZIP 2.0
    central.writeUInt16LE(20, 6);
    central.writeUInt16LE(0x0800, 8);
    central.writeUInt16LE(0, 10);
    central.writeUInt16LE(0, 12);
    central.writeUInt16LE(FIXED_DOS_DATE, 14);
    central.writeUInt32LE(checksum, 16);
    central.writeUInt32LE(content.length, 20);
    central.writeUInt32LE(content.length, 24);
    central.writeUInt16LE(nameBytes.length, 28);
    central.writeUInt16LE(0, 30);
    central.writeUInt16LE(0, 32);
    central.writeUInt16LE(0, 34);
    central.writeUInt16LE(0, 36);
    central.writeUInt32LE(0x81a40000, 38); // regular file, mode 0644
    central.writeUInt32LE(localOffset, 42);
    centralParts.push(central, nameBytes);

    localOffset += local.length + nameBytes.length + content.length;
  }

  const centralDirectory = Buffer.concat(centralParts);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(0, 4);
  end.writeUInt16LE(0, 6);
  end.writeUInt16LE(PACKAGE_FILES.length, 8);
  end.writeUInt16LE(PACKAGE_FILES.length, 10);
  end.writeUInt32LE(centralDirectory.length, 12);
  end.writeUInt32LE(localOffset, 16);
  end.writeUInt16LE(0, 20);

  return Buffer.concat([...localParts, centralDirectory, end]);
}

/** Read stored entries from a ZIP made by this packaging command. */
export function readArchiveEntries(archive) {
  if (!Buffer.isBuffer(archive)) {
    fail("Archive reader expects a Buffer.");
  }

  const entries = new Map();
  let offset = 0;
  while (offset < archive.length) {
    if (offset + 4 > archive.length) {
      fail("Archive ends before a ZIP signature.");
    }
    const signature = archive.readUInt32LE(offset);
    if (signature === 0x02014b50 || signature === 0x06054b50) {
      break;
    }
    if (signature !== 0x04034b50 || offset + 30 > archive.length) {
      fail("Archive has an invalid local ZIP entry.");
    }

    const flags = archive.readUInt16LE(offset + 6);
    const compression = archive.readUInt16LE(offset + 8);
    const compressedSize = archive.readUInt32LE(offset + 18);
    const nameLength = archive.readUInt16LE(offset + 26);
    const extraLength = archive.readUInt16LE(offset + 28);
    const nameStart = offset + 30;
    const dataStart = nameStart + nameLength + extraLength;
    const dataEnd = dataStart + compressedSize;
    if (flags !== 0x0800 || compression !== 0 || dataEnd > archive.length) {
      fail("Archive entry is not a supported deterministic stored ZIP entry.");
    }

    const name = archive.subarray(nameStart, nameStart + nameLength).toString("utf8");
    if (entries.has(name)) {
      fail(`Archive has duplicate entry ${name}.`);
    }
    entries.set(name, archive.subarray(dataStart, dataEnd));
    offset = dataEnd;
  }
  return entries;
}

/** Build one review archive in `dist/<profile>` or a caller-supplied output directory. */
export async function buildPackage(profileName, { outputDir } = {}) {
  const profile = resolveProfile(profileName);
  const destination = path.resolve(outputDir || path.join(EXTENSION_ROOT, "dist", profileName));
  const packageDir = path.join(destination, "package");
  const archivePath = path.join(destination, `streamdubber-twitch-extension-${profileName}.zip`);

  await rm(packageDir, { recursive: true, force: true });
  await rm(archivePath, { force: true });
  await mkdir(packageDir, { recursive: true });

  const entries = new Map();
  for (const name of PACKAGE_FILES) {
    const sourcePath = path.join(EXTENSION_ROOT, name);
    const destinationPath = path.join(packageDir, name);
    let content = await readFile(sourcePath);
    if (name === "js/shared.js") {
      content = Buffer.from(profileSharedSource(content.toString("utf8"), profile));
    }
    await mkdir(path.dirname(destinationPath), { recursive: true });
    await writeFile(destinationPath, content);
    entries.set(name, content);
  }

  validatePackageEntries(entries, profile);
  await writeFile(archivePath, createZip(entries));
  return { archivePath, packageDir, profile: profileName };
}

async function runCli() {
  const [profileName, ...extraArgs] = process.argv.slice(2);
  if (!profileName || extraArgs.length > 0) {
    fail("Usage: node scripts/package.mjs <staging|production>");
  }
  const result = await buildPackage(profileName);
  process.stdout.write(`${result.archivePath}\n`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === SCRIPT_PATH) {
  runCli().catch((error) => {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  });
}
