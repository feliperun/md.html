import { test } from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { build } from "../build.mjs";
import { boot, mountDocument } from "../src/bootstrap.js";
import { mountCore } from "../src/core.js";
import { decodeCanonicalSource } from "../src/canonical.js";
import { loadFixtures } from "./fixtures.mjs";
import { FakeElement, documentFor } from "./fake-dom.mjs";

const fixtureFile = fileURLToPath(new URL("../../fixtures/runtime-mount-basic.json", import.meta.url));
const forbidden = ["<", "/", "s", "c", "r", "i", "p", "t"].join("");
const encoded = ["<", "\\", "/", "s", "c", "r", "i", "p", "t"].join("");

function markdownElement(doc, id, textContent) {
  const source = doc ? doc.createElement("script") : new FakeElement("script", null);
  source.setAttribute("id", id);
  source.setAttribute("type", "text/markdown");
  source.textContent = textContent;
  return source;
}

test("mount fixture renders parsed body and preserves canonical source", async () => {
  const [fixture] = (await loadFixtures()).filter((entry) => entry.id === "runtime-mount-basic");
  const doc = documentFor(fixture.source);
  const original = doc.sourceElement.textContent;
  const result = mountDocument(doc);

  assert.deepEqual(result, fixture.expect);
  assert.equal(doc.sourceElement.textContent, original);
  assert.equal(doc.app.innerHTML, fixture.expect.html);
  assert.equal(doc.documentElement.getAttribute("data-mdhtml-runtime"), "1.0");
  assert.equal(doc.documentElement.getAttribute("data-mdhtml-ready"), "");
});

test("mount passes owned section bindings and returns binding evidence", () => {
  const source = "---\nsections:\n  target: { component: timeline, class: featured }\n  orphan: { component: cards }\n---\n# Target\n- One\n";
  const doc = documentFor(source);
  const result = mountDocument(doc);
  assert.equal(result.ok, true);
  assert.equal(result.html, '<section data-md-section="target" class="featured"><h1 id="target">Target</h1><div class="md-timeline"><ul><li>One</li></ul></div></section>');
  assert.deepEqual(result.warnings, []);
  assert.deepEqual(result.errors, [{ code: "E-SECT-01", target: "orphan" }]);
  assert.deepEqual(result.bindings, [
    { slug: "target", component: "timeline", class: "featured", valid: true, sectionClass: "featured" },
    { slug: "orphan", component: "cards", valid: false, error: { code: "E-SECT-01", target: "orphan" }, runtimeTarget: null },
  ]);
});

test("decodeCanonicalSource decodes only the rendering copy, case-insensitively", () => {
  const source = `${encoded} ${encoded.toUpperCase()}`;
  const expected = `${forbidden} ${forbidden}`;
  assert.equal(decodeCanonicalSource(source), expected);
  assert.notEqual(source, expected);
});

test("invalid document format fails before touching app innerHTML", () => {
  const doc = documentFor("# Title\n", { format: false });
  doc.app.innerHTML = "untouched";
  assert.deepEqual(mountDocument(doc), { ok: false, code: "E-FMT-01" });
  assert.equal(doc.app.innerHTML, "untouched");
});

test("duplicate markdown source fails before touching app innerHTML", () => {
  const doc = documentFor("# Title\n", {
    markdownScripts: [
      markdownElement(null, "mdhtml-source", "# One\n"),
      markdownElement(null, "other", "# Two\n"),
    ],
  });
  doc.app.innerHTML = "untouched";
  assert.deepEqual(mountDocument(doc), { ok: false, code: "E-FMT-01" });
  assert.equal(doc.app.innerHTML, "untouched");
});

test("missing markdown source fails before rendering or mutating source", () => {
  const doc = documentFor("# Title\n", { markdownScripts: [] });
  const original = doc.sourceElement.textContent;
  doc.app.innerHTML = "untouched";
  assert.deepEqual(mountDocument(doc), { ok: false, code: "E-FMT-01" });
  assert.equal(doc.sourceElement.textContent, original);
  assert.equal(doc.app.innerHTML, "untouched");
  assert.equal(doc.documentElement.getAttribute("data-mdhtml-runtime"), null);
  assert.equal(doc.documentElement.getAttribute("data-mdhtml-ready"), null);
});

test("missing app fails before rendering or mutating source", () => {
  const doc = documentFor("# Title\n", { apps: [] });
  const original = doc.sourceElement.textContent;
  doc.app.innerHTML = "untouched";
  assert.deepEqual(mountDocument(doc), { ok: false, code: "E-FMT-01" });
  assert.equal(doc.sourceElement.textContent, original);
  assert.equal(doc.app.innerHTML, "untouched");
  assert.equal(doc.documentElement.getAttribute("data-mdhtml-runtime"), null);
  assert.equal(doc.documentElement.getAttribute("data-mdhtml-ready"), null);
});

