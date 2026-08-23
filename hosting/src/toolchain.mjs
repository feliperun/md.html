import { createHash, randomUUID } from "node:crypto";
import { spawn } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join, relative, sep } from "node:path";

export { buildWasm } from "../scripts/build-wasm.mjs";

const CHILD_CODE = `
import { WASI } from "node:wasi";
import { readFileSync } from "node:fs";

const { WASI_ARGS, WASI_PATH, WASI_MOUNT, ...wasiEnv } = process.env;
const args = JSON.parse(WASI_ARGS);
const bytes = readFileSync(WASI_PATH);
const wasi = new WASI({
  version: "preview1",
  args,
  env: wasiEnv,
  preopens: { "/repo": WASI_MOUNT },
});
const { instance } = await WebAssembly.instantiate(bytes, {
  wasi_snapshot_preview1: wasi.wasiImport,
});
process.exitCode = wasi.start(instance);
`;

function sha256Hex(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

async function newWorkDir(repoRoot) {
  const workdir = join(repoRoot, ".runs", "hosting-work", randomUUID());
  await mkdir(workdir, { recursive: true });
  return workdir;
}

function guestPath(repoRoot, ...segments) {
  const fromRoot = relative(repoRoot, join(...segments)).split(sep).join("/");
  return `/repo/${fromRoot}`;
}

export async function invokeWasi({ repoRoot, wasmPath, args, env = {} }) {
  const result = await new Promise((resolve, reject) => {
    const child = spawn(
      process.execPath,
      ["--no-warnings", "--input-type=module", "-e", CHILD_CODE],
      {
        env: {
          ...process.env,
          ...env,
          WASI_ARGS: JSON.stringify(["mdhtml", ...args]),
          WASI_PATH: wasmPath,
          WASI_MOUNT: repoRoot,
        },
      },
    );
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => (stdout += chunk));
    child.stderr.on("data", (chunk) => (stderr += chunk));
    child.on("error", reject);
    child.on("close", (code) => resolve({ exitCode: code, stdout, stderr }));
  });
  return result;
}

export async function loadToolchain({ repoRoot, wasmPath }) {
  const bytes = await readFile(wasmPath);
  const toolchainId = sha256Hex(bytes);
  return {
    toolchainId,
    async build({ source, assets = [], sourceName = "document.md" }) {
      const workdir = await newWorkDir(repoRoot);
      const sourcePath = join(workdir, sourceName);
      await writeFile(sourcePath, Buffer.from(source, "utf8"));
      for (const asset of assets) {
        const assetPath = join(workdir, asset.name);
        await mkdir(dirname(assetPath), { recursive: true });
        await writeFile(assetPath, asset.bytes);
      }
      const outputName = `${sourceName}.html`;
      const invocation = await invokeWasi({
        repoRoot,
        wasmPath,
        args: [
          "build",
          guestPath(repoRoot, workdir, sourceName),
          "-o",
          guestPath(repoRoot, workdir, outputName),
        ],
        env: { MDHTML_ROOT: "/repo" },
      });
      if (invocation.exitCode !== 0) {
        return { html: null, exitCode: invocation.exitCode, stderr: invocation.stderr };
      }
      const html = await readFile(join(workdir, outputName), "utf8");
      return { html, exitCode: 0, stderr: "" };
    },
    async audit(html) {
      const workdir = await newWorkDir(repoRoot);
      const artifactName = "artifact.md.html";
      await writeFile(join(workdir, artifactName), Buffer.from(html, "utf8"));
      const invocation = await invokeWasi({
        repoRoot,
        wasmPath,
        args: ["audit", guestPath(repoRoot, workdir, artifactName), "--json"],
        env: { MDHTML_ROOT: "/repo" },
      });
      const report = JSON.parse(invocation.stdout);
      return {
        report,
        exitCode: invocation.exitCode,
        stdout: invocation.stdout,
        stderr: invocation.stderr,
      };
    },
  };
}
