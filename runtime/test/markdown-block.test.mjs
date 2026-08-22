import { test } from "node:test";
import assert from "node:assert/strict";
import { parseMarkdownBlocks } from "../src/markdown.js";
import { loadFixtures } from "./fixtures.mjs";

const plain = (object) => Object.fromEntries(Object.entries(object));

test("markdown-block fixtures parse to the expected AST", async () => {
  const fixtures = (await loadFixtures()).filter((fixture) => fixture.id.startsWith("markdown-block-"));
  assert.ok(fixtures.length >= 7, "all markdown-block fixtures must be discovered");
  for (const fixture of fixtures) {
    const result = parseMarkdownBlocks(fixture.source);
    assert.deepEqual(result.blocks, fixture.expect.blocks, `${fixture.id}: blocks`);
    assert.deepEqual(plain(result.references), fixture.expect.references ?? {}, `${fixture.id}: references`);
    assert.deepEqual(plain(result.footnotes), fixture.expect.footnotes ?? {}, `${fixture.id}: footnotes`);
  }
});

test("ATX headings require a space after the opening hashes", () => {
  const { blocks } = parseMarkdownBlocks("# Title\n#5 bolt\n\n####### seven\n");
  assert.deepEqual(blocks, [
    { type: "heading", level: 1, text: "Title" },
    { type: "paragraph", text: "#5 bolt" },
    { type: "paragraph", text: "####### seven" },
  ]);
});

test("thematic breaks beat list markers", () => {
  const { blocks } = parseMarkdownBlocks("- - -\n***\n___\n");
  assert.deepEqual(blocks, [
    { type: "thematicBreak" },
    { type: "thematicBreak" },
    { type: "thematicBreak" },
  ]);
});

test("fenced code preserves content bytes and optional language", () => {
  const { blocks } = parseMarkdownBlocks("```js\nlet x = 1;\n```\n");
  assert.deepEqual(blocks, [{ type: "codeBlock", language: "js", value: "let x = 1;\n" }]);
});

test("an unclosed fence consumes to the end of the document", () => {
  const { blocks } = parseMarkdownBlocks("```\na\nb\n");
  assert.deepEqual(blocks, [{ type: "codeBlock", language: null, value: "a\nb\n" }]);
});

test("reference and footnote maps are null-prototype", () => {
  const result = parseMarkdownBlocks("[a]: /url\n[^n]: note\n");
  assert.equal(Object.getPrototypeOf(result.references), null);
  assert.equal(Object.getPrototypeOf(result.footnotes), null);
});

test("definition-like lines inside a paragraph stay paragraph text", () => {
  const { blocks, references } = parseMarkdownBlocks("para\n[a]: /url\n");
  assert.deepEqual(blocks, [{ type: "paragraph", text: "para\n[a]: /url" }]);
  assert.deepEqual(plain(references), {});
});

test("reference ids are case-insensitive with collapsed whitespace", () => {
  const { references } = parseMarkdownBlocks("[A  B]: /one\n[a b]: /two\n");
  assert.deepEqual(plain(references), { "a b": { url: "/one", title: null } });
});

test("first definition wins for references and footnotes", () => {
  const { references, footnotes } = parseMarkdownBlocks("[a]: /one\n[a]: /two\n[^n]: first\n[^n]: second\n");
  assert.deepEqual(plain(references), { a: { url: "/one", title: null } });
  assert.deepEqual(plain(footnotes), { n: [{ type: "paragraph", text: "first" }] });
});

