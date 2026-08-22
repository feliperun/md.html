import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { build as esbuild } from "esbuild";
import { build as buildStyles, check as checkStyles } from "./build-styles.mjs";

const ROOT = fileURLToPath(new URL("../", import.meta.url));
const SRC = join(ROOT, "runtime/src");
const DIST = join(ROOT, "runtime/dist");
const MANIFEST_FILE = "manifest.json";

const FRAGMENT_DEFINITIONS = [
  { id: "core", source: "entry-core.js", file: "core.min.js", requires: [] },
  { id: "copy", source: "entry-copy.js", file: "copy.min.js", requires: ["core"] },
  { id: "toc", source: "entry-toc.js", file: "toc.min.js", requires: ["core"] },
  { id: "lightbox", source: "entry-lightbox.js", file: "lightbox.min.js", requires: ["core"] },
];

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function serialize(manifest) {
  return `${JSON.stringify(manifest, null, 2)}\n`;
}

function normalizeBytes(bytes) {
  const text = Buffer.from(bytes).toString("utf8").replace(/\r\n?/gu, "\n");
  const normalized = Buffer.from(text, "utf8");
  if ([...normalized].some((byte) => byte > 0x7f)) throw new Error("runtime fragment is not ASCII");
  return normalized;
}

function normalizedDepth(config) {
  return Number.isInteger(config?.depth) && config.depth >= 1 && config.depth <= 6 ? config.depth : 3;
}

export function selectFragmentIds(analysis = {}) {
  const config = analysis.config;
  const depth = normalizedDepth(config);
  const hasTocHeading = config !== false && (analysis.headings ?? []).some((heading) =>
    Number.isInteger(heading?.level) && heading.level >= 1 && heading.level <= depth);
  return FRAGMENT_DEFINITIONS
    .filter(({ id }) => id === "core" || id === "copy" || (id === "toc" && hasTocHeading) || (id === "lightbox" && analysis.hasImages === true))
    .map(({ id }) => id);
}

async function bundle(definition) {
  const result = await esbuild({
    entryPoints: [join(SRC, definition.source)],
    bundle: true,
    minify: true,
    write: false,
    format: "iife",
    charset: "ascii",
    platform: "browser",
  });
  return normalizeBytes(result.outputFiles[0].contents);
}

function manifestFor(fragments) {
  return {
    format: "mdhtml/manifest/1.0",
    fragments: fragments.map(({ id, file, requires, bytes }) => ({
      id,
      file,
      size: bytes.length,
      sha256: sha256(bytes),
      requires,
    })),
  };
}

async function generate() {
  const fragments = [];
  for (const definition of FRAGMENT_DEFINITIONS) {
    const bytes = await bundle(definition);
    fragments.push({ ...definition, bytes });
  }
  const manifest = manifestFor(fragments);
  const runtime = Buffer.concat(fragments.map(({ bytes }) => bytes));
  return { fragments, manifest, runtime };
}

export async function build() {
  await buildStyles();
  return generate();
}

async function committed(name) {
  try {
    return await readFile(join(DIST, name));
  } catch (error) {
    if (error.code === "ENOENT") return null;
    throw error;
  }
}

export async function check() {
  const stylesOk = await checkStyles();
  const result = await generate();
  const problems = [];
  for (const fragment of result.fragments) {
    const actual = await committed(fragment.file);
    if (actual === null || !actual.equals(fragment.bytes)) problems.push(`${fragment.file}: missing or drifted from a fresh build`);
  }
  const runtime = await committed("runtime.min.js");
  if (runtime === null || !runtime.equals(result.runtime)) problems.push("runtime.min.js: missing or drifted from a fresh build");
  const manifest = await committed(MANIFEST_FILE);
  if (manifest === null || !manifest.equals(Buffer.from(serialize(result.manifest), "utf8"))) {
    problems.push(`${MANIFEST_FILE}: missing or drifted from a fresh build`);
  }
  if (!stylesOk) problems.unshift("styles.generated.js: missing or drifted from source CSS");
  if (problems.length > 0) {
    console.error(`runtime artifact drift:\n${problems.join("\n")}`);
    process.exitCode = 1;
    return false;
  }
  console.log("check: runtime artifacts are up to date");
  return true;
}

async function main() {
  if (process.argv[2] === "check") {
    await check();
    return;
  }
  const result = await build();
  await mkdir(DIST, { recursive: true });
  for (const fragment of result.fragments) await writeFile(join(DIST, fragment.file), fragment.bytes);
  await writeFile(join(DIST, "runtime.min.js"), result.runtime);
  await writeFile(join(DIST, MANIFEST_FILE), serialize(result.manifest));
  console.log("build: wrote four fragments, runtime.min.js, and manifest.json");
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
