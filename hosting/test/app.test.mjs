import { test } from "node:test";
import assert from "node:assert/strict";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { createApp } from "../src/app.mjs";
import {
  CACHE_CONTENT,
  CACHE_SHORT_ID,
  longUrl,
} from "../src/contract.mjs";
import { takedown } from "../src/origin.mjs";

const BASE_URL = "https://docs.example";
const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);

test("end-to-end publish, serve, and takedown", { timeout: 300_000 }, async () => {
  const app = await createApp({
    repoRoot,
    wasmPath: undefined,
    baseUrl: BASE_URL,
  });

  const source =
    "---\ntitle: E2E test\n---\n\n# Hello from the e2e test\n\nSafe markdown only.\n";
  const form = new FormData();
  form.append(
    "source",
    new Blob([source], { type: "text/markdown" }),
    "doc.md",
  );
  const publishResponse = await app.handle(
    new Request("https://mdhtml.example/v1/documents", {
      method: "POST",
      body: form,
    }),
    { ip: "127.0.0.1" },
  );
  const publishBody = await publishResponse.text();
  assert.equal(publishResponse.status, 200, publishBody);
  const published = JSON.parse(publishBody);
  assert.equal(published.mdhtmlVersion, "1.0");
  assert.match(published.id, /^[A-Za-z0-9_-]{12}$/);
  assert.match(published.sha256, /^[0-9a-f]{64}$/);
  assert.equal(published.url, `${BASE_URL}/${published.id}`);

  const shortResponse = await app.handle(
    new Request(`${BASE_URL}/${published.id}`),
  );
  assert.equal(shortResponse.status, 308);
  assert.equal(
    shortResponse.headers.get("Location"),
    longUrl(BASE_URL, app.toolchain.toolchainId, published.sha256),
  );
  assert.equal(shortResponse.headers.get("Cache-Control"), CACHE_SHORT_ID);

  const documentResponse = await app.handle(
    new Request(shortResponse.headers.get("Location")),
  );
  assert.equal(documentResponse.status, 200);
  assert.equal(
    documentResponse.headers.get("Content-Type"),
    "text/html; charset=utf-8",
  );
  assert.equal(
    documentResponse.headers.get("Cache-Control"),
    CACHE_CONTENT,
  );
  assert.match(documentResponse.headers.get("X-Robots-Tag"), /noindex/);
  assert.match(
    documentResponse.headers.get("Content-Security-Policy"),
    /'sha256-[A-Za-z0-9+/=_-]{40,64}'/,
  );
  const documentBody = await documentResponse.text();
  assert.match(documentBody, /id="mdhtml-source"/);

  const robotsResponse = await app.handle(
    new Request(`${BASE_URL}/robots.txt`),
  );
  assert.equal(robotsResponse.status, 200);
  assert.match(await robotsResponse.text(), /Disallow: \//);

  const removed = await takedown({
    publicId: published.id,
    store: app.store,
    denyList: app.denyList,
  });
  assert.equal(removed.removed, true);
  assert.equal(
    (await app.handle(new Request(`${BASE_URL}/${published.id}`))).status,
    404,
  );
  assert.equal(await app.denyList.isDenied(published.sha256), true);
});
