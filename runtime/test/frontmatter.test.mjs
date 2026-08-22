import { test } from "node:test";
import assert from "node:assert/strict";
import { orderedMappingEntries, parseFrontMatter, FrontMatterError } from "../src/frontmatter.js";
import { loadFixtures } from "./fixtures.mjs";

test("missing opening delimiter returns the whole source as body", () => {
  const source = "# Title\nbody\n";
  assert.deepEqual(parseFrontMatter(source), {
    frontMatter: {},
    body: source,
    raw: "",
    bodyOffset: 0,
  });
});

test("empty front matter parses to an empty object", () => {
  const result = parseFrontMatter("---\n---\n# Body\n");
  assert.deepEqual(result.frontMatter, {});
  assert.equal(result.body, "# Body\n");
  assert.equal(result.raw, "---\n---\n");
  assert.equal(result.bodyOffset, 8);
});

test("mapping source order is hidden without changing plain mapping behavior", () => {
  const result = parseFrontMatter("---\nsections:\n  2: { component: cards }\n  1: { component: timeline }\n---\n");
  assert.deepEqual(result.frontMatter.sections, {
    1: { component: "timeline" },
    2: { component: "cards" },
  });
  assert.deepEqual(Object.keys(result.frontMatter.sections), ["1", "2"]);
  assert.deepEqual(orderedMappingEntries(result.frontMatter.sections), [
    ["2", { component: "cards" }],
    ["1", { component: "timeline" }],
  ]);
  assert.deepEqual(orderedMappingEntries({ 2: "cards", 1: "timeline" }), [
    ["1", "timeline"],
    ["2", "cards"],
  ]);
});

test("comment-only front matter yields an empty object", () => {
  const result = parseFrontMatter("---\n# just a comment\n---\nbody\n");
  assert.deepEqual(result.frontMatter, {});
  assert.equal(result.body, "body\n");
});

test("front matter at EOF without trailing newline", () => {
  const source = "---\ntitle: A\n---";
  const result = parseFrontMatter(source);
  assert.deepEqual(result.frontMatter, { title: "A" });
  assert.equal(result.body, "");
  assert.equal(result.bodyOffset, source.length);
  assert.equal(result.raw, source);
});

test("scalar resolution covers ints, floats, booleans and null", () => {
  const result = parseFrontMatter(
    "---\nint: 42\nneg: -7\nfloat: 3.14\nexp: 1e3\nsci: 1.5e-2\nt: true\nf: false\nn: null\ns: hello\n---\n",
  );
  assert.deepEqual(result.frontMatter, {
    int: 42,
    neg: -7,
    float: 3.14,
    exp: 1000,
    sci: 0.015,
    t: true,
    f: false,
    n: null,
    s: "hello",
  });
});

test("numeric scalar overflow is rejected with a stable one-based position", () => {
  let error;
  try {
    parseFrontMatter("---\nvalue: 1e309\n---\n");
  } catch (err) {
    error = err;
  }
  assert.ok(error instanceof FrontMatterError);
  assert.equal(error.code, "E-PARSE-01");
  assert.equal(error.line, 2);
  assert.equal(error.column, 8);
});

test("quoted scalars resolve escapes and comments stay outside quotes", () => {
  const result = parseFrontMatter('---\nsingle: \'it\'\'s\'\ndouble: "a\\tb"\nkey: plain # comment\n---\n');
  assert.deepEqual(result.frontMatter, {
    single: "it's",
    double: "a\tb",
    key: "plain",
  });
});

test("literal and folded blocks produce exact strings", () => {
  const result = parseFrontMatter("---\nl: |\n  one\n  two\nf: >\n  one\n  two\n---\n");
  assert.equal(result.frontMatter.l, "one\ntwo\n");
  assert.equal(result.frontMatter.f, "one two\n");
});

test("CRLF line endings preserve the body byte for byte", () => {
  const source = "---\r\ntitle: A\r\n---\r\n\r\n# Body\r\n";
  const result = parseFrontMatter(source);
  assert.deepEqual(result.frontMatter, { title: "A" });
  assert.equal(result.body, "\r\n# Body\r\n");
  assert.equal(result.raw, "---\r\ntitle: A\r\n---\r\n");
});

test("errors report one-based line and column without leaking paths", () => {
  let error;
  try {
    parseFrontMatter("---\na: 1\n b: 2\n---\n");
  } catch (err) {
    error = err;
  }
  assert.ok(error instanceof FrontMatterError);
  assert.equal(error.code, "E-PARSE-01");
  assert.equal(error.name, "FrontMatterError");
  assert.equal(error.line, 3);
  assert.equal(error.column, 2);
  assert.equal(typeof error.message, "string");
  assert.ok(error.message.length > 0);
  assert.ok(!error.message.includes("/"), "message must not expose internal paths");
});

test("tab indentation is rejected with a precise location", () => {
  let error;
  try {
    parseFrontMatter("---\na: 1\n\tb: 2\n---\n");
  } catch (err) {
    error = err;
  }
  assert.ok(error instanceof FrontMatterError);
  assert.equal(error.code, "E-PARSE-01");
  assert.equal(error.line, 3);
  assert.equal(error.column, 1);
});

