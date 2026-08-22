import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, readFile, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { gzipSync } from "node:zlib";
import { main, resolveTarget, verifyChecksum } from "../../scripts/mdhtml-cli.mjs";

const sha256 = (data) => createHash("sha256").update(data).digest("hex");
const binaryNameFor = (target) => (target === "windows-x64" ? "mdhtml.exe" : "mdhtml");

function tarGzWith(name, bytes) {
  const header = Buffer.alloc(512);
  header.write(name, 0, "utf8");
  header.write(bytes.length.toString(8).padStart(11, "0"), 124, 11, "ascii");
  header[135] = 0;
  header[156] = 48; // '0': regular file
  header.write("ustar", 257, "utf8");
  const payload = Buffer.concat([header, bytes, Buffer.alloc((512 - (bytes.length % 512)) % 512)]);
  return gzipSync(Buffer.concat([payload, Buffer.alloc(1024)]));
}

function fakeTransport(routes) {
  const calls = [];
  return {
    calls,
    async get(url) {
      calls.push(url);
      const route = routes.find((entry) => url.endsWith(entry.url));
      if (!route) return { status: 404, location: null, body: null };
      return { status: 200, location: route.location ?? null, body: route.body ?? null };
    },
  };
}

const silent = { log() {}, logError() {} };

test("resolveTarget maps every supported platform and arch", () => {
  assert.equal(resolveTarget("darwin", "arm64"), "darwin-arm64");
  assert.equal(resolveTarget("darwin", "x64"), "darwin-x64");
  assert.equal(resolveTarget("linux", "x64", () => false), "linux-x64-gnu");
  assert.equal(resolveTarget("linux", "x64", () => true), "linux-x64-musl");
  assert.equal(resolveTarget("win32", "x64"), "windows-x64");
});

test("resolveTarget fails with a clear message on an unsupported platform/arch", () => {
  for (const [platform, arch] of [
    ["darwin", "ia32"],
    ["linux", "arm64"],
    ["freebsd", "x64"],
    ["win32", "arm64"],
  ]) {
    assert.throws(
      () => resolveTarget(platform, arch),
      /unsupported platform\/arch .* supported targets: darwin-arm64, darwin-x64, linux-x64-gnu, linux-x64-musl, windows-x64/
    );
  }
});

test("verifyChecksum accepts a matching hash and rejects a mismatch", () => {
  const bytes = Buffer.from("payload");
  assert.equal(verifyChecksum(bytes, sha256(bytes)), sha256(bytes));
  assert.throws(() => verifyChecksum(bytes, "0".repeat(64)), /SHA-256 mismatch/);
});

test("main installs a verified release and runs the binary with the original argv", async () => {
  const installDir = await mkdtemp(join(tmpdir(), "mdhtml-install-"));
  const binaryBytes = Buffer.from("#!/bin/sh\nexit 0\n");
  const archive = tarGzWith("mdhtml", binaryBytes);
  const transport = fakeTransport([
    { url: "mdhtml-1.2.3-darwin-arm64.tar.gz.sha256", body: Buffer.from(`${sha256(archive)}  mdhtml-1.2.3-darwin-arm64.tar.gz\n`) },
    { url: "mdhtml-1.2.3-darwin-arm64.tar.gz", body: archive },
  ]);
  const runCalls = [];
  const options = {
    argv: ["node", "mdhtml-cli.mjs", "build", "doc.md"],
    env: { MDHTML_VERSION: "1.2.3", MDHTML_INSTALL_DIR: installDir },
    platform: "darwin",
    arch: "arm64",
    isMusl: () => false,
    transport,
    run: async (binaryPath, args) => {
      runCalls.push({ binaryPath, args });
      return 0;
    },
    ...silent,
  };

  assert.equal(await main(options), 0);
  assert.equal(transport.calls.length, 2);

  const binaryPath = join(installDir, "mdhtml");
  assert.deepEqual(await readFile(binaryPath), binaryBytes);
  if (process.platform !== "win32") {
    assert.notEqual((await stat(binaryPath)).mode & 0o111, 0);
  }
  const state = JSON.parse(await readFile(join(installDir, ".mdhtml-install.json"), "utf8"));
  assert.equal(state.version, "1.2.3");
  assert.equal(state.target, "darwin-arm64");
  assert.equal(state.binarySha256, sha256(binaryBytes));

  assert.deepEqual(runCalls, [{ binaryPath, args: ["build", "doc.md"] }]);
});

