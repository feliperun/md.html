import { test } from "node:test";
import assert from "node:assert/strict";
import {
  memoryBackend,
  createStore,
  newPublicId,
} from "../src/storage.mjs";
import {
  ID_ALPHABET,
  ID_LENGTH,
  KEY_DOCS,
  KEY_IDS,
  KEY_SOURCES,
} from "../src/contract.mjs";

function bytesFor(id) {
  return Uint8Array.from([...id], (ch) => ID_ALPHABET.indexOf(ch));
}

function idFromBytes(bytes) {
  return Array.from(bytes, (b) => ID_ALPHABET[b & 0x3f]).join("");
}

test("sha256 is deterministic and matches a known digest", () => {
  const store = createStore({ toolchainId: "t1" });
  const digest = store.sha256(new TextEncoder().encode("hello"));
  assert.equal(digest, store.sha256(Buffer.from("hello")));
  assert.equal(
    digest,
    "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
  );
});

test("putSource deduplicates by sha256", () => {
  const store = createStore({ toolchainId: "t1" });
  const bytes = new TextEncoder().encode("# hello\n");
  const first = store.putSource(bytes);
  const second = store.putSource(bytes);
  assert.equal(first.sha256, second.sha256);
  assert.equal(first.created, true);
  assert.equal(second.created, false);
});

test("putDocument deduplicates per (toolchainId, sha256)", () => {
  const storeA = createStore({ toolchainId: "t1" });
  const storeB = createStore({ toolchainId: "t2" });
  const sha = storeA.sha256(new TextEncoder().encode("src"));
  const html = "<p>hi</p>";
  const a1 = storeA.putDocument(sha, html);
  const a2 = storeA.putDocument(sha, html);
  const b1 = storeB.putDocument(sha, html);
  assert.equal(a1.key, `${KEY_DOCS}/t1/${sha}`);
  assert.equal(b1.key, `${KEY_DOCS}/t2/${sha}`);
  assert.equal(a1.created, true);
  assert.equal(a2.created, false);
  assert.equal(b1.created, true);
});

test("createId mints a 12-char id over the alphabet and resolveId round-trips", () => {
  const store = createStore({ toolchainId: "t1" });
  const sha = store.sha256(new TextEncoder().encode("doc"));
  const fixedNow = 1234567890;
  const { publicId } = store.createId(sha, { now: () => fixedNow });
  assert.equal(publicId.length, ID_LENGTH);
  for (const ch of publicId) assert.ok(ID_ALPHABET.includes(ch));
  assert.deepEqual(store.resolveId(publicId), {
    sha256: sha,
    toolchainId: "t1",
    createdAt: fixedNow,
  });
});

test("resolveId of unknown id is null", () => {
  const store = createStore({ toolchainId: "t1" });
  assert.equal(store.resolveId("AAAAAAAAAAAA"), null);
});

test("readDocument and readSource round-trip bytes", () => {
  const store = createStore({ toolchainId: "t1" });
  const src = new TextEncoder().encode("# round trip\n");
  const { sha256 } = store.putSource(src);
  store.putDocument(sha256, "<p>hi</p>");
  assert.deepEqual(Array.from(store.readSource(sha256)), Array.from(src));
  assert.equal(new TextDecoder().decode(store.readDocument(sha256)), "<p>hi</p>");
  assert.equal(store.readSource("0000000000000000000000000000000000000000000000000000000000000000"), null);
  assert.equal(store.readDocument("0000000000000000000000000000000000000000000000000000000000000000"), null);
});

test("backend.list shows exactly the three key spaces, sorted", () => {
  const backend = memoryBackend();
  const store = createStore({ backend, toolchainId: "t1" });
  const src = new TextEncoder().encode("list me");
  const { sha256 } = store.putSource(src);
  store.putDocument(sha256, "<p>d</p>");
  const { publicId } = store.createId(sha256, { now: () => 1 });
  assert.deepEqual(backend.list(""), [
    `${KEY_DOCS}/t1/${sha256}`,
    `${KEY_IDS}/${publicId}`,
    `${KEY_SOURCES}/${sha256}`,
  ]);
  assert.deepEqual(backend.list(`${KEY_SOURCES}/`), [`${KEY_SOURCES}/${sha256}`]);
});

test("removeId and removeContent delete the right keys", () => {
  const backend = memoryBackend();
  const store = createStore({ backend, toolchainId: "t1" });
  const src = new TextEncoder().encode("remove me");
  const { sha256 } = store.putSource(src);
  store.putDocument(sha256, "<p>r</p>");
  const { publicId } = store.createId(sha256, { now: () => 2 });

  assert.deepEqual(store.removeId(publicId), {
    sha256,
    toolchainId: "t1",
    createdAt: 2,
  });
  assert.equal(backend.get(`${KEY_IDS}/${publicId}`), null);
  assert.equal(store.resolveId(publicId), null);
  assert.equal(store.removeId(publicId), null);

  assert.equal(store.removeContent(sha256), true);
  assert.equal(backend.get(`${KEY_DOCS}/t1/${sha256}`), null);
  assert.equal(backend.get(`${KEY_SOURCES}/${sha256}`), null);
  assert.equal(store.removeContent(sha256), false);
});

test("createId regenerates when the minted id collides with an existing key", () => {
  const store = createStore({ toolchainId: "t1" });
  const sha = store.sha256(new TextEncoder().encode("collide"));

  const first = store.createId(sha, { now: () => 1 });
  const collidingBytes = bytesFor(first.publicId);
  const freshBytes = Uint8Array.from(collidingBytes);
  freshBytes[0] = (freshBytes[0] + 1) & 0x3f;

  let calls = 0;
  const injected = (n) => {
    assert.equal(n, ID_LENGTH);
    calls += 1;
    return calls <= 2 ? collidingBytes : freshBytes;
  };

  const second = store.createId(sha, { now: () => 2, randomBytes: injected });
  assert.equal(calls, 3);
  assert.notEqual(second.publicId, first.publicId);
  assert.equal(second.publicId, idFromBytes(freshBytes));
  assert.deepEqual(store.resolveId(second.publicId), {
    sha256: sha,
    toolchainId: "t1",
    createdAt: 2,
  });
  assert.deepEqual(store.resolveId(first.publicId), {
    sha256: sha,
    toolchainId: "t1",
    createdAt: 1,
  });
});

test("newPublicId draws exactly ID_LENGTH bytes and never uses Math.random", () => {
  const seen = [];
  let called = false;
  const injected = (n) => {
    assert.equal(n, ID_LENGTH);
    called = true;
    const bytes = new Uint8Array(n);
    for (let i = 0; i < n; i++) bytes[i] = i & 0x3f;
    seen.push(bytes);
    return bytes;
  };
  const id = newPublicId({ randomBytes: injected });
  assert.ok(called);
  assert.equal(seen.length, 1);
  assert.equal(id, "ABCDEFGHIJKL");
});
