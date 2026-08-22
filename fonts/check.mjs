import { createHash } from "node:crypto";
import { readFile, readdir } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../", import.meta.url));
const FONTS = join(ROOT, "fonts");
const FIXTURE_FILE = join(ROOT, "fixtures", "font-selection.json");
const CATALOG_FILE = join(FONTS, "catalog.json");

const INSTRUMENT_SOURCE = "https://registry.npmjs.org/@fontsource-variable/instrument-sans/-/instrument-sans-5.3.0.tgz";
const NEWSREADER_SOURCE = "https://registry.npmjs.org/@fontsource-variable/newsreader/-/newsreader-5.3.0.tgz";
const GEIST_SOURCE = "https://raw.githubusercontent.com/vercel/geist-font/v1.7.1/fonts/GeistMono/webfonts/GeistMono%5Bwght%5D.woff2";
const GEIST_COMMIT = "8b8b75fa63e339db10a3cd52fb28536615b5cc63";

const EXPECTED_FILES = [
  "InstrumentSans-latin-wght-normal.woff2",
  "InstrumentSans-latin-wght-italic.woff2",
  "Newsreader-latin-wght-normal.woff2",
  "Newsreader-latin-wght-italic.woff2",
  "GeistMono-wght-normal.woff2",
  "InstrumentSans-OFL.txt",
  "Newsreader-OFL.txt",
  "GeistMono-OFL.txt",
  "NOTICE.md",
  "README.md",
  "catalog.json",
  "check.mjs",
].sort();

const LICENSE_HASHES = {
  "InstrumentSans-OFL.txt": "c27a3c53c3beed7f5c26853afa15991478ff7145d3754a36b0382f84e10c0d03",
  "Newsreader-OFL.txt": "26028ec4e13b650065fa525a09532176f8a668b76ff849ea01c564a7480f91e7",
  "GeistMono-OFL.txt": "c683bfbcc7e087f5d37a54ef628f10387c451a83ddc459b151403a164ac46c90",
};

const FONT_SPEC = {
  "instrument-sans": {
    name: "Instrument Sans",
    role: "body",
    licenseFile: "InstrumentSans-OFL.txt",
    source: INSTRUMENT_SOURCE,
    version: "5.3.0",
    package: "@fontsource-variable/instrument-sans",
    integrity: "sha512-u4gKbDBTNFGkg997tfQn3eHOhHuquWUFTRT/rwzuKtrxX5P2ekfs2x+LgBPP4P32+cC+vUwF1Cr+IdRoPQbrGw==",
    faces: {
      normal: {
        style: "normal",
        min: 400,
        max: 700,
        file: "InstrumentSans-latin-wght-normal.woff2",
        bytes: 30092,
        sha256: "2ee17598a98d8a59e4df8152d015bec9ab8e4d5672cc0ab42bef806b568e3971",
      },
      italic: {
        style: "italic",
        min: 400,
        max: 700,
        file: "InstrumentSans-latin-wght-italic.woff2",
        bytes: 31828,
        sha256: "77210cdde0281b5ecb0d592e063a98656f1bc36993a3b98f506eb91ff4a433a5",
      },
    },
  },
  newsreader: {
    name: "Newsreader",
    role: "body",
    licenseFile: "Newsreader-OFL.txt",
    source: NEWSREADER_SOURCE,
    version: "5.3.0",
    package: "@fontsource-variable/newsreader",
    integrity: "sha512-rrzYi43qMpbzwuFtf9OkWH8sxAPVPcQQQEwXpPtwaKYeJ8yVg5aLs5kawmo1f2Q1t1M38TLmEKCkGVDsYwgdFw==",
    faces: {
      normal: {
        style: "normal",
        min: 200,
        max: 800,
        file: "Newsreader-latin-wght-normal.woff2",
        bytes: 58084,
        sha256: "62981321d9a3cc7a61a73792729043703fd6112da86e8ec848bb57f088578757",
      },
      italic: {
        style: "italic",
        min: 200,
        max: 800,
        file: "Newsreader-latin-wght-italic.woff2",
        bytes: 64520,
        sha256: "48bc8861b9b2ca9300747cad4fd6a3b4ac3028d364df00bd1b72097baa75e509",
      },
    },
  },
  "geist-mono": {
    name: "Geist Mono",
    role: "mono",
    licenseFile: "GeistMono-OFL.txt",
    source: GEIST_SOURCE,
    version: "v1.7.1",
    commit: GEIST_COMMIT,
    integrity: `commit:${GEIST_COMMIT}`,
    faces: {
      normal: {
        style: "normal",
        min: 100,
        max: 900,
        file: "GeistMono-wght-normal.woff2",
        bytes: 71596,
        sha256: "afaacc4c5fbba89d2ebf7a02dc4070208540874592a5504d57175782fe893101",
      },
    },
  },
};

