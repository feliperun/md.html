import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import vm from "node:vm";
import { build, selectFragmentIds } from "../build.mjs";
import { appDocument, documentFor } from "./fake-dom.mjs";

const dist = fileURLToPath(new URL("../dist/", import.meta.url));
const fixtureFile = fileURLToPath(new URL("../../fixtures/runtime-fragments.json", import.meta.url));

function digest(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

async function committed(name) {
  return readFile(`${dist}/${name}`);
}

function run(bytes, document) {
  const context = { document, console };
  context.globalThis = context;
  vm.runInNewContext(bytes.toString("ascii"), context);
  return context;
}

test("manifest describes four fixed fragments and exact bytes", async () => {
  const { fragments, runtime, manifest } = await build();
  assert.deepEqual(manifest.fragments.map(({ id }) => id), ["core", "copy", "toc", "lightbox"]);
  assert.deepEqual(fragments.map(({ id }) => id), manifest.fragments.map(({ id }) => id));
  assert.deepEqual(runtime, Buffer.concat(fragments.map(({ bytes }) => bytes)));
  for (const fragment of manifest.fragments) {
    const bytes = fragments.find((entry) => entry.id === fragment.id).bytes;
    assert.equal(fragment.file, `${fragment.id}.min.js`);
    assert.equal(fragment.size, bytes.length);
    assert.equal(fragment.sha256, digest(bytes));
    assert.deepEqual(fragment.requires, fragment.id === "core" ? [] : ["core"]);
    assert.deepEqual(await committed(fragment.file), bytes);
  }
  assert.deepEqual(await committed("runtime.min.js"), runtime);
  assert.deepEqual(JSON.parse(await committed("manifest.json")), manifest);
});

test("selection is pure, evidence-based, normalized, and manifest ordered", async () => {
  const cases = JSON.parse(await readFile(fixtureFile, "utf8")).cases;
  for (const entry of cases) assert.deepEqual(selectFragmentIds(entry.analysis), entry.selected, entry.id);
  const evidence = { headings: [{ level: 2 }], config: { depth: 2, position: "inline" }, hasImages: true };
  assert.deepEqual(selectFragmentIds(evidence), ["core", "copy", "toc", "lightbox"]);
  assert.deepEqual(selectFragmentIds({ headings: [{ level: 4 }], config: { depth: 3 }, hasImages: false }), ["core", "copy"]);
});

test("optional fragments alone are harmless and core failure is not extended", async () => {
  const { fragments } = await build();
  const byId = new Map(fragments.map((fragment) => [fragment.id, fragment.bytes]));
  for (const id of ["copy", "toc", "lightbox"]) {
    const doc = appDocument();
    assert.doesNotThrow(() => run(byId.get(id), doc), id);
    assert.equal(doc.getElementById("mdhtml-toolbar"), null);
    assert.equal(doc.getElementById("mdhtml-toc"), null);
    assert.equal(doc.getElementById("mdhtml-lightbox"), null);
  }
  const failed = documentFor("---\ntitle: [\n---\n# Broken\n");
  assert.doesNotThrow(() => run(Buffer.concat([byId.get("core"), byId.get("copy"), byId.get("toc"), byId.get("lightbox")]), failed));
  assert.equal(failed.getElementById("mdhtml-toolbar"), null);
});

test("toc is harmless when core succeeds without the copy surface", async () => {
  const { fragments } = await build();
  const byId = new Map(fragments.map((fragment) => [fragment.id, fragment.bytes]));
  const doc = documentFor("# Title\n");
  assert.doesNotThrow(() => run(Buffer.concat([byId.get("core"), byId.get("toc")]), doc));
  assert.equal(doc.getElementById("mdhtml-toc"), null);
});

test("selected and full fragments execute as classic scripts in order", async () => {
  const { fragments, runtime } = await build();
  const selected = fragments.filter(({ id }) => ["core", "copy", "toc"].includes(id));
  const doc = documentFor("# Title\n");
  run(Buffer.concat(selected.map(({ bytes }) => bytes)), doc);
  assert.equal(doc.getElementById("mdhtml-toolbar").tagName, "NAV");
  assert.equal(doc.getElementById("mdhtml-toc").tagName, "NAV");
  assert.equal(doc.getElementById("mdhtml-lightbox"), null);

  const full = documentFor("# Title\n\n![Image](image.png)\n");
  const asset = full.createElement("script");
  asset.setAttribute("type", "application/octet-stream");
  asset.setAttribute("data-path", "image.png");
  asset.setAttribute("data-type", "image/png");
  asset.textContent = "AQID";
  full.body.appendChild(asset);
  run(runtime, full);
  assert.equal(full.getElementById("mdhtml-toolbar").tagName, "NAV");
  assert.equal(full.getElementById("mdhtml-toc").tagName, "NAV");
  assert.equal(full.getElementById("mdhtml-lightbox").tagName, "DIALOG");
});