test("anchors and aliases are rejected", () => {
  assert.throws(
    () => parseFrontMatter("---\nanchor: &shared\n  name: hidden\ncopy: *shared\n---\n"),
    (err) => err instanceof FrontMatterError && err.code === "E-PARSE-01",
  );
});

test("tags, anchors and aliases are rejected as mapping keys", () => {
  for (const source of [
    "---\n!tag: value\n---\n",
    "---\n&anchor: value\n---\n",
    "---\n*alias: value\n---\n",
  ]) {
    assert.throws(
      () => parseFrontMatter(source),
      (err) => err instanceof FrontMatterError && err.code === "E-PARSE-01",
      source,
    );
  }
});

test("tags, anchors and aliases are rejected as keys inside sequence items", () => {
  for (const marker of ["!tag", "&anchor", "*alias"]) {
    const source = `---\nitems:\n  - ${marker}: value\n---\n`;
    assert.throws(
      () => parseFrontMatter(source),
      (err) => err instanceof FrontMatterError && err.code === "E-PARSE-01",
      source,
    );
  }
});

test("quoted keys parse inside sequence-item maps", () => {
  for (const quote of ['"', "'"]) {
    const result = parseFrontMatter(`---\nitems:\n  - ${quote}name${quote}: Ada\n---\n`);
    assert.deepEqual(result.frontMatter, { items: [{ name: "Ada" }] }, quote);
  }
});

test("quoted keys parse across multi-entry sequence-item maps", () => {
  const result = parseFrontMatter('---\nitems:\n  - "name": Ada\n    "role": dev\n---\n');
  assert.deepEqual(result.frontMatter, { items: [{ name: "Ada", role: "dev" }] });
});

test("tags, anchors and aliases are rejected as flow mapping keys", () => {
  for (const marker of ["!tag", "&anchor", "*alias"]) {
    const source = `---\na: { ${marker}: value }\n---\n`;
    assert.throws(
      () => parseFrontMatter(source),
      (err) => err instanceof FrontMatterError && err.code === "E-PARSE-01",
      source,
    );
  }
});

test("plain values with a mapping colon are rejected", () => {
  assert.throws(
    () => parseFrontMatter("---\na: one: two\n---\n"),
    (err) => err instanceof FrontMatterError && err.code === "E-PARSE-01",
  );
});

test("quoted and flow values require whitespace before a trailing comment", () => {
  for (const source of [
    '---\na: "value"#not-comment\n---\n',
    "---\na: [1]#not-comment\n---\n",
    "---\na: { b: 1 }#not-comment\n---\n",
  ]) {
    assert.throws(
      () => parseFrontMatter(source),
      (err) => err instanceof FrontMatterError && err.code === "E-PARSE-01",
      source,
    );
  }
});

test("quoted and flow values keep space-separated trailing comments", () => {
  const result = parseFrontMatter('---\na: "x" # comment\nb: [1] # comment\nc: { d: 2 } # comment\n---\n');
  assert.deepEqual(result.frontMatter, { a: "x", b: [1], c: { d: 2 } });
});

test("duplicate keys are detected after quoted/plain resolution", () => {
  let error;
  try {
    parseFrontMatter('---\na: 1\n"a": 2\n---\n');
  } catch (err) {
    error = err;
  }
  assert.ok(error instanceof FrontMatterError);
  assert.match(error.message, /duplicate key/);
  assert.equal(error.line, 3);
});

test("unterminated front matter is rejected at EOF", () => {
  let error;
  try {
    parseFrontMatter("---\ntitle: x\n");
  } catch (err) {
    error = err;
  }
  assert.ok(error instanceof FrontMatterError);
  assert.equal(error.code, "E-PARSE-01");
  assert.equal(error.line, 3);
});

test("valid front matter fixtures parse to their expected value", async () => {
  const fixtures = (await loadFixtures()).filter((fixture) => fixture.id.startsWith("frontmatter-valid"));
  assert.ok(fixtures.length >= 6, "all valid front matter fixtures must be discovered");
  for (const fixture of fixtures) {
    const result = parseFrontMatter(fixture.source);
    assert.deepEqual(result.frontMatter, fixture.expect.frontMatter, `${fixture.id}: frontMatter`);
    assert.equal(result.raw + result.body, fixture.source, `${fixture.id}: source reassembles`);
    assert.equal(result.bodyOffset, result.raw.length, `${fixture.id}: bodyOffset`);
    assert.ok(result.bodyOffset >= 0 && result.bodyOffset <= fixture.source.length, `${fixture.id}: bounds`);
  }
});

test("invalid front matter fixtures reject with E-PARSE-01", async () => {
  const fixtures = (await loadFixtures()).filter((fixture) => fixture.id.startsWith("frontmatter-invalid"));
  assert.ok(fixtures.length >= 7, "all invalid front matter fixtures must be discovered");
  for (const fixture of fixtures) {
    assert.throws(
      () => parseFrontMatter(fixture.source),
      (err) => {
        assert.ok(err instanceof FrontMatterError, `${fixture.id}: FrontMatterError`);
        assert.equal(err.code, "E-PARSE-01", `${fixture.id}: code`);
        assert.equal(err.name, "FrontMatterError", `${fixture.id}: name`);
        assert.ok(err.line >= 1 && err.column >= 1, `${fixture.id}: one-based location`);
        return true;
      },
      fixture.id,
    );
  }
});