const PRESETS = {
  technical: { body: "instrument-sans", mono: "geist-mono" },
  editorial: { body: "newsreader", mono: "geist-mono" },
  system: { body: null, mono: null },
};

const COPYRIGHT = [
  "Copyright 2022 The Instrument Sans Project Authors (https://github.com/Instrument/instrument-sans)",
  "Copyright 2020 The Newsreader Project Authors (http://github.com/productiontype/Newsreader)",
  "Copyright 2024 The Geist Project Authors (https://github.com/vercel/geist-font)",
];

function same(actual, expected) {
  return JSON.stringify(actual) === JSON.stringify(expected);
}

function keys(value) {
  return Object.keys(value ?? {}).sort();
}

function add(problems, condition, message) {
  if (!condition) problems.push(message);
}

function faceFromCatalog(familyName, faceName, catalog) {
  return catalog.families[familyName]?.faces?.[faceName];
}

export function selectFaces({ preset, emphasis = false, code = false } = {}, catalogValue = catalog) {
  const selectedPreset = catalogValue.presets[preset];
  if (!selectedPreset) throw new Error(`unknown font preset: ${preset}`);
  if (selectedPreset.body === null) return [];
  const faces = [
    { family: selectedPreset.body, name: "normal" },
    ...(emphasis ? [{ family: selectedPreset.body, name: "italic" }] : []),
    ...(code ? [{ family: selectedPreset.mono, name: "normal" }] : []),
  ];
  return faces.map(({ family, name }) => ({
    family,
    name,
    ...catalogValue.families[family].faces[name],
  }));
}

export function checkCatalog(catalogValue = catalog) {
  const problems = [];
  add(problems, catalogValue && same(keys(catalogValue), ["families", "format", "presets"]), "catalog top-level keys are not closed");
  add(problems, catalogValue?.format === "mdhtml/fonts/1.0", "catalog format is invalid");
  add(problems, same(catalogValue?.presets, PRESETS), "catalog presets are invalid");
  add(problems, same(keys(catalogValue?.families), Object.keys(FONT_SPEC).sort()), "catalog families are invalid");

  for (const [familyName, expected] of Object.entries(FONT_SPEC)) {
    const family = catalogValue?.families?.[familyName];
    add(problems, family !== undefined, `${familyName}: family is missing`);
    if (!family) continue;
    add(problems, same(keys(family), ["faces", "license", "licenseFile", "name", "notice", "provenance", "role"]), `${familyName}: family keys are not closed`);
    add(problems, family.name === expected.name, `${familyName}: name is invalid`);
    add(problems, family.role === expected.role, `${familyName}: role is invalid`);
    add(problems, family.license === "OFL-1.1", `${familyName}: license is invalid`);
    add(problems, family.licenseFile === expected.licenseFile, `${familyName}: license file is invalid`);
    add(problems, family.notice === "NOTICE.md", `${familyName}: notice is invalid`);
    const provenance = family.provenance;
    const provenanceKeys = familyName === "geist-mono" ? ["commit", "integrity", "source", "version"] : ["integrity", "package", "source", "version"];
    add(problems, same(keys(provenance), provenanceKeys), `${familyName}: provenance keys are not closed`);
    add(problems, provenance?.source === expected.source, `${familyName}: source is invalid`);
    add(problems, provenance?.version === expected.version, `${familyName}: version is invalid`);
    add(problems, provenance?.integrity === expected.integrity, `${familyName}: integrity is invalid`);
    if (familyName === "geist-mono") add(problems, provenance?.commit === expected.commit, `${familyName}: commit is invalid`);
    else add(problems, provenance?.package === expected.package, `${familyName}: package is invalid`);
    add(problems, same(keys(family.faces), Object.keys(expected.faces).sort()), `${familyName}: faces are not closed`);

    for (const [faceName, expectedFace] of Object.entries(expected.faces)) {
      const face = family.faces?.[faceName];
      add(problems, face !== undefined, `${familyName}/${faceName}: face is missing`);
      if (!face) continue;
      add(problems, same(keys(face), ["axes", "bytes", "file", "license", "licenseFile", "notice", "provenance", "sha256", "style"]), `${familyName}/${faceName}: face keys are not closed`);
      add(problems, face.style === expectedFace.style, `${familyName}/${faceName}: style is invalid`);
      add(problems, same(face.axes, { wght: { min: expectedFace.min, max: expectedFace.max } }), `${familyName}/${faceName}: axes are invalid`);
      add(problems, face.file === expectedFace.file, `${familyName}/${faceName}: file is invalid`);
      add(problems, face.bytes === expectedFace.bytes, `${familyName}/${faceName}: byte count is invalid`);
      add(problems, face.sha256 === expectedFace.sha256, `${familyName}/${faceName}: hash is invalid`);
      add(problems, face.license === "OFL-1.1", `${familyName}/${faceName}: license is invalid`);
      add(problems, face.licenseFile === expected.licenseFile, `${familyName}/${faceName}: license file is invalid`);
      add(problems, face.notice === "NOTICE.md", `${familyName}/${faceName}: notice is invalid`);
      add(problems, same(face.provenance, provenance), `${familyName}/${faceName}: provenance is invalid`);
    }
  }
  return problems;
}

