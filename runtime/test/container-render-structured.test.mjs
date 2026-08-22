import { test } from "node:test";
import assert from "node:assert/strict";
import { renderMarkdown } from "../src/render.js";
import { loadFixtures } from "./fixtures.mjs";

test("structured containers match the focused golden fixture", async () => {
  const fixture = (await loadFixtures()).find((entry) => entry.id === "container-structured");
  assert.ok(fixture);
  assert.deepEqual(renderMarkdown(fixture.source), fixture.expect);
  for (const testCase of fixture.cases) {
    assert.deepEqual(renderMarkdown(testCase.source), testCase.expect, testCase.name);
  }
});