test("nested lists and blockquotes round-trip content without loss", () => {
  const { blocks } = parseMarkdownBlocks("- a\n  - b\n    > c\n    >\n    > d\n- e\n");
  assert.deepEqual(blocks, [
    {
      type: "list",
      ordered: false,
      start: 1,
      items: [
        {
          checked: null,
          children: [
            { type: "paragraph", text: "a" },
            {
              type: "list",
              ordered: false,
              start: 1,
              items: [
                {
                  checked: null,
                  children: [
                    { type: "paragraph", text: "b" },
                    {
                      type: "blockquote",
                      children: [
                        { type: "paragraph", text: "c" },
                        { type: "paragraph", text: "d" },
                      ],
                    },
                  ],
                },
              ],
            },
          ],
        },
        { checked: null, children: [{ type: "paragraph", text: "e" }] },
      ],
    },
  ]);
});

test("paragraph text preserves hard-break trailing spaces", () => {
  const { blocks } = parseMarkdownBlocks("first  \nsecond\n");
  assert.deepEqual(blocks, [{ type: "paragraph", text: "first  \nsecond" }]);
});

test("top-level paragraph text preserves leading whitespace", () => {
  const { blocks } = parseMarkdownBlocks("a\n  b\n");
  assert.deepEqual(blocks, [{ type: "paragraph", text: "a\n  b" }]);
});

test("lazy blockquote continuation preserves leading whitespace", () => {
  const { blocks } = parseMarkdownBlocks("> a\n  b\n");
  assert.deepEqual(blocks, [
    { type: "blockquote", children: [{ type: "paragraph", text: "a\n  b" }] },
  ]);
});

test("lazy list continuation preserves leading whitespace", () => {
  const { blocks } = parseMarkdownBlocks("- a\n b\n");
  assert.deepEqual(blocks, [
    {
      type: "list",
      ordered: false,
      start: 1,
      items: [{ checked: null, children: [{ type: "paragraph", text: "a\n b" }] }],
    },
  ]);
});

test("closed fence keeps final content newline when the source ends with one", () => {
  const { blocks } = parseMarkdownBlocks("```\nalpha\n```\n");
  assert.deepEqual(blocks, [{ type: "codeBlock", language: null, value: "alpha\n" }]);
});

test("closed fence keeps final content newline without a trailing source newline", () => {
  const { blocks } = parseMarkdownBlocks("```\nalpha\n```");
  assert.deepEqual(blocks, [{ type: "codeBlock", language: null, value: "alpha\n" }]);
});

test("unclosed fence keeps the source's final newline", () => {
  const { blocks } = parseMarkdownBlocks("```\nalpha\n");
  assert.deepEqual(blocks, [{ type: "codeBlock", language: null, value: "alpha\n" }]);
});

test("unclosed fence does not invent a trailing newline", () => {
  const { blocks } = parseMarkdownBlocks("```\nalpha");
  assert.deepEqual(blocks, [{ type: "codeBlock", language: null, value: "alpha" }]);
});

test("tables normalize alignment and row cell counts", () => {
  const { blocks } = parseMarkdownBlocks("| a | b |\n| :--- | ---: |\n| 1 |\n| 1 | 2 | 3 |\n");
  assert.deepEqual(blocks, [
    {
      type: "table",
      align: ["left", "right"],
      header: ["a", "b"],
      rows: [["1", ""], ["1", "2"]],
      sourceCellCounts: { header: 2, rows: [1, 3] },
    },
  ]);
});

test("hostile input terminates and never drops content", () => {
  const lines = [];
  for (let i = 0; i < 200; i++) lines.push(`- item ${i}`, `> quote ${i}`);
  const result = parseMarkdownBlocks(lines.join("\n") + "\n");
  const texts = [];
  const walk = (blocks) => {
    for (const block of blocks) {
      if (block.type === "paragraph") texts.push(block.text);
      else if (block.type === "blockquote") walk(block.children);
      else if (block.type === "list") for (const item of block.items) walk(item.children);
    }
  };
  walk(result.blocks);
  const joined = texts.join("\n");
  for (let i = 0; i < 200; i++) {
    assert.ok(joined.includes(`item ${i}`), `item ${i} must survive`);
    assert.ok(joined.includes(`quote ${i}`), `quote ${i} must survive`);
  }
});
