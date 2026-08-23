import {
  API_CODES,
  ARTIFACT_LIMIT_BYTES,
  CODES,
  RATE_LIMITS,
  SOURCE_LIMIT_BYTES,
  apiSuccess,
  errorResponse,
  jsonResponse,
  publicUrl,
} from "./contract.mjs";

const WINDOWS = [
  { ms: 60_000, limit: RATE_LIMITS.perMinute },
  { ms: 3_600_000, limit: RATE_LIMITS.perHour },
  { ms: 86_400_000, limit: RATE_LIMITS.perDay },
];

const SCHEME_RE = /^[A-Za-z][A-Za-z0-9+.-]*:/;
const BUILD_ERROR_RE = /(E-MDHSEC-\d{3}|E-FMT-\d{2}|E-CLI-\d{2})/;

export function isSafeAssetPath(path) {
  return (
    typeof path === "string" &&
    path !== "" &&
    !path.startsWith("/") &&
    !SCHEME_RE.test(path) &&
    !path.split("/").includes("..") &&
    !path.includes("\\") &&
    !path.includes("\0")
  );
}

export function createRateLimiter({ now = () => Date.now() } = {}) {
  const buckets = new Map();
  const inflight = new Map();

  function acquire(ip) {
    const t = now();
    let windows = buckets.get(ip);
    if (!windows) {
      windows = WINDOWS.map(({ ms }) => ({
        start: Math.floor(t / ms) * ms,
        count: 0,
      }));
      buckets.set(ip, windows);
    }

    let retryAfterSeconds = 0;
    for (let i = 0; i < WINDOWS.length; i++) {
      const { ms, limit } = WINDOWS[i];
      const window = windows[i];
      const start = Math.floor(t / ms) * ms;
      if (window.start !== start) {
        window.start = start;
        window.count = 0;
      }
      if (window.count >= limit) {
        const seconds = Math.max(
          1,
          Math.min(86400, Math.ceil((window.start + ms - t) / 1000)),
        );
        retryAfterSeconds =
          retryAfterSeconds === 0
            ? seconds
            : Math.min(retryAfterSeconds, seconds);
      }
    }
    if (retryAfterSeconds > 0) {
      return { allowed: false, retryAfterSeconds };
    }

    const count = inflight.get(ip) ?? 0;
    if (count >= RATE_LIMITS.concurrent) {
      return { allowed: false, retryAfterSeconds: 0 };
    }
    inflight.set(ip, count + 1);
    for (const window of windows) {
      window.count += 1;
    }
    return { allowed: true, retryAfterSeconds: 0 };
  }

  function release(ip) {
    const count = inflight.get(ip) ?? 0;
    if (count > 0) {
      inflight.set(ip, count - 1);
    }
  }

  return { acquire, release };
}

export async function handlePublish(
  request,
  {
    toolchain,
    store,
    denyList,
    rateLimiter,
    ip,
    baseUrl = "https://docs.example",
  },
) {
  const gate = await rateLimiter.acquire(ip);
  try {
    if (!gate.allowed) {
      return errorResponse(429, API_CODES.RATE_LIMITED, "rate limited", {}, {
        "Retry-After": String(gate.retryAfterSeconds),
      });
    }

    let formData;
    try {
      formData = await request.formData();
    } catch {
      return errorResponse(400, API_CODES.MALFORMED_MULTIPART, "malformed multipart form");
    }

    const sourceFile = formData.get("source");
    if (!(sourceFile instanceof Blob)) {
      return errorResponse(400, API_CODES.MISSING_SOURCE, "missing source field");
    }
    const sourceBytes = new Uint8Array(await sourceFile.arrayBuffer());
    if (sourceBytes.length > SOURCE_LIMIT_BYTES) {
      return errorResponse(413, API_CODES.SOURCE_TOO_LARGE, "source exceeds 2 MiB");
    }

    const source = new TextDecoder().decode(sourceBytes);
    const assets = [];
    for (const entry of formData.getAll("asset")) {
      if (!isSafeAssetPath(entry.name)) {
        return errorResponse(400, CODES.UNSAFE_ASSET_PATH, "unsafe asset path: " + entry.name);
      }
      assets.push({ name: entry.name, bytes: new Uint8Array(await entry.arrayBuffer()) });
    }

    const sha256 = store.sha256(sourceBytes);
    if (await denyList.isDenied(sha256)) {
      return errorResponse(403, API_CODES.DENIED, "document is not available");
    }

    const built = await toolchain.build({ source, assets });
    if (built.exitCode !== 0) {
      const match = built.stderr.match(BUILD_ERROR_RE);
      const code = match ? match[0] : API_CODES.BUILD_REJECTED;
      return errorResponse(422, code, built.stderr.trim());
    }

    const audited = await toolchain.audit(built.html);
    if (audited.exitCode !== 0 || audited.report?.safe !== true) {
      const code =
        audited.report?.html === "unsafe" || false
          ? CODES.UNSAFE_ARTIFACT
          : API_CODES.BUILD_REJECTED;
      const detail = audited.report
        ? ` (html=${audited.report.html}, css=${audited.report.css}, runtime=${audited.report.runtime})`
        : "";
      return errorResponse(422, code, "artifact audit failed" + detail);
    }

    if (new TextEncoder().encode(built.html).length > ARTIFACT_LIMIT_BYTES) {
      return errorResponse(413, API_CODES.SOURCE_TOO_LARGE, "built artifact exceeds 8 MiB");
    }

    await store.putSource(sourceBytes);
    await store.putDocument(sha256, built.html);
    const { publicId } = store.createId(sha256);
    return jsonResponse(
      200,
      apiSuccess({ id: publicId, url: publicUrl(baseUrl, publicId), sha256 }),
    );
  } finally {
    rateLimiter.release(ip);
  }
}
