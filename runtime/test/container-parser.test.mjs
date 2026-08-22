import { test } from "node:test";
import assert from "node:assert/strict";
import { parseMarkdownBlocks, parseMarkdown } from "../src/markdown.js";
import { loadFixtures } from "./fixtures.mjs";

test("container balancing ignores valid container syntax inside fenced code", async () => {
  const fixture = (await loadFixtures()).find((entry) => entry.id === "container-nesting");
  const example = fixture.expect.cases.find((entry) => entry.name === "fenced-code-content");
  assert.deepEqual(parseMarkdownBlocks(example.source).blocks, [example.expect.ast]);
});

test("container balancing handles nested containers after list markers", () => {
  const { blocks } = parseMarkdownBlocks("::: note\n- ::: mystery\n  Inner.\n  :::\n:::\n");
  assert.deepEqual(blocks, [
    {
      type: "container",
      name: "note",
      argument: null,
      children: [
        {
          type: "list",
          ordered: false,
          start: 1,
          items: [
            {
              checked: null,
              children: [
                {
                  type: "container",
                  name: "mystery",
                  argument: null,
                  children: [{ type: "paragraph", text: "Inner." }],
                },
              ],
            },
          ],
        },
      ],
    },
  ]);
});

test("container balancing handles nested containers after blockquote prefixes", () => {
  const { blocks } = parseMarkdownBlocks("::: note\n> - ::: mystery\n>   Inner.\n>   :::\n:::\n");
  assert.deepEqual(blocks, [
    {
      type: "container",
      name: "note",
      argument: null,
      children: [
        {
          type: "blockquote",
          children: [
            {
              type: "list",
              ordered: false,
              start: 1,
              items: [
                {
                  checked: null,
                  children: [
                    {
                      type: "container",
                      name: "mystery",
                      argument: null,
                      children: [{ type: "paragraph", text: "Inner." }],
                    },
                  ],
                },
              ],
            },
          ],
        },
      ],
    },
  ]);
});

test("container balancing ignores fenced-code syntax after structural prefixes", async () => {
  const fixture = (await loadFixtures()).find((entry) => entry.id === "container-nesting");
  for (const name of ["list-prefixed-fenced-code", "blockquote-prefixed-fenced-code"]) {
    const example = fixture.expect.cases.find((entry) => entry.name === name);
    assert.deepEqual(parseMarkdownBlocks(example.source).blocks, [example.expect.ast]);
  }
});

test("container inline enrichment recurses through children", () => {
  const { blocks } = parseMarkdown("::: note\n**body**\n:::\n");
  assert.deepEqual(blocks, [
    {
      type: "container",
      name: "note",
      argument: null,
      children: [
        {
          type: "paragraph",
          text: "**body**",
          children: [{ type: "strong", children: [{ type: "text", value: "body" }] }],
        },
      ],
    },
  ]);
});
