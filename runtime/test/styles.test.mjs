import { test } from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { mountStyles, runtimeCss } from "../src/styles.js";
import { BASE_CSS, EDITORIAL_CSS, TECHNICAL_CSS } from "../src/styles.generated.js";
import { generateStylesModule } from "../build-styles.mjs";
import { appDocument } from "./fake-dom.mjs";

const TOKENS = ["--md-bg", "--md-text", "--md-muted", "--md-surface", "--md-border", "--md-accent", "--md-focus"];

test("generated base CSS defines complete light, dark, and system token sets", () => {
  const css = BASE_CSS;
  for (const selector of [
    ":root",
    ':root[data-mdhtml-theme="light"]',
    ':root[data-mdhtml-theme="dark"]',
    ':root[data-mdhtml-theme="system"]',
  ]) {
    const start = css.indexOf(selector);
    assert.notEqual(start, -1, selector);
    const end = css.indexOf("}", start);
    const block = css.slice(start, end);
    for (const token of TOKENS) assert.match(block, new RegExp(`${token}:`, "u"), `${selector} ${token}`);
  }
  assert.match(css, /prefers-color-scheme:\s*dark/u);
  assert.match(css, /:focus-visible/u);
  assert.match(css, /@media print/u);
  assert.match(css, /#mdhtml-toolbar[^}]*display:\s*none/u);
  assert.match(css, /#mdhtml-toc[^}]*display:\s*none/u);
  assert.match(css, /#mdhtml-source-view[^}]*display:\s*none/u);
  assert.match(css, /#mdhtml-lightbox/u);
  assert.match(css, /data-md-reduced-motion[^}]*animation:\s*none/u);
  assert.match(css, /@media \(prefers-reduced-motion:\s*reduce\)[\s\S]*animation:\s*none/u);
  assert.match(css, /@media \(prefers-reduced-motion:\s*reduce\)[\s\S]*transition:\s*none/u);
  assert.match(css, /@media \(prefers-reduced-motion:\s*reduce\)[\s\S]*scroll-behavior:\s*auto/u);
  assert.match(css, /max-width:\s*none/u);
  assert.match(css, /overflow:\s*visible/u);
  assert.match(css, /background:\s*#fff/u);
  assert.match(css, /color:\s*#000/u);
});

test("presets contain tokens only and runtime CSS selects editorial explicitly", async () => {
  const fixture = JSON.parse(await readFile(fileURLToPath(new URL("../../fixtures/theme-presets.json", import.meta.url)), "utf8"));
  for (const selector of fixture.requiredSelectors) assert.match(BASE_CSS, new RegExp(selector.replace(/\./gu, "\\."), "u"), selector);
  for (const [name, css] of Object.entries({ technical: TECHNICAL_CSS, editorial: EDITORIAL_CSS })) {
    assert.match(css, /^:root\s*\{/u);
    assert.equal((css.match(/:root/g) ?? []).length, 1);
    assert.deepEqual([...css.matchAll(/(--md-[\w-]+):\s*([^;]+);/gu)].map(([, key, value]) => [key, value]), fixture.presets[name].tokens);
  }
  assert.equal(runtimeCss("editorial"), `${BASE_CSS}${EDITORIAL_CSS}`);
  assert.equal(runtimeCss("technical"), `${BASE_CSS}${TECHNICAL_CSS}`);
  assert.equal(runtimeCss("custom/path.theme.css"), `${BASE_CSS}${TECHNICAL_CSS}`);
});

test("styles generated module is byte-exactly reproducible from source CSS", async () => {
  const generated = await readFile(fileURLToPath(new URL("../src/styles.generated.js", import.meta.url)), "utf8");
  const [base, technical, editorial] = await Promise.all([
    readFile(fileURLToPath(new URL("../../themes/base.css", import.meta.url)), "utf8"),
    readFile(fileURLToPath(new URL("../../themes/technical.theme.css", import.meta.url)), "utf8"),
    readFile(fileURLToPath(new URL("../../themes/editorial.theme.css", import.meta.url)), "utf8"),
  ]);
  assert.equal(generateStylesModule({ base, technical, editorial }), generated);
});

test("mountStyles mounts one selected runtime style in head and is idempotent", () => {
  const doc = appDocument();
  const first = mountStyles(doc, "editorial");
  const second = mountStyles(doc, "editorial");
  assert.equal(first, second);
  assert.equal(first.tagName, "STYLE");
  assert.equal(first.id, "mdhtml-runtime-style");
  assert.equal(first.textContent, runtimeCss("editorial"));
  assert.equal(doc.documentElement.getAttribute("data-mdhtml-preset"), "editorial");
  assert.equal(doc.head.children.length, 1);
});
