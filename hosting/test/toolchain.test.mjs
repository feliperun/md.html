import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

import { buildWasm, loadToolchain } from "../src/toolchain.mjs";

const REPO_ROOT = fileURLToPath(new URL("../../", import.meta.url));
const SOURCE = "---\ntitle: Hello\n---\n\n# Hello\n\nBody.\n";

const pin = await buildWasm({ repoRoot: REPO_ROOT });
const toolchain = await loadToolchain({ repoRoot: REPO_ROOT, wasmPath: pin.wasmPath });

let builtHtml = null;

test("toolchain pin is self-consistent", async () => {
  assert.equal(pin.toolchainId, pin.sha256);
  const bytes = await readFile(pin.wasmPath);
  assert.equal(createHash("sha256").update(bytes).digest("hex"), pin.sha256);
  assert.equal(pin.sizeBytes, bytes.length);
});

test("build returns a portable artifact", async () => {
  const result = await toolchain.build({ source: SOURCE, assets: [] });
  assert.equal(result.exitCode, 0, result.stderr);
  assert.ok(result.html.includes('id="mdhtml-source"'), "html embeds the canonical source script");
  builtHtml = result.html;
});

test("audit of that artifact passes every category", async () => {
  assert.notEqual(builtHtml, null, "a prior build must have produced the artifact");
  const audit = await toolchain.audit(builtHtml);
  assert.equal(audit.exitCode, 0, audit.stderr);
  assert.equal(audit.report.safe, true);
  assert.equal(audit.report.sourceIntegrity, true);
  assert.equal(audit.report.html, "pass");
  assert.equal(audit.report.css, "pass");
  assert.equal(audit.report.runtime, "pass");
});
