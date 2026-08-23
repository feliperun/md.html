import {
  CSP_TEMPLATE,
  FIXED_HEADERS,
  KEY_DENYLIST,
  ROBOTS_TXT,
  X_ROBOTS_TAG,
  textResponse,
} from "./contract.mjs";

const META_TAG = /<meta\b[^>]*>/gi;
const HTTP_EQUIV = /http-equiv\s*=\s*(["'])(.*?)\1/i;
const CONTENT_ATTR = /content\s*=\s*(["'])(.*?)\1/i;
const RUNTIME_HASH = /'sha256-([A-Za-z0-9+/=_-]{40,64})'/;

export function extractRuntimeHash(html) {
  const metas = html.match(META_TAG) ?? [];
  for (const meta of metas) {
    const equiv = HTTP_EQUIV.exec(meta);
    if (
      equiv === null ||
      equiv[2].trim().toLowerCase() !== "content-security-policy"
    ) {
      continue;
    }
    const content = CONTENT_ATTR.exec(meta);
    if (content === null) return null;
    const hash = RUNTIME_HASH.exec(content[2]);
    return hash === null ? null : hash[1];
  }
  return null;
}

export function securityHeaders(html) {
  const runtimeHash = extractRuntimeHash(html);
  if (runtimeHash === null) throw new Error("missing runtime hash");
  return {
    "Content-Security-Policy": CSP_TEMPLATE.replace(
      "{RUNTIME_HASH}",
      runtimeHash,
    ),
    "X-Robots-Tag": X_ROBOTS_TAG,
    ...FIXED_HEADERS,
  };
}

export function robotsTxtResponse() {
  return textResponse(200, ROBOTS_TXT);
}

export function createDenyList({ backend }) {
  const keyOf = (sha256) => `${KEY_DENYLIST}/${sha256}`;
  return {
    async add(sha256) {
      await backend.put(keyOf(sha256), Uint8Array.of(1));
    },
    async isDenied(sha256) {
      return (await backend.get(keyOf(sha256))) !== null;
    },
    async list() {
      return backend.list(KEY_DENYLIST);
    },
  };
}

export async function takedown({
  publicId,
  store,
  denyList,
  purge = async () => {},
  log = () => {},
}) {
  const mapping = await store.resolveId(publicId);
  if (mapping === null) return { removed: false };
  const { sha256 } = mapping;
  await store.removeId(publicId);
  await store.removeContent(sha256);
  await denyList.add(sha256);
  await purge(sha256);
  await log({
    publicId,
    sha256,
    toolchainId: mapping.toolchainId,
    at: Date.now(),
  });
  return { removed: true };
}
