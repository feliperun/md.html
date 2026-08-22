import { test } from "node:test";
import assert from "node:assert/strict";
import { renderDocument, renderMarkdown, slugify } from "../src/render.js";
import { parseMarkdown } from "../src/markdown.js";
import { loadFixtures } from "./fixtures.mjs";

test("render fixtures match byte-for-byte output", async () => {
  const fixtures = (await loadFixtures()).filter((fixture) => fixture.id.startsWith("render-"));
  assert.equal(fixtures.length, 5, "all renderer fixtures must be discovered");
  for (const fixture of fixtures) {
    assert.deepEqual(renderMarkdown(fixture.source), fixture.expect, fixture.id);
  }
});

test("slugify implements SECT-01", () => {
  assert.equal(slugify("Crème brûlée / 2"), "creme-brulee--2");
  assert.equal(slugify("  A\tB\nC  "), "-a-b-c-");
  assert.equal(slugify("Symbols: * & ?"), "symbols---");
});

test("renderDocument does not mutate the accepted AST", () => {
  const parsed = parseMarkdown("# Title\n\n- **item**\n");
  const beforeBlocks = structuredClone(parsed.blocks);
  const beforeReferences = Object.fromEntries(Object.entries(parsed.references));
  const beforeFootnotes = Object.fromEntries(Object.entries(parsed.footnotes));
  renderDocument(parsed);
  assert.deepEqual(parsed.blocks, beforeBlocks);
  assert.deepEqual(Object.fromEntries(Object.entries(parsed.references)), beforeReferences);
  assert.deepEqual(Object.fromEntries(Object.entries(parsed.footnotes)), beforeFootnotes);
});

test("renderMarkdown and renderDocument return the same shape", () => {
  const source = "# Title\n\nText\n";
  assert.deepEqual(renderMarkdown(source), renderDocument(parseMarkdown(source)));
});

test("duplicate explicit section ids warn before uniqueness suffixing", () => {
  const result = renderMarkdown("# One {#Same ID}\n\n# Two {#same-id}\n");
  assert.deepEqual(result.headings, [
    { level: 1, id: "same-id", text: "One" },
    { level: 1, id: "same-id-2", text: "Two" },
  ]);
  assert.deepEqual(result.warnings, [{ code: "W-SECT-01" }]);
});