async function checkFiles(catalogValue, catalogText) {
  const problems = [];
  const actualFiles = (await readdir(FONTS)).sort();
  add(problems, same(actualFiles, EXPECTED_FILES), "fonts file set is not closed");
  add(problems, !/opsz/iu.test(catalogText), "catalog declares opsz");
  add(problems, !actualFiles.some((file) => /opsz/iu.test(file)), "font filename declares opsz");

  const notice = await readFile(join(FONTS, "NOTICE.md"), "utf8");
  for (const line of COPYRIGHT) add(problems, notice.includes(line), `notice is missing: ${line}`);
  add(problems, notice.replace(/\s+/gu, " ").includes("These are upstream latin distributions, not locally subsetted or converted font files."), "notice does not state upstream latin provenance");

  for (const [familyName, family] of Object.entries(catalogValue.families)) {
    const licenseBytes = await readFile(join(FONTS, family.licenseFile));
    const licenseHash = createHash("sha256").update(licenseBytes).digest("hex");
    add(problems, licenseHash === LICENSE_HASHES[family.licenseFile], `${familyName}: license hash mismatch`);
    for (const [faceName, face] of Object.entries(family.faces)) {
      const bytes = await readFile(join(FONTS, face.file));
      add(problems, bytes.length === face.bytes, `${familyName}/${faceName}: size mismatch`);
      add(problems, createHash("sha256").update(bytes).digest("hex") === face.sha256, `${familyName}/${faceName}: SHA-256 mismatch`);
      add(problems, bytes.subarray(0, 4).toString("ascii") === "wOF2", `${familyName}/${faceName}: missing WOFF2 magic`);
    }
  }
  return problems;
}

async function checkFixtures(catalogValue) {
  const problems = [];
  const fixtureText = await readFile(FIXTURE_FILE, "utf8");
  const fixture = JSON.parse(fixtureText);
  add(problems, same(keys(fixture), ["cases", "format", "id", "source"]), "selection fixture keys are not closed");
  add(problems, fixture.id === "font-selection", "selection fixture id is invalid");
  add(problems, fixture.source === "", "selection fixture source is invalid");
  add(problems, fixture.format === "mdhtml/fonts/selection/1.0", "selection fixture format is invalid");
  const expectedNames = ["technical-none", "technical-emphasis", "technical-code", "technical-both", "editorial-none", "editorial-emphasis", "editorial-code", "editorial-both", "system-none", "system-emphasis", "system-code", "system-both"];
  add(problems, fixture.cases?.length === expectedNames.length, "selection fixture case count is invalid");
  for (let index = 0; index < expectedNames.length; index += 1) {
    const entry = fixture.cases?.[index];
    add(problems, entry?.name === expectedNames[index], `selection fixture order is invalid at ${index}`);
    if (!entry) continue;
    add(problems, same(keys(entry), ["bytes", "code", "emphasis", "files", "name", "preset"]), `${entry.name}: fixture keys are not closed`);
    const selected = selectFaces(entry, catalogValue).map((face) => face.file);
    add(problems, same(selected, entry.files), `${entry.name}: selection order/files are invalid`);
    const total = selectFaces(entry, catalogValue).reduce((sum, face) => sum + face.bytes, 0);
    add(problems, total === entry.bytes, `${entry.name}: byte total is invalid`);
  }
  return problems;
}

async function main() {
  let catalog;
  try {
    const catalogText = await readFile(CATALOG_FILE, "utf8");
    catalog = JSON.parse(catalogText);
    const problems = [
      ...checkCatalog(catalog),
      ...(await checkFiles(catalog, catalogText)),
      ...(await checkFixtures(catalog)),
    ];
    if (problems.length > 0) {
      console.error(`font check failed:\n${problems.join("\n")}`);
      process.exitCode = 1;
      return;
    }
    console.log("check: fonts catalog, licenses, binaries, and selection fixtures are valid");
  } catch (error) {
    console.error(`font check failed: ${error.message}`);
    process.exitCode = 1;
  }
}

const catalog = JSON.parse(await readFile(CATALOG_FILE, "utf8"));
if (process.argv[1] === fileURLToPath(import.meta.url)) await main();
