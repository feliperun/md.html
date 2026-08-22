import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { FIXTURES_DIR, loadFixtures, validateFixture } from "./fixtures.mjs";

const SEED_IDS = [
  "frontmatter-valid-basic",
  "frontmatter-invalid-alias",
  "inline-code-precedence",
  "slug-collision-override",
  "container-warning",
  "forbidden-script-terminator",
];

test("discovers every committed fixture with a stable id", async () => {
  const fixtures = await loadFixtures();
  assert.ok(fixtures.length >= SEED_IDS.length, "seed fixtures must be discovered");
  const seen = new Set();
  for (const fixture of fixtures) {
    assert.equal(
      fixture.id,
      basename(fixture.file, ".json"),
      `${fixture.file}: id must match the file stem`,
    );
    assert.ok(!seen.has(fixture.id), `duplicate fixture id ${fixture.id}`);
    seen.add(fixture.id);
  }
});

test("loadFixtures yields deterministic, stable fixture paths", async () => {
  const fixtures = await loadFixtures();
  const ids = fixtures.map((fixture) => fixture.id);
  assert.deepEqual(ids, [...ids].sort(), "fixtures must be returned in deterministic sorted order");
  for (const fixture of fixtures) {
    assert.equal(
      fixture.file,
      join(FIXTURES_DIR, `${fixture.id}.json`),
      `${fixture.id}: path must be derived deterministically from id and directory`,
    );
  }
});

test("loaded source preserves committed fixture newlines byte for byte", async () => {
  const fixtures = await loadFixtures();
  for (const fixture of fixtures) {
    const raw = await readFile(fixture.file, "utf8");
    const stored = JSON.parse(raw).source;
    assert.equal(
      fixture.source,
      stored,
      `${fixture.id}: loader must return the stored source verbatim`,
    );
    assert.equal(
      (fixture.source.match(/\n/g) || []).length,
      (stored.match(/\n/g) || []).length,
      `${fixture.id}: newline count must survive loading`,
    );
  }
});

test("loader preserves CRLF and trailing newlines from a synthetic fixture", async (t) => {
  const dir = await mkdtemp(join(tmpdir(), "mdhtml-newline-"));
  t.after(() => rm(dir, { recursive: true, force: true }));
  const source = "first line\nsecond line\r\nthird line\n";
  const fixture = { id: "newline-preservation", requirement: "PARSE-02", status: "valid", source };
  await writeFile(join(dir, `${fixture.id}.json`), `${JSON.stringify(fixture, null, 2)}\n`);
  const [loaded] = await loadFixtures(dir);
  assert.equal(loaded.source, source);
  assert.equal(Buffer.byteLength(loaded.source, "utf8"), Buffer.byteLength(source, "utf8"));
});

test("loadFixtures rejects a synthetic set with a duplicate id", async (t) => {
  const dir = await mkdtemp(join(tmpdir(), "mdhtml-duplicate-"));
  t.after(() => rm(dir, { recursive: true, force: true }));
  const base = { requirement: "PARSE-01", status: "valid" };
  await writeFile(join(dir, "dup-a.json"), `${JSON.stringify({ ...base, id: "shared-id", source: "first\n" })}\n`);
  await writeFile(join(dir, "dup-b.json"), `${JSON.stringify({ ...base, id: "shared-id", source: "second\n" })}\n`);
  await assert.rejects(loadFixtures(dir), /duplicate fixture id shared-id/);
});

test("seed fixtures are present and structurally valid", async () => {
  const fixtures = await loadFixtures();
  const byId = new Map(fixtures.map((fixture) => [fixture.id, fixture]));
  for (const id of SEED_IDS) {
    const fixture = byId.get(id);
    assert.ok(fixture, `missing seed fixture ${id}`);
    assert.deepEqual(validateFixture(fixture), [], `${id} must validate`);
  }
});

test("seed fixtures declare the expected status contract", async () => {
  const fixtures = await loadFixtures();
  const byId = new Map(fixtures.map((fixture) => [fixture.id, fixture]));
  const expectations = {
    "frontmatter-valid-basic": { requirement: "PARSE-01", status: "valid" },
    "frontmatter-invalid-alias": { requirement: "PARSE-01", status: "invalid", diagnostic: "E-PARSE-01" },
    "inline-code-precedence": { requirement: "PARSE-03", status: "valid" },
    "slug-collision-override": { requirement: "SECT-01", status: "valid" },
    "container-warning": { requirement: "COMP-02", status: "valid", warnings: ["W-COMP-02"] },
    "forbidden-script-terminator": { requirement: "FMT-02", status: "invalid", diagnostic: "E-FMT-02" },
  };
  for (const [id, expected] of Object.entries(expectations)) {
    const fixture = byId.get(id);
    assert.equal(fixture.requirement, expected.requirement, `${id} requirement`);
    assert.equal(fixture.status, expected.status, `${id} status`);
    if (expected.diagnostic !== undefined) {
      assert.equal(fixture.diagnostic, expected.diagnostic, `${id} diagnostic`);
    }
    if (expected.warnings !== undefined) {
      assert.deepEqual(fixture.warnings, expected.warnings, `${id} warnings`);
    }
  }
});

test("validateFixture rejects malformed records", () => {
  const valid = { id: "sample-valid", requirement: "PARSE-01", status: "valid", source: "# Title\n" };
  assert.deepEqual(validateFixture(valid), []);
  const cases = [
    { ...valid, id: "Bad Id" },
    { ...valid, requirement: "parse-01" },
    { ...valid, status: "maybe" },
    { ...valid, source: "" },
    { ...valid, source: 42 },
    { ...valid, status: "invalid" },
    { ...valid, status: "invalid", diagnostic: "nope" },
    { ...valid, diagnostic: "E-PARSE-01" },
    { ...valid, warnings: ["W-COMP-02", 7] },
  ];
  for (const fixture of cases) {
    assert.ok(validateFixture(fixture).length > 0, "malformed fixture must fail validation");
  }
});
