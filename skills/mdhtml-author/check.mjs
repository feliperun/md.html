#!/usr/bin/env node
// T17 (PROD-02): focused validation for the mdhtml-author skill.
// Proves the skill references only committed templates/ and examples/, and
// that no template or example content is duplicated inside the skill files.

import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const SKILL_DIR = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(SKILL_DIR, '..', '..');
const KINDS = ['resume', 'memo', 'spec', 'recipe', 'chapter'];
const GROUPS = ['templates', 'examples'];

const failures = [];
const expect = (ok, message) => {
  if (!ok) failures.push(message);
};

const canonicalFiles = GROUPS.flatMap((group) =>
  KINDS.map((kind) => `${group}/${kind}.md`),
);

const skillFiles = [
  'SKILL.md',
  ...readdirSync(join(SKILL_DIR, 'references'))
    .sort()
    .map((file) => `references/${file}`),
];

// 1. The committed canonical files exist.
for (const rel of canonicalFiles) {
  expect(existsSync(join(ROOT, rel)), `missing committed file ${rel}`);
}

// 2. Every templates/ or examples/ path referenced by the skill is committed.
const pathPattern = /\b(templates|examples)\/[A-Za-z0-9._-]+\.md\b/g;
for (const rel of skillFiles) {
  const text = readFileSync(join(SKILL_DIR, rel), 'utf8');
  for (const match of text.matchAll(pathPattern)) {
    expect(
      canonicalFiles.includes(match[0]) && existsSync(join(ROOT, match[0])),
      `skill file ${rel} references unknown path ${match[0]}`,
    );
  }
}

// 3. The skill names every canonical template and example.
const skillText = skillFiles
  .map((rel) => readFileSync(join(SKILL_DIR, rel), 'utf8'))
  .join('\n');
for (const rel of canonicalFiles) {
  expect(skillText.includes(rel), `skill does not reference committed ${rel}`);
}

// 4. No template or example content line is duplicated in the skill files.
const significant = (line) => {
  const text = line.trim();
  if (text.length === 0) return null;
  if (text === '---') return null;
  if (/^(`{3,}|~{3,})/.test(text)) return null;
  if (/^:::+$/.test(text)) return null;
  if (/^:::[a-z][a-z0-9-]*$/.test(text)) return null;
  if (/^\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?\s*$/.test(text)) return null;
  return text;
};

const contentLines = new Map();
for (const rel of canonicalFiles) {
  for (const line of readFileSync(join(ROOT, rel), 'utf8').split('\n')) {
    const key = significant(line);
    if (key === null) continue;
    if (!contentLines.has(key)) contentLines.set(key, []);
    contentLines.get(key).push(rel);
  }
}

for (const rel of skillFiles) {
  const lines = readFileSync(join(SKILL_DIR, rel), 'utf8')
    .split('\n')
    .map(significant)
    .filter((line) => line !== null);
  for (const line of lines) {
    const sources = contentLines.get(line);
    if (sources !== undefined) {
      expect(
        false,
        `skill file ${rel} duplicates content from ${sources.join(', ')}: ${line}`,
      );
    }
  }
}

if (failures.length > 0) {
  console.error(`mdhtml-author: ${failures.length} problem(s):`);
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}
console.log(
  'mdhtml-author: skill references committed templates and examples, no content duplication',
);
