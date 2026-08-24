import test from "node:test";
import assert from "node:assert/strict";

import {
  createRateLimiter,
  handlePublish,
  isSafeAssetPath,
} from "../src/publish.mjs";
import { SOURCE_LIMIT_BYTES } from "../src/contract.mjs";

const HASH = "a".repeat(64);
const NOW = 1_700_000_000_000;
const PUBLIC_ID = "AB12cd34EF-_";
const HAPPY_HTML =
  '<html data-mdhtml="1.0"><script id="mdhtml-source" type="text/markdown">x</script></html>';

function makeToolchain(overrides = {}) {
  return {
    build:
      overrides.build ??
      (async () => ({ html: HAPPY_HTML, exitCode: 0, stderr: "" })),
    audit:
      overrides.audit ??
      (async () => ({
        report: { safe: true, html: "pass", css: "pass", runtime: "pass" },
        exitCode: 0,
      })),
  };
}

function makeStore() {
  const calls = [];
  return {
    calls,
    sha256: () => HASH,
    async putSource(bytes) {
      calls.push(["putSource", bytes]);
      return { sha256: HASH, created: true };
    },
    async putDocument(sha256, html) {
      calls.push(["putDocument", sha256, html]);
      return { key: `docs/toolchain/${sha256}`, created: true };
    },
    createId(sha256, options) {
      calls.push(["createId", sha256, options]);
      return { publicId: PUBLIC_ID };
    },
  };
}

function makeDenyList(denied = false) {
  return { isDenied: async () => denied };
}

function makeDeps(overrides = {}) {
  return {
    toolchain: overrides.toolchain ?? makeToolchain(),
    store: overrides.store ?? makeStore(),
    denyList: overrides.denyList ?? makeDenyList(),
    rateLimiter: overrides.rateLimiter ?? createRateLimiter({ now: () => NOW }),
    ip: overrides.ip ?? "203.0.113.7",
    baseUrl: overrides.baseUrl ?? "https://docs.example",
  };
}

function publishRequest({ source, assets } = {}) {
  const form = new FormData();
  if (source !== undefined) form.append("source", source);
  for (const asset of assets ?? []) form.append("asset", asset);
  return new Request("https://docs.example/v1/documents", {
    method: "POST",
    body: form,
  });
}

test("valid publish returns 200 with id, url, sha256, mdhtmlVersion and a 12-char id", async () => {
  const store = makeStore();
  const response = await handlePublish(
    publishRequest({ source: new File(["# Hello"], "doc.md") }),
    makeDeps({ store }),
  );
  assert.equal(response.status, 200);
  const body = await response.json();
  assert.deepEqual(body, {
    id: PUBLIC_ID,
    url: `https://docs.example/${PUBLIC_ID}`,
    sha256: HASH,
    mdhtmlVersion: "1.0",
  });
  assert.equal(body.id.length, 12);
  assert.match(body.id, /^[A-Za-z0-9_-]{12}$/);
  assert.deepEqual(
    store.calls.map(([name]) => name),
    ["putSource", "putDocument", "createId"],
  );
});

test("missing source returns 400 E-API-004", async () => {
  const response = await handlePublish(publishRequest(), makeDeps());
  assert.equal(response.status, 400);
  const { error } = await response.json();
  assert.equal(error.code, "E-API-004");
  assert.equal(error.message, "missing source field");
});

test("non-file source returns 400 E-API-004", async () => {
  const form = new FormData();
  form.append("source", "plain text is not a file");
  const request = new Request("https://docs.example/v1/documents", {
    method: "POST",
    body: form,
  });
  const response = await handlePublish(request, makeDeps());
  assert.equal(response.status, 400);
  const { error } = await response.json();
  assert.equal(error.code, "E-API-004");
});

test("source too large returns 413 E-API-001", async () => {
  const big = new Uint8Array(SOURCE_LIMIT_BYTES + 1);
  const response = await handlePublish(
    publishRequest({ source: new File([big], "big.md") }),
    makeDeps(),
  );
  assert.equal(response.status, 413);
  const { error } = await response.json();
  assert.equal(error.code, "E-API-001");
  assert.equal(error.message, "source exceeds 2 MiB");
});

test("unsafe asset paths return 400 E-MDHSEC-014", async () => {
  for (const name of ["../x", "a\\b", "/abs", "C:x"]) {
    const response = await handlePublish(
      publishRequest({
        source: new File(["# Hello"], "doc.md"),
        assets: [new File(["x"], name)],
      }),
      makeDeps(),
    );
    assert.equal(response.status, 400, `expected 400 for ${JSON.stringify(name)}`);
    const { error } = await response.json();
    assert.equal(error.code, "E-MDHSEC-014");
    assert.equal(error.message, `unsafe asset path: ${name}`);
  }
});