test("a second run with an already-correct install performs no download", async () => {
  const installDir = await mkdtemp(join(tmpdir(), "mdhtml-idempotent-"));
  const binaryBytes = Buffer.from("#!/bin/sh\nexit 0\n");
  const archive = tarGzWith("mdhtml", binaryBytes);
  const transport = fakeTransport([
    { url: "mdhtml-1.2.3-linux-x64-gnu.tar.gz.sha256", body: Buffer.from(`${sha256(archive)}  mdhtml-1.2.3-linux-x64-gnu.tar.gz\n`) },
    { url: "mdhtml-1.2.3-linux-x64-gnu.tar.gz", body: archive },
  ]);
  const runCalls = [];
  const options = {
    argv: ["node", "mdhtml-cli.mjs", "check", "doc.md"],
    env: { MDHTML_VERSION: "1.2.3", MDHTML_INSTALL_DIR: installDir },
    platform: "linux",
    arch: "x64",
    isMusl: () => false,
    transport,
    run: async (binaryPath, args) => {
      runCalls.push({ binaryPath, args });
      return 3;
    },
    ...silent,
  };

  assert.equal(await main(options), 3);
  assert.equal(transport.calls.length, 2);

  runCalls.length = 0;
  const logs = [];
  assert.equal(await main({ ...options, log: (line) => logs.push(line) }), 3);
  assert.equal(transport.calls.length, 2);
  assert.equal(runCalls.length, 1);
  assert.match(logs.join("\n"), /already installed/);
});

test("a checksum mismatch refuses to install", async () => {
  const installDir = await mkdtemp(join(tmpdir(), "mdhtml-mismatch-"));
  const archive = tarGzWith("mdhtml", Buffer.from("payload"));
  const transport = fakeTransport([
    { url: "mdhtml-1.2.3-darwin-arm64.tar.gz.sha256", body: Buffer.from(`${"0".repeat(64)}  mdhtml-1.2.3-darwin-arm64.tar.gz\n`) },
    { url: "mdhtml-1.2.3-darwin-arm64.tar.gz", body: archive },
  ]);
  const errors = [];
  const result = await main({
    argv: ["node", "mdhtml-cli.mjs", "--version"],
    env: { MDHTML_VERSION: "1.2.3", MDHTML_INSTALL_DIR: installDir },
    platform: "darwin",
    arch: "arm64",
    isMusl: () => false,
    transport,
    run: async () => 0,
    log() {},
    logError: (line) => errors.push(line),
  });

  assert.equal(result, 1);
  assert.match(errors.join("\n"), /SHA-256 mismatch/);
  await assert.rejects(() => stat(join(installDir, binaryNameFor("darwin-arm64"))), /ENOENT/);
  await assert.rejects(() => readFile(join(installDir, ".mdhtml-install.json")), /ENOENT/);
});

test("main resolves the latest release via the release redirect", async () => {
  const installDir = await mkdtemp(join(tmpdir(), "mdhtml-latest-"));
  const binaryBytes = Buffer.from("#!/bin/sh\nexit 0\n");
  const archive = tarGzWith("mdhtml.exe", binaryBytes);
  const transport = fakeTransport([
    { url: "/releases/latest", location: "https://github.com/feliperun/md.html/releases/tag/v2.0.0" },
    { url: "mdhtml-2.0.0-windows-x64.tar.gz.sha256", body: Buffer.from(`${sha256(archive)}  mdhtml-2.0.0-windows-x64.tar.gz\n`) },
    { url: "mdhtml-2.0.0-windows-x64.tar.gz", body: archive },
  ]);
  const runCalls = [];
  const result = await main({
    argv: ["node", "mdhtml-cli.mjs", "themes"],
    env: { MDHTML_INSTALL_DIR: installDir },
    platform: "win32",
    arch: "x64",
    isMusl: () => false,
    transport,
    run: async (binaryPath, args) => {
      runCalls.push({ binaryPath, args });
      return 0;
    },
    ...silent,
  });

  assert.equal(result, 0);
  assert.equal(runCalls.length, 1);
  assert.equal(runCalls[0].binaryPath, join(installDir, "mdhtml.exe"));
  assert.deepEqual(runCalls[0].args, ["themes"]);
  assert.match(transport.calls[1], /mdhtml-2\.0\.0-windows-x64\.tar\.gz\.sha256$/);
});