test("parse failures expose only the stable code", () => {
  const doc = documentFor("---\ntitle: [\n---\n# Broken\n");
  const result = mountDocument(doc);
  assert.deepEqual(result, { ok: false, code: "E-PARSE-01" });
  assert.equal(doc.app.innerHTML, "");
  assert.match(doc.app.textContent, /E-PARSE-01/);
  assert.doesNotMatch(doc.app.textContent, /stack|runtime|md\.html|node_modules|Error/u);
});

test("boot is a safe no-op without a document", () => {
  assert.equal(boot(undefined), undefined);
});

test("mountCore returns closed shared evidence and owns core surfaces", () => {
  const doc = documentFor("# Title\n");
  const evidence = mountCore(doc);
  assert.equal(Object.isFrozen(evidence), true);
  assert.equal(evidence.result.ok, true);
  assert.equal(evidence.storedSource, "# Title\n");
  assert.deepEqual(evidence.images, []);
  assert.equal(typeof evidence.projectMarkdown, "function");
  assert.equal(doc.getElementById("mdhtml-runtime-style").tagName, "STYLE");
  assert.equal(doc.getElementById("mdhtml-toolbar"), null);
});

test("boot mounts styles, chrome, and TOC only after a successful document render", () => {
  const doc = documentFor("# Title\n");
  const result = boot(doc);
  assert.equal(result.ok, true);
  assert.equal(doc.getElementById("mdhtml-runtime-style").tagName, "STYLE");
  assert.equal(doc.documentElement.getAttribute("data-mdhtml-preset"), "technical");
  assert.equal(doc.getElementById("mdhtml-toolbar").tagName, "NAV");
  assert.equal(doc.getElementById("mdhtml-toc").tagName, "NAV");
  assert.equal(doc.getElementById("mdhtml-source-view").tagName, "DIALOG");

  const failed = documentFor("---\ntitle: [\n---\n# Broken\n");
  assert.equal(boot(failed).ok, false);
  assert.equal(failed.getElementById("mdhtml-runtime-style"), null);
  assert.equal(failed.getElementById("mdhtml-toolbar"), null);
});

test("boot passes the selected built-in preset to the style mount", () => {
  const doc = documentFor("---\ntheme: editorial\n---\n# Title\n");
  const result = boot(doc);
  assert.equal(result.ok, true);
  assert.equal(doc.documentElement.getAttribute("data-mdhtml-preset"), "editorial");
  assert.match(doc.getElementById("mdhtml-runtime-style").textContent, /Newsreader/u);
});

test("boot falls back to technical for a hand-edited unknown theme", () => {
  const doc = documentFor("---\ntheme: custom\n---\n# Title\n");
  const result = boot(doc);
  assert.equal(result.ok, true);
  assert.equal(doc.documentElement.getAttribute("data-mdhtml-preset"), "technical");
  assert.match(doc.getElementById("mdhtml-runtime-style").textContent, /Instrument Sans/u);
});

test("boot hydrates mounted images before chrome and TOC integration", () => {
  const doc = documentFor("# Title\n\n![Logo](logo.png \"Logo\")\n");
  const block = doc.createElement("script");
  block.setAttribute("type", "application/octet-stream");
  block.setAttribute("data-path", "logo.png");
  block.setAttribute("data-type", "image/png");
  block.textContent = "AQID";
  doc.body.appendChild(block);

  const result = boot(doc);
  const [renderedImage] = doc.app.querySelectorAll("img");

  assert.equal(result.ok, true);
  assert.deepEqual(result.warnings, []);
  assert.equal(renderedImage.getAttribute("src"), "data:image/png;base64,AQID");
  assert.equal(renderedImage.getAttribute("data-md-asset-ready"), "");
  assert.equal(doc.getElementById("mdhtml-lightbox").tagName, "DIALOG");
  assert.equal(doc.getElementById("mdhtml-runtime-style").tagName, "STYLE");
  assert.equal(doc.getElementById("mdhtml-toolbar").tagName, "NAV");
  assert.equal(doc.getElementById("mdhtml-toc").tagName, "NAV");
});

test("generated runtime is the safe ASCII classic concatenation", async () => {
  const { runtime } = await build();
  const text = runtime.toString("ascii");
  assert.match(text, /^\(\(\)=>\{/u);
  assert.doesNotMatch(text, /\b(?:import|export)\b/u);
  for (const api of ["fetch", "history", "pushState", "setTimeout", "setInterval", "XMLHttpRequest"]) {
    assert.doesNotMatch(text, new RegExp(`\\b${api}\\b`, "u"), api);
  }
  assert.equal(text.toLowerCase().includes(forbidden), false);
  assert.equal([...text].every((character) => character.charCodeAt(0) < 128), true);
  assert.equal(text, (await readFile(fileURLToPath(new URL("../dist/runtime.min.js", import.meta.url)), "ascii")));
  for (const file of [
    fileURLToPath(new URL("../src/bootstrap.js", import.meta.url)),
    fileURLToPath(new URL("./bootstrap.test.mjs", import.meta.url)),
    fixtureFile,
  ]) {
    assert.equal((await readFile(file, "utf8")).toLowerCase().includes(forbidden), false, file);
  }
});
