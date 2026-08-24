// Phase 6 adversarial render walk: every `fixtures/security/mutation-xss-*`
// and valid `html-*` case renders its markdown body through the runtime
// renderer — the layer where mutation XSS actually lands — and asserts the
// payload markup never survives as a real element. The build verdicts for the
// same files are asserted by crates/mdhtml/tests/security_adversarial.rs and
// security_html.rs.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { parseFrontMatter } from "../src/frontmatter.js";
import { renderMarkdown } from "../src/render.js";

// Element names that would make a payload live if it survived rendering as
// markup. Escaped text shows up as `&lt;script`, so matching a literal `<`
// only fires on real elements the renderer emitted.
const FORBIDDEN_ELEMENTS =
  /<(script|iframe|object|embed|svg|img|form|input|base|meta|link|math|template|noscript|style|frame|applet|marquee|use|animate|set|foreignobject|video|audio|details|body)\b/i;
const EXECUTABLE_HREF = /<a\s[^>]*href="javascript:/i;

const dir = fileURLToPath(new URL("../../fixtures/security/", import.meta.url));
const names = readdirSync(dir)
  .filter((name) => /^(mutation-xss|html)-.*\.json$/.test(name))
  .sort();

test("the mutation-xss corpus is present", () => {
  const mutations = names.filter((name) => name.startsWith("mutation-xss-"));
  assert.ok(mutations.length >= 15, `expected at least 15 fixtures, found ${mutations.length}`);
});

for (const name of names) {
  const fixture = JSON.parse(readFileSync(`${dir}/${name}`, "utf8"));
  if (fixture.status !== "valid") continue;

  test(`${fixture.id}: payload never survives rendering as markup`, () => {
    const parsed = parseFrontMatter(fixture.source);
    const rendered = renderMarkdown(parsed.body);
    assert.match(fixture.id, /^(mutation-xss|html)-/, "id matches its file prefix");
    assert.ok(
      !FORBIDDEN_ELEMENTS.test(rendered.html),
      `${fixture.id} rendered executable markup: ${rendered.html}`,
    );
    assert.ok(
      !EXECUTABLE_HREF.test(rendered.html),
      `${fixture.id} rendered a javascript: href: ${rendered.html}`,
    );
  });
}
