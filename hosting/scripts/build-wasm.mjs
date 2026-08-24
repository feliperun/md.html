import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_REPO_ROOT = fileURLToPath(new URL("../../", import.meta.url));

export async function buildWasm({ repoRoot }) {
  const root = repoRoot ?? SCRIPT_REPO_ROOT;
  const targetDir = join(root, ".runs", "cargo-target");
  const build = spawnSync(
    "cargo",
    ["build", "-p", "mdhtml", "--release", "--target", "wasm32-wasip1", "--target-dir", targetDir],
    { cwd: root, stdio: "inherit" },
  );
  if (build.status !== 0) {
    throw new Error(`cargo build failed with exit code ${build.status}`);
  }
  const wasmPath = join(targetDir, "wasm32-wasip1", "release", "mdhtml.wasm");
  const bytes = await readFile(wasmPath);
  const sha256 = createHash("sha256").update(bytes).digest("hex");
  const pin = { toolchainId: sha256, sha256, sizeBytes: bytes.length, wasmPath };
  await mkdir(join(root, ".runs"), { recursive: true });
  await writeFile(
    join(root, ".runs", "hosting-toolchain-pin.json"),
    `${JSON.stringify(pin)}\n`,
  );
  return pin;
}

async function main() {
  const pin = await buildWasm({ repoRoot: SCRIPT_REPO_ROOT });
  process.stdout.write(`${JSON.stringify(pin)}\n`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
