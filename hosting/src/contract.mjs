// Frozen hosting contract for the Phase 4 hosting/API/storage layer.
//
// Every constant below is derived verbatim from an ADR, the Tech Spec, or a
// research brief; do not change any value without a superseding ADR. This file
// is frozen DATA plus pure response-shape helpers — never hosting behavior.
// Implementation modules import from it and must not redefine these values.

export const MDHTML_VERSION = "1.0";

// ADR 0012 — the Publish API is source-only (upload model Option B).
export const API_PATH = "/v1/documents";

// ADR 0018 / Tech Spec "Rate limiting".
export const SOURCE_LIMIT_BYTES = 2 * 1024 * 1024; // 2 MiB
export const ARTIFACT_LIMIT_BYTES = 8 * 1024 * 1024; // 8 MiB
export const RATE_LIMITS = Object.freeze({
  perMinute: 10,
  perHour: 50,
  perDay: 200,
  concurrent: 2,
});

// ADR 0013 — private Blob key spaces. `sha256` is always SHA-256 of the
// canonical Markdown source, never of the built artifact.
export const KEY_SOURCES = "sources"; // sources/{sha256(source)}
export const KEY_DOCS = "docs"; // docs/{toolchainId}/{sha256}
export const KEY_IDS = "ids"; // ids/{publicId}
export const KEY_DENYLIST = "denylist"; // denylist/{sha256} (ADR 0018 append-only)
export const LONG_URL_PREFIX = "/d"; // /d/{toolchainId}/{sha256}

// ADR 0014 — 12-character base64url ID (72 bits), server-side CSPRNG.
export const ID_ALPHABET =
  "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
export const ID_LENGTH = 12;

// ADR 0013 / Tech Spec "Caching model".
export const CACHE_CONTENT =
  "public, max-age=31536000, immutable, s-maxage=31536000";
export const CACHE_SHORT_ID =
  "public, max-age=3600, s-maxage=86400, stale-while-revalidate=86400, stale-if-error=86400";

// ADR 0018 / Tech Spec "Abuse controls".
export const X_ROBOTS_TAG = "noindex, nofollow, noarchive";
export const ROBOTS_TXT = "User-agent: *\nDisallow: /\n";

// Tech Spec "Hosted headers add" + docs/research/d-browser-security.md
// "Proposed CSP for hosted artifacts (HTTP headers)". The artifact meta CSP is
// a subset; the header adds header-only directives (connect-src, frame-src,
// object-src, base-uri, form-action, frame-ancestors) and reuses the artifact's
// runtime hash in script-src.
export const CSP_TEMPLATE =
  "default-src 'none'; script-src 'sha256-{RUNTIME_HASH}'; style-src 'unsafe-inline'; " +
  "img-src data: blob:; font-src data:; media-src data: blob:; connect-src 'none'; " +
  "frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";
export const FIXED_HEADERS = Object.freeze({
  "X-Content-Type-Options": "nosniff",
  "Referrer-Policy": "no-referrer",
  "X-Frame-Options": "DENY",
  "Cross-Origin-Opener-Policy": "same-origin",
  "Cross-Origin-Resource-Policy": "same-origin",
  "Origin-Agent-Cluster": "?1",
  "Permissions-Policy":
    "camera=(), microphone=(), geolocation=(), payment=(), usb=(), serial=(), hid=()",
  "Strict-Transport-Security": "max-age=63072000; includeSubDomains; preload",
});

// Frozen E-MDHSEC-* codes the hosting API reuses (Tech Spec addendum).
export const CODES = Object.freeze({
  UNSAFE_URI: "E-MDHSEC-012",
  UNSAFE_ASSET_PATH: "E-MDHSEC-014",
  UNSAFE_ARTIFACT: "E-MDHSEC-018",
});

// API-surface codes — additive to SPEC §16, never reused.
export const API_CODES = Object.freeze({
  SOURCE_TOO_LARGE: "E-API-001",
  RATE_LIMITED: "E-API-002",
  MALFORMED_MULTIPART: "E-API-003",
  MISSING_SOURCE: "E-API-004",
  ASSET_TOO_LARGE: "E-API-005",
  BUILD_REJECTED: "E-API-006",
  DENIED: "E-API-007",
});

// Tech Spec "API contract" — success and structured error shapes.
export function apiSuccess({ id, url, sha256, mdhtmlVersion = MDHTML_VERSION }) {
  return { id, url, sha256, mdhtmlVersion };
}

export function apiError(code, message, extra = {}) {
  const error = { code, message };
  if (Number.isInteger(extra.line)) error.line = extra.line;
  if (Number.isInteger(extra.column)) error.column = extra.column;
  return { error };
}

export function jsonResponse(status, body, headers = {}) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json; charset=utf-8", ...headers },
  });
}

export function textResponse(status, body, headers = {}) {
  return new Response(body, {
    status,
    headers: { "Content-Type": "text/plain; charset=utf-8", ...headers },
  });
}

export function errorResponse(status, code, message, extra = {}, headers = {}) {
  return jsonResponse(status, apiError(code, message, extra), headers);
}

// Tech Spec "Hosting architecture" — public URL shapes (frozen).
export function publicUrl(baseUrl, publicId) {
  return `${String(baseUrl).replace(/\/+$/, "")}/${publicId}`;
}

export function longUrl(baseUrl, toolchainId, sha256) {
  return `${String(baseUrl).replace(/\/+$/, "")}${LONG_URL_PREFIX}/${toolchainId}/${sha256}`;
}
