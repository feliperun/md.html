// Blob storage layout for the hosting layer — the three content-addressed key
// spaces from ADR 0013, plus CSPRNG public-ID minting (ADR 0014).
import { createHash, randomBytes as nodeRandomBytes } from "node:crypto";
import {
  ID_ALPHABET,
  ID_LENGTH,
  KEY_DOCS,
  KEY_IDS,
  KEY_SOURCES,
} from "./contract.mjs";

export function memoryBackend() {
  const map = new Map();
  return {
    put(key, bytes) {
      map.set(key, Uint8Array.from(bytes));
    },
    get(key) {
      const value = map.get(key);
      return value === undefined ? null : Uint8Array.from(value);
    },
    delete(key) {
      return map.delete(key);
    },
    list(prefix) {
      return [...map.keys()].filter((key) => key.startsWith(prefix)).sort();
    },
  };
}

export function defaultRandomBytes(n) {
  return nodeRandomBytes(n);
}

export function newPublicId({ randomBytes = defaultRandomBytes } = {}) {
  const bytes = randomBytes(ID_LENGTH);
  let id = "";
  for (let i = 0; i < ID_LENGTH; i++) {
    id += ID_ALPHABET[bytes[i] & 0x3f];
  }
  return id;
}

export function createStore({ backend = memoryBackend(), toolchainId }) {
  function sha256(bytes) {
    return createHash("sha256").update(bytes).digest("hex");
  }

  function putSource(bytes) {
    const digest = sha256(bytes);
    const key = `${KEY_SOURCES}/${digest}`;
    const created = backend.get(key) === null;
    backend.put(key, bytes);
    return { sha256: digest, created };
  }

  function putDocument(sourceSha256, html) {
    const key = `${KEY_DOCS}/${toolchainId}/${sourceSha256}`;
    const created = backend.get(key) === null;
    backend.put(key, new TextEncoder().encode(html));
    return { key, created };
  }

  function createId(sourceSha256, { now = () => Date.now(), randomBytes } = {}) {
    let publicId;
    let key;
    const mapping = { sha256: sourceSha256, toolchainId, createdAt: now() };
    do {
      publicId = newPublicId(
        randomBytes === undefined ? undefined : { randomBytes },
      );
      key = `${KEY_IDS}/${publicId}`;
    } while (backend.get(key) !== null);
    backend.put(key, new TextEncoder().encode(JSON.stringify(mapping)));
    return { publicId };
  }

  function resolveId(publicId) {
    const raw = backend.get(`${KEY_IDS}/${publicId}`);
    return raw === null ? null : JSON.parse(new TextDecoder().decode(raw));
  }

  function readDocument(sourceSha256) {
    return backend.get(`${KEY_DOCS}/${toolchainId}/${sourceSha256}`);
  }

  function readSource(sourceSha256) {
    return backend.get(`${KEY_SOURCES}/${sourceSha256}`);
  }

  function removeId(publicId) {
    const key = `${KEY_IDS}/${publicId}`;
    const raw = backend.get(key);
    if (raw === null) return null;
    backend.delete(key);
    return JSON.parse(new TextDecoder().decode(raw));
  }

  function removeContent(sourceSha256) {
    const docKey = `${KEY_DOCS}/${toolchainId}/${sourceSha256}`;
    const sourceKey = `${KEY_SOURCES}/${sourceSha256}`;
    const docExisted = backend.get(docKey) !== null;
    const sourceExisted = backend.get(sourceKey) !== null;
    backend.delete(docKey);
    backend.delete(sourceKey);
    return docExisted || sourceExisted;
  }

  return {
    sha256,
    putSource,
    putDocument,
    createId,
    resolveId,
    readDocument,
    readSource,
    removeId,
    removeContent,
  };
}
