import { test } from "node:test";
import assert from "node:assert/strict";

import {
  CSP_TEMPLATE,
  FIXED_HEADERS,
  ROBOTS_TXT,
  X_ROBOTS_TAG,
} from "../src/contract.mjs";
import {
  createDenyList,
  extractRuntimeHash,
  robotsTxtResponse,
  securityHeaders,
  takedown,
} from "../src/origin.mjs";
import { createRateLimiter, handlePublish } from "../src/publish.mjs";
import { createStore, memoryBackend } from "../src/storage.mjs";

const RUNTIME_HASH = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
const META_CSP =
  `<meta http-equiv="Content-Security-Policy" content="default-src 'none'; ` +
  `script-src 'sha256-${RUNTIME_HASH}'; style-src 'unsafe-inline'; ` +
  `img-src data: blob:; font-src data:; media-src data: blob:; ` +
  `connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'">`;
const SAMPLE_HTML =
  `<!doctype html>\n<html>\n  <head>\n    <meta charset="utf-8">\n    ${META_CSP}\n` +
  `  </head>\n  <body>\n    <script id="mdhtml-source" type="text/markdown"># Hello</script>\n` +
  `  </body>\n</html>\n`;

const MOCK_TOOLCHAIN = {
  toolchainId: "mock-toolchain",
  async build({ source }) {
    const text =
      typeof source === "string" ? source : new TextDecoder().decode(source);
    return {
      html: SAMPLE_HTML.replace("># Hello<", `>${text}<`),
      exitCode: 0,
      stderr: "",
    };
  },
  async audit() {
    return {
      report: { safe: true, html: "pass", css: "pass", runtime: "pass" },
      exitCode: 0,
      stdout: "",
      stderr: "",
    };
  },
};

async function seedDocument() {
  const backend = memoryBackend();
  const store = createStore({ backend, toolchainId: MOCK_TOOLCHAIN.toolchainId });
  const denyList = createDenyList({ backend });
  const form = new FormData();
  form.append(
    "source",
    new Blob(["# Hello\n"], { type: "text/markdown" }),
    "doc.md",
  );
  const request = new Request("https://mdhtml.example/v1/documents", {
    method: "POST",
    body: form,
  });
  const response = await handlePublish(request, {
    toolchain: MOCK_TOOLCHAIN,
    store,
    denyList,
    rateLimiter: createRateLimiter(),
    ip: "127.0.0.1",
    baseUrl: "https://docs.example",
  });
  const body = await response.text();
  assert.equal(response.status, 200, body);
  const published = JSON.parse(body);
  return { store, denyList, ...published };
}

test("extractRuntimeHash returns the hash from the first meta CSP", () => {
  assert.equal(extractRuntimeHash(SAMPLE_HTML), RUNTIME_HASH);
  assert.equal(extractRuntimeHash("<html><head></head></html>"), null);
  assert.equal(
    extractRuntimeHash(
      `<meta http-equiv="Content-Security-Policy" content="default-src 'none'">`,
    ),
    null,
  );
});

test("securityHeaders substitutes the hash and applies all fixed headers", () => {
  const headers = securityHeaders(SAMPLE_HTML);
  assert.equal(
    headers["Content-Security-Policy"],
    CSP_TEMPLATE.replace("{RUNTIME_HASH}", RUNTIME_HASH),
  );
  assert.equal(headers["X-Robots-Tag"], X_ROBOTS_TAG);
  for (const [name, value] of Object.entries(FIXED_HEADERS)) {
    assert.equal(headers[name], value, name);
  }
});

test("securityHeaders fails closed without a runtime hash", () => {
  assert.throws(
    () => securityHeaders("<html><body>no csp here</body></html>"),
    /missing runtime hash/,
  );
});

test("robotsTxtResponse serves the robots.txt body", async () => {
  const response = robotsTxtResponse();
  assert.equal(response.status, 200);
  assert.equal(await response.text(), ROBOTS_TXT);
});

test("denyList add/isDenied/list", async () => {
  const denyList = createDenyList({ backend: memoryBackend() });
  const sha256 = "a".repeat(64);
  assert.equal(await denyList.isDenied(sha256), false);
  await denyList.add(sha256);
  assert.equal(await denyList.isDenied(sha256), true);
  const keys = await denyList.list();
  assert.ok(keys.some((k) => k === sha256 || k.endsWith(`/${sha256}`)));
});

test("takedown removes the document and denies the source hash", async () => {
  const { store, denyList, id, sha256 } = await seedDocument();
  const purged = [];
  const logged = [];
  const result = await takedown({
    publicId: id,
    store,
    denyList,
    purge: async (hash) => purged.push(hash),
    log: async (entry) => logged.push(entry),
  });
  assert.deepEqual(result, { removed: true });
  assert.equal(await store.resolveId(id), null);
  assert.equal(await store.readDocument(sha256), null);
  assert.equal(await store.readSource(sha256), null);
  assert.equal(await denyList.isDenied(sha256), true);
  assert.deepEqual(purged, [sha256]);
  assert.equal(logged.length, 1);
  assert.equal(logged[0].publicId, id);
  assert.equal(logged[0].sha256, sha256);
  assert.equal(logged[0].toolchainId, MOCK_TOOLCHAIN.toolchainId);
  assert.equal(typeof logged[0].at, "number");
});

test("takedown reports removed:false for an unknown id", async () => {
  const backend = memoryBackend();
  const store = createStore({ backend, toolchainId: MOCK_TOOLCHAIN.toolchainId });
  const denyList = createDenyList({ backend });
  const result = await takedown({
    publicId: "doesnotexist",
    store,
    denyList,
  });
  assert.deepEqual(result, { removed: false });
});
