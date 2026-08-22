#!/usr/bin/env node
// mdhtml — npx entry point (DIST-01).
//
// Installs the mdhtml CLI for the current platform/architecture from a GitHub
// release into a per-user bin directory, then runs it with the original
// arguments. Idempotent: when the installed binary is already correct (its
// SHA-256 matches the checksum recorded at install time), nothing is
// downloaded and the binary is run directly.
//
// Reuses the release naming and checksum convention from the CI release
// workflow exactly: assets are mdhtml-<version>-<target>.tar.gz for the
// targets darwin-arm64, darwin-x64, linux-x64-gnu, linux-x64-musl, and
// windows-x64, each with a sha256sum-format <asset>.sha256 sidecar. The
// archive checksum is verified before the binary is written.
//
// Optional environment overrides (not part of the public contract):
//   MDHTML_REPO         owner/repo of the GitHub project (default: feliperun/md.html)
//   MDHTML_VERSION      version to install, e.g. 1.2.3 (default: latest release)
//   MDHTML_INSTALL_DIR  per-user bin directory (default: ~/.local/bin, or
//                       %LOCALAPPDATA%\mdhtml\bin on Windows)

import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, rename, rm, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { gunzipSync } from "node:zlib";

const DEFAULT_REPO = "feliperun/md.html";
const SUPPORTED_TARGETS = [
  "darwin-arm64",
  "darwin-x64",
  "linux-x64-gnu",
  "linux-x64-musl",
  "windows-x64",
];
const STATE_FILE = ".mdhtml-install.json";

function sha256(data) {
  return createHash("sha256").update(data).digest("hex");
}

export function resolveTarget(platform, arch, isMusl = isMuslLibc) {
  let target = null;
  if (platform === "darwin" && arch === "arm64") target = "darwin-arm64";
  else if (platform === "darwin" && arch === "x64") target = "darwin-x64";
  else if (platform === "linux" && arch === "x64") target = isMusl() ? "linux-x64-musl" : "linux-x64-gnu";
  else if (platform === "win32" && arch === "x64") target = "windows-x64";
  if (target === null) {
    throw new Error(
      `mdhtml: unsupported platform/arch ${platform}/${arch}; supported targets: ${SUPPORTED_TARGETS.join(", ")}`
    );
  }
  return target;
}

function isMuslLibc() {
  try {
    const result = spawnSync("ldd", ["--version"], { encoding: "utf8" });
    return /musl/i.test(`${result.stdout ?? ""}${result.stderr ?? ""}`);
  } catch {
    return false;
  }
}

export function verifyChecksum(data, expectedHex) {
  const actual = sha256(data);
  if (actual !== String(expectedHex).trim().toLowerCase()) {
    throw new Error(
      `mdhtml: SHA-256 mismatch (expected ${String(expectedHex).trim()}, got ${actual}); refusing to install`
    );
  }
  return actual;
}

function binaryNameFor(target) {
  return target === "windows-x64" ? "mdhtml.exe" : "mdhtml";
}

function repo(env) {
  const value = env.MDHTML_REPO ?? DEFAULT_REPO;
  if (!/^[^/\s]+\/[^/\s]+$/.test(value)) {
    throw new Error(`mdhtml: invalid repository "${value}"`);
  }
  return value;
}

function normalizeVersion(value) {
  const text = String(value).trim().replace(/^v/, "");
  if (!/^\d+(\.\d+){0,2}([-+][0-9A-Za-z.-]+)?$/.test(text)) {
    throw new Error(`mdhtml: invalid release version "${value}"`);
  }
  return text;
}

