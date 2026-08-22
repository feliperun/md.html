import { test } from "node:test";
import assert from "node:assert/strict";
import { renderMarkdown } from "../src/render.js";
import { loadFixtures } from "./fixtures.mjs";

test("simple containers match the golden fixture", async () => {
  const fixture = (await loadFixtures()).find((entry) => entry.id === "container-simple");
  assert.ok(fixture);
  assert.deepEqual(renderMarkdown(fixture.source), fixture.expect);
  for (const testCase of fixture.cases) {
    assert.deepEqual(renderMarkdown(testCase.source), testCase.expect, testCase.name);
  }
});

test("container rendering preserves shared heading identity and footnotes", () => {
  const result = renderMarkdown(
    "::: note\n# Same\n\nText[^n].\n:::\n\n# Same\n\n[^n]: Note.\n",
  );
  assert.equal(
    result.html,
    '<aside class="md-callout md-note"><span class="md-callout-badge">Note</span><section data-md-section="same"><h1 id="same">Same</h1><p>Text<sup><a href="#fn-n">[n]</a></sup>.</p></section></aside><section data-md-section="same-2"><h1 id="same-2">Same</h1></section><section class="footnotes" data-md-footnotes><ol><li id="fn-n"><p>Note.</p></li></ol></section>',
  );
  assert.deepEqual(result.headings, [
    { level: 1, id: "same", text: "Same" },
    { level: 1, id: "same-2", text: "Same" },
  ]);
});
