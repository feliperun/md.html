import { test } from "node:test";
import assert from "node:assert/strict";
import { decodeCanonicalSource, projectMarkdown } from "../src/canonical.js";
import { loadFixtures } from "./fixtures.mjs";

const encoded = ["<", "\\", "/", "s", "c", "r", "i", "p", "t"].join("");
const decoded = ["<", "/", "s", "c", "r", "i", "p", "t"].join("");

test("projection fixture preserves full bytes and projects body and smart modes exactly", async () => {
  const fixture = (await loadFixtures()).find((entry) => entry.id === "copy-modes");
  assert.ok(fixture);
  assert.equal(projectMarkdown(fixture.source, "full"), fixture.expect.full);
  assert.equal(projectMarkdown(fixture.source, "body"), fixture.expect.body);
  assert.equal(projectMarkdown(fixture.source), fixture.expect.smart);
});

test("decoder remains case-insensitive and separate from full projection", () => {
  const source = `${encoded} ${encoded.toUpperCase()}`;
  assert.equal(decodeCanonicalSource(source), `${decoded} ${decoded}`);
  assert.equal(projectMarkdown(source, "full"), source);
});

test("smart projection decodes source without front matter and omits empty front matter", () => {
  assert.equal(projectMarkdown(`Body ${encoded}`, "smart"), `Body ${decoded}`);
  assert.equal(projectMarkdown(`---\ntheme: technical\n---\nBody ${encoded}`, "smart"), `Body ${decoded}`);
});
