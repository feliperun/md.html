import { randomUUID } from "node:crypto";
import { readFile, rename, rm, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const RUNTIME = dirname(fileURLToPath(import.meta.url));
const ROOT = dirname(RUNTIME);
const SOURCES = {
  base: join(ROOT, "themes/base.css"),
  technical: join(ROOT, "themes/technical.theme.css"),
  editorial: join(ROOT, "themes/editorial.theme.css"),
};
const OUTPUT = join(RUNTIME, "src/styles.generated.js");

export function generateStylesModule({ base, technical, editorial }) {
  return [
    `export const BASE_CSS = ${JSON.stringify(base)};`,
    `export const TECHNICAL_CSS = ${JSON.stringify(technical)};`,
    `export const EDITORIAL_CSS = ${JSON.stringify(editorial)};`,
    "",
  ].join("\n");
}

async function readSources() {
  const [base, technical, editorial] = await Promise.all([
    readFile(SOURCES.base, "utf8"),
    readFile(SOURCES.technical, "utf8"),
    readFile(SOURCES.editorial, "utf8"),
  ]);
  return { base, technical, editorial };
}

export async function generatedStyles() {
  return generateStylesModule(await readSources());
}

export async function build() {
  const generated = await generatedStyles();
  const temporary = `${OUTPUT}.${randomUUID()}.tmp`;
  try {
    await writeFile(temporary, generated);
    await rename(temporary, OUTPUT);
  } finally {
    await rm(temporary, { force: true });
  }
  return generated;
}

export async function check() {
  const generated = await generatedStyles();
  let committed;
  try {
    committed = await readFile(OUTPUT, "utf8");
  } catch (error) {
    if (error.code === "ENOENT") committed = null;
    else throw error;
  }
  const ok = committed === generated;
  if (!ok) console.error("styles.generated.js: missing or drifted from source CSS");
  return ok;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const command = process.argv[2] ?? "build";
  const operation = command === "check" ? check : build;
  operation().then((ok) => {
    if (command === "check") {
      if (ok) console.log("check: generated styles are up to date");
      else process.exitCode = 1;
    } else {
      console.log("build: wrote runtime/src/styles.generated.js");
    }
  }).catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
