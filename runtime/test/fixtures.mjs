import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const FIXTURES_DIR = fileURLToPath(new URL("../../fixtures/", import.meta.url));
const ID_RE = /^[a-z0-9][a-z0-9-]*$/;
const REQUIREMENT_RE = /^[A-Z]+-[0-9]{2}$/;
const DIAGNOSTIC_RE = /^[EWI]-[A-Z0-9-]+$/;

export { FIXTURES_DIR };

export async function loadFixtures(dir = FIXTURES_DIR) {
  const files = (await readdir(dir)).filter((name) => name.endsWith(".json")).sort();
  const fixtures = [];
  const byId = new Map();
  for (const file of files) {
    const text = await readFile(join(dir, file), "utf8");
    const fixture = JSON.parse(text);
    if (fixture.id === undefined) continue;
    const previous = byId.get(fixture.id);
    if (previous !== undefined) {
      throw new Error(`duplicate fixture id ${fixture.id}: ${previous} and ${file}`);
    }
    byId.set(fixture.id, file);
    fixtures.push({ ...fixture, file: join(dir, file) });
  }
  return fixtures;
}

export function validateFixture(fixture) {
  const errors = [];
  const { id, requirement, status, source, diagnostic, warnings, expect } = fixture;
  if (typeof id !== "string" || !ID_RE.test(id)) {
    errors.push("id must be a string matching [a-z0-9][a-z0-9-]*");
  }
  if (typeof requirement !== "string" || !REQUIREMENT_RE.test(requirement)) {
    errors.push("requirement must be a string matching [A-Z]+-[0-9]{2}");
  }
  if (status !== "valid" && status !== "invalid") {
    errors.push("status must be 'valid' or 'invalid'");
  }
  if (typeof source !== "string" || source.length === 0) {
    errors.push("source must be a non-empty string");
  }
  if (status === "invalid") {
    if (typeof diagnostic !== "string" || !DIAGNOSTIC_RE.test(diagnostic)) {
      errors.push("invalid fixtures require a diagnostic code matching [EWI]-[A-Z0-9-]+");
    }
  } else {
    if (diagnostic !== undefined) {
      errors.push("valid fixtures must not carry a diagnostic");
    }
    if (
      warnings !== undefined &&
      (!Array.isArray(warnings) || warnings.some((entry) => typeof entry !== "string"))
    ) {
      errors.push("warnings must be an array of strings when present");
    }
  }
  if (expect !== undefined && (expect === null || typeof expect !== "object" || Array.isArray(expect))) {
    errors.push("expect must be a plain object when present");
  }
  return errors;
}