async function resolveVersion(env, transport) {
  if (env.MDHTML_VERSION) return normalizeVersion(env.MDHTML_VERSION);
  const latestUrl = `https://github.com/${repo(env)}/releases/latest`;
  const response = await transport.get(latestUrl);
  const match = response.location?.match(/\/releases\/tag\/([^/?#]+)$/);
  if (!match) {
    throw new Error("mdhtml: could not resolve the latest release version");
  }
  return normalizeVersion(match[1]);
}

function defaultInstallDir(env) {
  if (process.platform === "win32") {
    const base = env.LOCALAPPDATA ?? path.join(os.homedir(), "AppData", "Local");
    return path.join(base, "mdhtml", "bin");
  }
  return path.join(os.homedir(), ".local", "bin");
}

async function readState(installDir) {
  try {
    return JSON.parse(await readFile(path.join(installDir, STATE_FILE), "utf8"));
  } catch {
    return null;
  }
}

async function writeState(installDir, state) {
  await writeFile(path.join(installDir, STATE_FILE), `${JSON.stringify(state, null, 2)}\n`);
}

async function isInstalledCorrect(binaryPath, installDir, version, target) {
  const state = await readState(installDir);
  if (!state || state.version !== version || state.target !== target) return false;
  try {
    const installed = await stat(binaryPath);
    if (!installed.isFile()) return false;
  } catch {
    return false;
  }
  return sha256(await readFile(binaryPath)) === state.binarySha256;
}

function parseChecksum(bytes) {
  const text = Buffer.isBuffer(bytes) ? bytes.toString("utf8") : String(bytes);
  const hex = text.trim().split(/\s+/)[0] ?? "";
  if (!/^[0-9a-fA-F]{64}$/.test(hex)) {
    throw new Error("mdhtml: invalid checksum file");
  }
  return hex;
}

function parsePaxPath(records, fallback) {
  let value = fallback;
  for (const record of records.split("\n")) {
    const match = record.match(/^(\d+) ([^=]*)=(.*)$/);
    if (match && match[2] === "path") value = match[3];
  }
  return value;
}

function extractTarGz(bytes) {
  let data;
  try {
    data = gunzipSync(bytes);
  } catch {
    throw new Error("mdhtml: downloaded asset is not a valid gzip archive");
  }
  const files = new Map();
  const text = new TextDecoder("utf-8");
  let offset = 0;
  let pendingName = null;
  while (offset + 512 <= data.length) {
    const header = data.subarray(offset, offset + 512);
    offset += 512;
    const name = text.decode(header.subarray(0, 100)).replace(/\0.*$/u, "");
    if (name === "") break;
    const typeflag = header[156] ?? 0;
    const sizeField = text.decode(header.subarray(124, 136)).replace(/\0.*$/u, "").trim();
    const size = sizeField === "" ? 0 : Number.parseInt(sizeField, 8);
    const payload = data.subarray(offset, offset + size);
    offset += Math.ceil(size / 512) * 512;
    if (typeflag === 120) {
      pendingName = parsePaxPath(text.decode(payload), pendingName);
      continue;
    }
    if (typeflag === 76) {
      pendingName = text.decode(payload).replace(/\0.*$/u, "");
      continue;
    }
    if (typeflag === 48 || typeflag === 0) {
      files.set(pendingName ?? name, payload);
    }
    pendingName = null;
  }
  return files;
}

async function installBinary(binaryPath, bytes, state) {
  const installDir = path.dirname(binaryPath);
  await mkdir(installDir, { recursive: true });
  const tmp = path.join(installDir, `.mdhtml-${process.pid}-${Date.now()}.tmp`);
  await writeFile(tmp, bytes, { mode: 0o755 });
  try {
    await rename(tmp, binaryPath);
  } catch {
    await rm(binaryPath, { force: true });
    await rename(tmp, binaryPath);
  }
  await writeState(installDir, state);
}

function runBinary(binaryPath, args) {
  return new Promise((resolve, reject) => {
    let settled = false;
    const child = spawn(binaryPath, args, { stdio: "inherit" });
    child.on("error", (error) => {
      if (settled) return;
      settled = true;
      reject(new Error(`mdhtml: could not run ${binaryPath}: ${error.message}`));
    });
    child.on("exit", (code, signal) => {
      if (settled) return;
      settled = true;
      if (signal) {
        try {
          process.kill(process.pid, signal);
        } catch {
          // Fall through to a nonzero exit below.
        }
        resolve(1);
      } else {
        resolve(code ?? 1);
      }
    });
  });
}

export async function main(options = {}) {
  const {
    argv = process.argv,
    env = process.env,
    platform = process.platform,
    arch = process.arch,
    isMusl = isMuslLibc,
    transport = httpGet,
    run = runBinary,
    log = console.log,
    logError = console.error,
  } = options;

  try {
    const target = resolveTarget(platform, arch, isMusl);
    const version = await resolveVersion(env, transport);
    const installDir = env.MDHTML_INSTALL_DIR ?? defaultInstallDir(env);
    const binaryPath = path.join(installDir, binaryNameFor(target));
    const args = argv.slice(2);

    if (await isInstalledCorrect(binaryPath, installDir, version, target)) {
      log(`mdhtml ${version} already installed at ${binaryPath}; nothing to download`);
      return run(binaryPath, args);
    }

    const base = `https://github.com/${repo(env)}/releases/download/v${version}`;
    const archive = `mdhtml-${version}-${target}.tar.gz`;

    const checksumResponse = await transport.get(`${base}/${archive}.sha256`);
    if (checksumResponse.status !== 200) {
      throw new Error(`mdhtml: could not fetch the release checksum for ${version} (${target})`);
    }
    const expectedHex = parseChecksum(checksumResponse.body);

    const assetResponse = await transport.get(`${base}/${archive}`);
    if (assetResponse.status !== 200) {
      throw new Error(`mdhtml: could not fetch the release asset for ${version} (${target})`);
    }
    verifyChecksum(assetResponse.body, expectedHex);

    const files = extractTarGz(assetResponse.body);
    const binaryBytes = files.get(binaryNameFor(target));
    if (!binaryBytes) {
      throw new Error(`mdhtml: release asset ${archive} does not contain ${binaryNameFor(target)}`);
    }

    const state = {
      version,
      target,
      archiveSha256: sha256(assetResponse.body),
      binarySha256: sha256(binaryBytes),
    };
    await installBinary(binaryPath, binaryBytes, state);
    log(`mdhtml ${version} installed at ${binaryPath}`);
    return run(binaryPath, args);
  } catch (error) {
    logError(error?.message ?? String(error));
    return 1;
  }
}

async function httpGet(url) {
  const response = await fetch(url, { redirect: "manual" });
  const location = response.headers.get("location");
  const body = location === null ? Buffer.from(await response.arrayBuffer()) : null;
  return { status: response.status, location, body };
}

const isMain = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) {
  main()
    .then((code) => {
      process.exitCode = code;
    })
    .catch((error) => {
      console.error(error?.message ?? String(error));
      process.exitCode = 1;
    });
}
