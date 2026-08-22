import { test } from "node:test";
import assert from "node:assert/strict";
import { parseInline, parseMarkdown } from "../src/markdown.js";
import { loadFixtures } from "./fixtures.mjs";

const plain = (object) => Object.fromEntries(Object.entries(object));

test("inline fixtures parse to the expected token arrays", async () => {
  const fixtures = (await loadFixtures()).filter((fixture) => fixture.id.startsWith("markdown-inline-"));
  assert.equal(fixtures.length, 6, "all inline fixture families must be discovered");
  for (const fixture of fixtures) {
    const result = fixture.expect.blocks !== undefined || fixture.expect.footnotes !== undefined
      ? parseMarkdown(fixture.source)
      : { inline: parseInline(fixture.source) };
    if (fixture.expect.inline !== undefined) {
      assert.deepEqual(result.inline, fixture.expect.inline, `${fixture.id}: inline`);
    }
    if (fixture.expect.blocks !== undefined) {
      assert.deepEqual(result.blocks, fixture.expect.blocks, `${fixture.id}: blocks`);
    }
    if (fixture.expect.references !== undefined) {
      assert.deepEqual(plain(result.references), fixture.expect.references, `${fixture.id}: references`);
    }
    if (fixture.expect.footnotes !== undefined) {
      assert.deepEqual(plain(result.footnotes), fixture.expect.footnotes, `${fixture.id}: footnotes`);
    }
  }
});

test("parseMarkdown enriches nested blocks without replacing raw block text", () => {
  const result = parseMarkdown("# **Title**\n\n- [x] *item*\n  - nested\n\n| **head** | cell |\n| --- | --- |\n| row | `code` |\n");
  assert.equal(result.blocks[0].text, "**Title**");
  assert.deepEqual(result.blocks[0].children, [
    { type: "strong", children: [{ type: "text", value: "Title" }] },
  ]);
  assert.equal(result.blocks[1].items[0].children[0].text, "*item*");
  assert.deepEqual(result.blocks[1].items[0].children[0].children, [
    { type: "emphasis", children: [{ type: "text", value: "item" }] },
  ]);
  assert.deepEqual(result.blocks[2].header, ["**head**", "cell"]);
  assert.deepEqual(result.blocks[2].headerInlines, [
    [{ type: "strong", children: [{ type: "text", value: "head" }] }],
    [{ type: "text", value: "cell" }],
  ]);
  assert.deepEqual(result.blocks[2].rowInlines, [[
    [{ type: "text", value: "row" }],
    [{ type: "code", value: "code" }],
  ]]);
});

test("escaped backticks stay text instead of opening a code span", () => {
  assert.deepEqual(parseInline("\\`code\\`"), [{ type: "text", value: "`code`" }]);
});
