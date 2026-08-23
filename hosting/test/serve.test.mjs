import { test } from "node:test";
import assert from "node:assert/strict";

import {
  CACHE_CONTENT,
  CACHE_SHORT_ID,
  ROBOTS_TXT,
  X_ROBOTS_TAG,
  longUrl,
} from "../src/contract.mjs";
import { createDenyList } from "../src/origin.mjs";
import { createRateLimiter, handlePublish } from "../src/publish.mjs";
import { serve } from "../src/serve.mjs";
import { createStore, memoryBackend } from "../src/storage.mjs";

const BASE_URL = "https://docs.example";
const RUNTIME_HASH = "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB=";
const META_CSP =
  `<meta http-equiv="Content-Security-Policy" content="default-src 'none'; ` +
  `script-src 'sha256-${RUNTIME_HASH}'; style-src 'unsafe-inline'; ` +
  `img-src data: blob:; font-src data:; media-src data: blob:; ` +
  `connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'">`;

const MOCK_TOOLCHAIN = {
  toolchainId: "mock-toolchain",
  async build({ source }) {
    const text =
      typeof source === "string" ? source : new TextDecoder().decode(source);
    return {
      html:
        `<!doctype html><html><head>${META_CSP}</head><body>` +
        `<script id="mdhtml-source" type="text/markdown">${text}</script></body></html>`,
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

async function seed() {
  const backend = memoryBackend();
  const store = createStore({ backend, toolchainId: MOCK_TOOLCHAIN.toolchainId });
  const denyList = createDenyList({ backend });
  const form = new FormData();
  form.append(
    "source",
    new Blob(["# Served\n"], { type: "text/markdown" }),
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
    baseUrl: BASE_URL,
  });
  const body = await response.text();
  assert.equal(response.status, 200, body);
  const published = JSON.parse(body);
  return { store, denyList, ...published };
}

test("long URL serves the document with security headers", async () => {
  const { store, denyList, toolchainId, sha256 } = await seed();
  const response = await serve(
    new Request(`${BASE_URL}/d/${toolchainId}/${sha256}`),
    { store, denyList, baseUrl: BASE_URL },
  );
  assert.equal(response.status, 200);
  assert.equal(
    response.headers.get("Content-Type"),
    "text/html; charset=utf-8",
  );
  assert.equal(response.headers.get("Cache-Control"), CACHE_CONTENT);
  assert.equal(response.headers.get("X-Robots-Tag"), X_ROBOTS_TAG);
  assert.match(
    response.headers.get("Content-Security-Policy"),
    new RegExp(`'sha256-${RUNTIME_HASH}'`),
  );
  const body = await response.text();
  assert.match(body, /id="mdhtml-source"/);
});

test("short ID resolves via cached 308 to the long URL", async () => {
  const { store, denyList, id, sha256 } = await seed();
  const response = await serve(new Request(`${BASE_URL}/${id}`), {
    store,
    denyList,
    baseUrl: BASE_URL,
  });
  assert.equal(response.status, 308);
  assert.equal(
    response.headers.get("Location"),
    longUrl(BASE_URL, MOCK_TOOLCHAIN.toolchainId, sha256),
  );
  assert.equal(response.headers.get("Cache-Control"), CACHE_SHORT_ID);
});

test("robots.txt returns the deny-all robots body without Cache-Control", async () => {
  const { store, denyList } = await seed();
  const response = await serve(
    new Request(`${BASE_URL}/robots.txt`),
    { store, denyList, baseUrl: BASE_URL },
  );
  assert.equal(response.status, 200);
  assert.equal(await response.text(), ROBOTS_TXT);
  assert.equal(response.headers.get("Cache-Control"), null);
});

test("unknown ids, unknown long URLs, and the API path 404", async () => {
  const { store, denyList } = await seed();
  const urls = [
    `${BASE_URL}/doesnotexist`,
    `${BASE_URL}/d/unknown-toolchain/0000000000000000000000000000000000000000000000000000000000000000`,
    `${BASE_URL}/v1/documents`,
  ];
  for (const url of urls) {
    const response = await serve(new Request(url), {
      store,
      denyList,
      baseUrl: BASE_URL,
    });
    assert.equal(response.status, 404, url);
    assert.equal(await response.text(), "not found");
  }
});
