import { test } from "node:test";
import assert from "node:assert/strict";
import { parseFrontMatter } from "../src/frontmatter.js";
import { renderMarkdown } from "../src/render.js";
import { loadFixtures } from "./fixtures.mjs";

test("section bindings preserve map order, nested targets, and safe fallback", async () => {
  const fixture = (await loadFixtures()).find((entry) => entry.id === "section-bindings");
  assert.ok(fixture);
  const frontMatterEnd = fixture.source.indexOf("\n---", 4);
  const source = fixture.source.slice(frontMatterEnd + 5);
  const sections = {
    nested: { component: "timeline" },
    orphan: { component: "cards" },
    "bad-shape": [],
    "missing-component": { class: "good" },
    "non-string-component": { component: ["timeline"] },
    "bad-class": { component: "timeline", class: "good!" },
    target: { component: "timeline", class: "one two" },
    unknown: { component: "mystery" },
  };
  assert.deepEqual(renderMarkdown(source, { sections }), fixture.expect);
});

test("parsed numeric section keys preserve source order for bindings and orphan errors", () => {
  const parsed = parseFrontMatter("---\nsections:\n  2: { component: timeline }\n  1: { component: cards }\n  3: { component: hero }\n---\n# 1\nContent.\n");
  const result = renderMarkdown(parsed.body, { sections: parsed.frontMatter.sections });

  assert.deepEqual(result.bindings.map(({ slug }) => slug), ["2", "1", "3"]);
  assert.deepEqual(result.errors, [
    { code: "E-SECT-01", target: "2" },
    { code: "E-SECT-01", target: "3" },
  ]);
});

test("section component matrix matches the normative fixture", async () => {
  const fixture = (await loadFixtures()).find((entry) => entry.id === "section-components");
  assert.ok(fixture);
  const renderFixture = (entry) => {
    const parsed = parseFrontMatter(entry.source);
    return renderMarkdown(parsed.body, { sections: parsed.frontMatter.sections });
  };
  const actual = renderFixture(fixture);
  assert.equal(actual.html, Object.values(fixture.expect.html).join(""));
  assert.deepEqual(actual.warnings, fixture.expect.warnings);
  assert.deepEqual(actual.errors, fixture.expect.errors);
  assert.deepEqual(actual.bindings, fixture.expect.bindings);
  for (const testCase of fixture.cases) {
    const result = renderFixture(testCase);
    if (typeof testCase.expect.html === "string") {
      assert.deepEqual(result, testCase.expect, testCase.name);
    } else {
      assert.equal(result.html, Object.values(testCase.expect.html).join(""), testCase.name);
      assert.deepEqual(result.warnings, testCase.expect.warnings, testCase.name);
      assert.deepEqual(result.errors, testCase.expect.errors, testCase.name);
      assert.deepEqual(result.bindings, testCase.expect.bindings, testCase.name);
    }
  }
});

test("section heading footnotes are registered before bound body footnotes", () => {
  const source = "# Target[^heading]\n\n- Body[^body]\n\n[^heading]: Heading note.\n[^body]: Body note.\n";
  const result = renderMarkdown(source, { sections: { target: { component: "timeline" } } });

  assert.equal(result.html, '<section data-md-section="target"><h1 id="target">Target<sup><a href="#fn-heading">[heading]</a></sup></h1><div class="md-timeline"><ul><li>Body<sup><a href="#fn-body">[body]</a></sup></li></ul></div></section><section class="footnotes" data-md-footnotes><ol><li id="fn-heading"><p>Heading note.</p></li><li id="fn-body"><p>Body note.</p></li></ol></section>');
});
