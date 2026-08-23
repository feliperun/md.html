---
type: ADR
id: "0010"
title: "Content Security Policy strategy"
status: proposed
date: 2026-08-22
---

## Context

An mdhtml artifact is one self-contained HTML file consumed under `file://`, as an
email attachment, or from any static host. Under `file://` there are no HTTP
response headers and the origin is opaque, so `<meta http-equiv>` is the only CSP
delivery channel and header-only directives (`frame-ancestors`, `sandbox`,
`report-*`) do not exist locally (docs/research/d-browser-security.md).

The v1.0 baseline ships `default-src 'none'; script-src 'unsafe-inline';
style-src 'unsafe-inline'` via meta. `'unsafe-inline'` for scripts authorizes any
injected inline script, event handler, or `javascript:` URL, so it cannot satisfy
PRD §11 (the official runtime is the only trusted executable; script hashes should
eliminate `'unsafe-inline'`) or PRD §12 (strictest CSP compatible with mdhtml, the
meta-vs-header split, `file://` vs HTTP differences, inline styles, and mandatory
HTTP response headers on the hosting service). The Tech Spec's "Runtime attack
surface" and "CSP" sections decide the final policy.

## Decision

**Every generated artifact carries exactly one meta CSP as the first element in
`<head>`, with a hash-only `script-src`:**

```text
default-src 'none';
script-src 'sha256-<RUNTIME_HASH>';
style-src 'unsafe-inline';
img-src data: blob:;
font-src data:;
media-src data: blob:;
connect-src 'none';
frame-src 'none';
object-src 'none';
base-uri 'none';
form-action 'none'
```

- `RUNTIME_HASH` is the SHA-256 that `mdhtml build` computes over the exact text
  content of the single `script#mdhtml-runtime` as the browser parses it (LF line
  endings, unescaped), per fragment composition. **CSP script hashes authorize the
  exact runtime; no nonce, no `'unsafe-eval'`, no `'unsafe-inline'` for scripts.**
  The hash simultaneously authorizes and integrity-pins the only executable
  surface. `'unsafe-inline'` remains for styles only: author CSS is mdhtml's
  presentation model, and CSS-originated requests are closed by the
  scheme-limited `img-src`/`font-src`/`media-src` plus the CSS policy (ADR 0008).
- The hosting service serves the same policy via HTTP response header, adding
  `frame-ancestors 'none'` (header-only) plus `X-Content-Type-Options: nosniff`,
  `Referrer-Policy: no-referrer`, `X-Frame-Options: DENY`,
  `Cross-Origin-Opener-Policy: same-origin`,
  `Cross-Origin-Resource-Policy: same-origin`, `Origin-Agent-Cluster: ?1`,
  `Permissions-Policy: camera=(), microphone=(), geolocation=(), payment=(),
  usb=(), serial=(), hid=()`, and `Strict-Transport-Security:
  max-age=63072000; includeSubDomains; preload`. Meta and header policies are
  additive, so the intersection equals the header policy.
- `sandbox` is rejected in both environments: on a top-level document it breaks
  the clipboard API (opaque origin loses secure-context status) and downloads.
  `report-to`/`report-uri` are absent by design — the project has no telemetry.
- `fonts.url` is the only non-portable relaxation: the declared `https:` origin is
  appended to `font-src`; the official hosting rejects `fonts.url` documents at
  MVP.

Because the meta CSP travels inside the artifact, third-party static hosts
(GitHub Pages, S3, nginx) get the same enforcement as `file://` — safety is a
property of the artifact, not the host (PRD Constraint 5).

## Options considered

- **Keep v1.0 `script-src 'unsafe-inline'`** (rejected): authorizes injected
  scripts, inline handlers, and `javascript:` URLs — the exact PRD §4 threats.
- **Nonces instead of hashes** (rejected): a nonce in a static artifact is
  readable by any content injection and authorizes an attribute, not content;
  a hash binds the exact bytes (d-browser-security.md).
- **`'unsafe-eval'`** (not needed): the runtime is classic JS with no
  `eval`/`new Function`; the audit asserts this stays true.
- **Hash `#mdhtml-source` too** (rejected): it is non-executable
  (`type="text/markdown"`) and must remain hand-editable; hashing it would break
  every prose-only edit.
- **Strict `style-src` (hash every style block)** (rejected): author CSS varies
  per document and inline `style=""` attributes are not hashable at all; CSS
  exfiltration is closed by scheme-limited sources and the CSS policy (ADR 0008),
  not by CSP.
- **`sandbox` for hosted documents** (rejected): breaks clipboard,
  secure-context, and download behavior in top-level documents.
- **HTTP-header-only enforcement** (impossible): `file://` and third-party
  static hosts cannot set headers; the meta CSP is the portable enforcement
  layer.
- **`report-to`/`report-uri`** (absent): no telemetry by design.

## Consequences

- `mdhtml build` must compute the runtime hash over the exact emitted bytes (LF,
  unescaped) and emit the meta CSP as the first element in `<head>`; the v1.0
  `script-src 'unsafe-inline'` baseline is superseded (`'unsafe-inline'` survives
  for styles only).
- The runtime stays one classic inline script, byte-reproducible per fragment
  composition — reinforcing ADR 0004 (dependency-free runtime) and ADR 0005
  (fragment boundaries).
- A hand-edited runtime fails CSP closed and the document degrades to the
  `<noscript>` fallback; `mdhtml audit` recomputes the embedded runtime hash and
  compares it to the manifest and the CSP value.
- Hosting must apply the full header set verbatim on every document response.
- Tech Spec Phases and Tasks that depend on this decision: Phase 2 "Compute
  runtime hash and emit hash-only CSP"; Phase 3 "Implement `mdhtml audit
  [--json]`" (runtime-integrity check); Phase 4 "Apply isolated origin, security
  headers, robots/indexing, and takedown controls"; Phase 6 security fixture
  corpus and E2E coverage verifying the hash-only CSP over `file://`.