test("rate limit: 11th publish in one minute returns 429 E-API-002 with Retry-After", async () => {
  const rateLimiter = createRateLimiter({ now: () => NOW });
  const deps = makeDeps({ rateLimiter, ip: "203.0.113.9" });
  for (let i = 0; i < 10; i++) {
    const response = await handlePublish(
      publishRequest({ source: new File(["# Hello"], "doc.md") }),
      deps,
    );
    assert.equal(response.status, 200, `publish ${i + 1} should be allowed`);
  }
  const denied = await handlePublish(
    publishRequest({ source: new File(["# Hello"], "doc.md") }),
    deps,
  );
  assert.equal(denied.status, 429);
  const { error } = await denied.json();
  assert.equal(error.code, "E-API-002");
  assert.equal(error.message, "rate limited");
  const expected = Math.max(
    1,
    Math.ceil((Math.floor(NOW / 60_000) * 60_000 + 60_000 - NOW) / 1000),
  );
  assert.equal(denied.headers.get("Retry-After"), String(expected));
});

test("deny-list hit returns 403 E-API-007", async () => {
  const response = await handlePublish(
    publishRequest({ source: new File(["# Bad"], "doc.md") }),
    makeDeps({ denyList: makeDenyList(true) }),
  );
  assert.equal(response.status, 403);
  const { error } = await response.json();
  assert.equal(error.code, "E-API-007");
  assert.equal(error.message, "document is not available");
});

test("build rejection returns 422 with the extracted E-MDHSEC code", async () => {
  const toolchain = makeToolchain({
    build: async () => ({
      html: "",
      exitCode: 1,
      stderr: "mdhtml: E-MDHSEC-012: Unsafe URI scheme",
    }),
  });
  const response = await handlePublish(
    publishRequest({ source: new File(["# Bad"], "doc.md") }),
    makeDeps({ toolchain }),
  );
  assert.equal(response.status, 422);
  const { error } = await response.json();
  assert.equal(error.code, "E-MDHSEC-012");
  assert.equal(error.message, "mdhtml: E-MDHSEC-012: Unsafe URI scheme");
});

test("audit failure returns 422 E-API-006", async () => {
  const toolchain = makeToolchain({
    audit: async () => ({
      report: { safe: false, html: "fail", css: "fail", runtime: "fail" },
      exitCode: 0,
    }),
  });
  const response = await handlePublish(
    publishRequest({ source: new File(["# Bad"], "doc.md") }),
    makeDeps({ toolchain }),
  );
  assert.equal(response.status, 422);
  const { error } = await response.json();
  assert.equal(error.code, "E-API-006");
  assert.equal(
    error.message,
    "artifact audit failed (html=fail, css=fail, runtime=fail)",
  );
});

test("unsafe artifact audit returns 422 E-MDHSEC-018", async () => {
  const toolchain = makeToolchain({
    audit: async () => ({
      report: { safe: false, html: "unsafe", css: "pass", runtime: "pass" },
      exitCode: 0,
    }),
  });
  const response = await handlePublish(
    publishRequest({ source: new File(["# Bad"], "doc.md") }),
    makeDeps({ toolchain }),
  );
  assert.equal(response.status, 422);
  const { error } = await response.json();
  assert.equal(error.code, "E-MDHSEC-018");
});

test("malformed multipart returns 400 E-API-003", async () => {
  const request = new Request("https://docs.example/v1/documents", {
    method: "POST",
    headers: { "Content-Type": "multipart/form-data; boundary=xyz" },
    body: "this is not a valid multipart body",
  });
  const response = await handlePublish(request, makeDeps());
  assert.equal(response.status, 400);
  const { error } = await response.json();
  assert.equal(error.code, "E-API-003");
});

test("concurrency: acquire blocks the 3rd concurrent until release", () => {
  const rateLimiter = createRateLimiter({ now: () => NOW });
  const ip = "203.0.113.10";
  assert.equal(rateLimiter.acquire(ip).allowed, true);
  assert.equal(rateLimiter.acquire(ip).allowed, true);
  const third = rateLimiter.acquire(ip);
  assert.equal(third.allowed, false);
  rateLimiter.release(ip);
  assert.equal(rateLimiter.acquire(ip).allowed, true);
});

test("release never drives concurrency below zero", () => {
  const rateLimiter = createRateLimiter({ now: () => NOW });
  const ip = "203.0.113.11";
  rateLimiter.release(ip);
  rateLimiter.release(ip);
  assert.equal(rateLimiter.acquire(ip).allowed, true);
});

test("isSafeAssetPath accepts safe relative paths and rejects unsafe ones", () => {
  for (const ok of ["images/photo.png", "a.txt", ".hidden", "dir/sub/file.txt"]) {
    assert.equal(isSafeAssetPath(ok), true, ok);
  }
  for (const bad of [
    "../x",
    "a\\b",
    "/abs",
    "C:x",
    "http://example.com/x",
    "a\0b",
    "",
    "a/../b",
  ]) {
    assert.equal(isSafeAssetPath(bad), false, JSON.stringify(bad));
  }
});
