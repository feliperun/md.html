# Browser security research — Agent D (d-browser-security)

> Input to the mdhtml Tech Spec (PRD §47-49). This node researches and recommends; it
> does not implement. Scope: CSP delivery (`<meta http-equiv>` vs HTTP headers), script
> hashing and SRI, inline-style behavior under a strict `style-src`, what is available
> under `file://` versus `https://`, origin isolation for the future hosting service
> (PRD §22), and browser differences with reliable knowledge only.
>
> Sources reviewed: `docs/prd/mdhtml-safety-hosting-prd.md` (§4, §7, §11-12, §22,
> §47-51), `SPEC.md` (FMT-01, §3 canonical source handling, §13 asset embedding, §17
> runtime fragment manifest), `docs/ARCHITECTURE.md` (security model, runtime & hosting
> contracts), `runtime/src/bootstrap.js`, `runtime/src/chrome.js`,
> `runtime/src/canonical.js`.
>
> Current baseline in the spec skeleton: `default-src 'none'; script-src 'unsafe-inline';
> style-src 'unsafe-inline'; img-src data: blob:; font-src data:; media-src data: blob:`
> delivered via `<meta http-equiv>`. The recommendation below keeps the perimeter and
> tightens `script-src` from `'unsafe-inline'` to a runtime hash.

## file:// vs HTTP: what security mechanisms are available in each

The generated artifact is one self-contained HTML file: a classic inline runtime
(`script#mdhtml-runtime`), a canonical Markdown block (`script#mdhtml-source` with
`type="text/markdown"`), embedded asset blocks (`type="application/octet-stream"`),
inline `<style>` blocks, and embedded fonts/images. No external resources exist in a
portable document (ARCHITECTURE: "CSP é o perímetro"; SPEC §13). That single fact
drives almost everything below.

### Under `file://` (local open, email attachment, drag-and-drop)

Available:

- **`<meta http-equiv="Content-Security-Policy">`** is honored by all modern engines
  (Chrome/Edge, Firefox, Safari incl. iOS) and is the *only* CSP delivery mechanism that
  exists for a file opened from disk. It enforces the full directive set except the
  header-only directives listed below.
- **Script hashes in `script-src`** (`'sha256-...'`) work in meta-delivered CSP and are
  computed over the inline script's parsed text content — the exact fit for an inline
  runtime.
- **Secure-context APIs**: per the Secure Contexts spec, `file://` is a potentially
  trustworthy origin, so `navigator.clipboard.writeText` (with a synchronous user
  gesture) and `crypto.subtle` are generally available. Browser variance exists —
  `runtime/src/chrome.js` already carries the `execCommand` fallback for the cases where
  they are not.
- **`blob:` URLs** for the ≥32 KiB lazy image hydration (`URL.createObjectURL`) and the
  Download button work from `file://`.

Not available or unreliable under `file://`:

- **HTTP response headers** — there are none. Everything header-borne is out of reach:
  CSP delivered by header, `frame-ancestors`, `sandbox`, `report-to`/`report-uri`,
  HSTS, `Cross-Origin-Opener-Policy`, `Cross-Origin-Embedder-Policy`,
  `Cross-Origin-Resource-Policy`, `Origin-Agent-Cluster`, `Permissions-Policy`,
  `Referrer-Policy`, `X-Content-Type-Options`, `X-Frame-Options`.
- **Opaque origin** — the document's origin is `null`/opaque. Consequences:
  - ES modules (`type="module"` and dynamic `import()`) are blocked by CORS; only
    classic inline scripts execute (documented in ARCHITECTURE and AGENTS.md).
  - `fetch`/XHR to any `http(s)` origin is blocked by CORS (`Origin: null`); `fetch` of
    `data:` URLs is also unreliable. The runtime must not depend on fetch — it does not
    (bootstrap.js imports are ESM test-only composition; the shipped fragments are
    classic IIFEs).
  - `history.pushState` **throws** a `SecurityError` (AGENTS.md gotcha); section
    navigation is hash + `hashchange`.
  - Cookies and `localStorage` are unreliable/blocked (origin is opaque). The runtime
    uses neither (chrome.js uses DOM, Blob URLs, and the clipboard API only).
- **Service workers** cannot be registered on `file://` (requires `http(s)`).
- **`frame-ancestors`, `sandbox`, `report-*`** cannot be expressed at all: they are
  forbidden in meta CSP by the CSP3 spec, and there is no header to carry them. A
  standalone local file also has no meaningful embedding attacker, so this is an
  acceptable gap.
- **SRI** (`integrity=` on `<script src>`/`<link rel=stylesheet>`) applies only to
  external resources; a portable artifact has none, so SRI is inapplicable locally.

### Under `https://` (official hosting, GitHub Pages, S3, Vercel, nginx, …)

Everything the local file gets, **plus**:

- CSP delivered via HTTP header, enforced from the first byte of the response and
  immune to in-document tampering (a malicious edit cannot delete a header).
- Header-only CSP directives: `frame-ancestors`, `sandbox`, `report-to`/`report-uri`
  (the last only if a reporting endpoint exists — by default this project has no
  telemetry).
- Context/transport headers: HSTS, COOP, CORP, `Origin-Agent-Cluster`,
  `Permissions-Policy`, `Referrer-Policy`, `X-Content-Type-Options`, `X-Frame-Options`.
- A real, non-opaque origin — which is exactly what enables the origin-isolation
  strategy in PRD §22 (a dedicated registrable domain for user content).

When a hosted document also carries its own meta CSP, **both policies are enforced**
(policies are additive; the effective policy is the intersection). Keeping the
artifact's meta CSP identical to the header CSP means the header is a strict superset
(it adds only header-only directives) and the intersection equals the header.

## Runtime integrity strategy

### Goal

Authorize exactly the official mdhtml runtime bytes — and nothing else — without
`'unsafe-inline'` for scripts. Per PRD §11: hash the canonical runtime, use CSP script
hashes, evaluate SRI, keep runtime generation deterministic, verify runtime integrity
in `mdhtml audit`.

### Recommendation: CSP `script-src 'sha256-<RUNTIME_HASH>'` over the single runtime element

- The document has exactly one executable script: `script#mdhtml-runtime` (SPEC §2
  skeleton). `script#mdhtml-source[type="text/markdown"]` and asset blocks
  (`type="application/octet-stream"`) are **non-executable script types** and are not
  governed by `script-src` at all. So one hash authorizes the entire executable surface.
- Per CSP3, when `script-src` contains a hash, `'unsafe-inline'` is **ignored** — there
  is no need to also list it, and omitting it means:
  - any `<script>` whose text is not byte-identical to the runtime is blocked;
  - inline event handlers (`onclick=` etc.) are blocked;
  - `javascript:` URLs are blocked (they require `'unsafe-inline'` or `'unsafe-hashes'`).
- The hash is computed by `mdhtml build` over the **exact final bytes the browser will
  hash**: the text content of the runtime `<script>` element as parsed. Two build rules
  follow from this:
  - emit the runtime with **LF line endings** — the HTML parser normalizes CRLF/CR to
    LF before hashing, so CRLF bytes would produce a hash that never matches;
  - do **not** HTML-entity-escape the runtime content — the parser decodes entities
    before hashing. The runtime is raw text (only `</script` is forbidden, already an
    FMT-02 invariant), so writing it verbatim is safe.
- **Per-document composition**: the embedded runtime is the concatenation of the
  selected fragments (`core` + `copy` + [`toc`] + [`lightbox`], SPEC §17). The hash must
  therefore be computed per composition, not per fragment. Builds are already required
  to be byte-reproducible (SPEC §17, `runtime/build.mjs check`), so the hash is stable
  for a given (runtime version, fragment selection) and deterministic across machines.
- The committed fragment manifest (`mdhtml/manifest/1.0`) already records SHA-256 per
  fragment. Reuse it: `mdhtml audit` recomputes the embedded runtime's hash from the
  document bytes and compares it against the manifest and against the CSP value
  (PRD §11 "runtime integrity verification during mdhtml audit").
- **Tamper-evidence is a feature**: if a hand-editor changes the runtime block, the CSP
  fails closed, the runtime does not mount, and the document degrades to the `<noscript>`
  pre-formatted fallback while `check`/`audit` flag the mismatch. Prose edits keep
  working because `#mdhtml-source` is not hashed (SPEC §3: editing prose must work
  without a rebuild).

### What we explicitly do not hash or authorize

- **Do not hash `#mdhtml-source`.** It is non-executable (`type="text/markdown"`) and it
  must remain hand-editable. Hashing it would break every prose-only edit, violating
  SPEC §3.
- **Do not use nonces.** A nonce in a static artifact is visible to any content
  injection that can read the file (there is no per-response secret under `file://`), and
  a nonce authorizes an *attribute*, not content. A hash binds the exact bytes: it is
  simultaneously authorization and integrity pinning.
- **No `'unsafe-eval'`.** The runtime is dependency-free classic JS; `eval`/`new
  Function` must remain absent (open question O2 below asserts this in the audit).

### SRI

SRI (`integrity=` on `<script src>`/`<link>`) applies to external resources only. The
self-contained artifact has none, so **SRI has no direct application in the current
architecture** — the CSP script hash is the SRI-equivalent for inline code. Keep SRI in
mind for two future cases only: (1) if the hosting service ever externalizes any asset
(fonts/runtime) from the document, serve it with `integrity` plus a host allowlist in
`script-src`/`font-src`; (2) the CLI release binary already uses checksums (DIST-01).
Note that a hash in `script-src` does **not** authorize an external script — external
scripts are authorized by host/scheme sources, so if externalization ever happens the
policy must add the host too, not just the hash.

## Proposed CSP for local artifacts

Delivered by `<meta http-equiv="Content-Security-Policy">`, placed as the **first
element in `<head>`** (meta CSP outside `<head>` is ignored; some WebKit versions are
sensitive to late placement — see browser section). Exactly **one** meta CSP per
document.

```html
<meta http-equiv="Content-Security-Policy" content="
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
  form-action 'none'">
```

Directive-by-directive rationale:

- `default-src 'none'` — everything is closed unless explicitly opened; a forgotten
  directive stays closed. This is the existing perimeter (ARCHITECTURE) and stays.
- `script-src 'sha256-<RUNTIME_HASH>'` — replaces the baseline `'unsafe-inline'`. Only
  the exact runtime runs; inline handlers, `javascript:` URLs, external and injected
  scripts are blocked. This is the single most valuable change versus the 1.0 baseline.
- `style-src 'unsafe-inline'` — **kept deliberately**. mdhtml's presentation model is
  author CSS: the `#mdhtml-tokens`/`#mdhtml-theme`/`#mdhtml-user` `<style>` elements, the
  `<noscript>` fallback style, and author inline `style=""` attributes all need it.
  Reasoning:
  - CSS cannot execute code in any current engine (the old IE `expression()` is dead);
    no script execution is enabled by allowing CSS.
  - Network exfiltration through CSS is closed elsewhere: CSS `url()` loads are governed
    by `img-src`, `@font-face` by `font-src`, `@import` by `style-src` — all restricted
    to `data:`/`blob:` (and declared font origins when `fonts.url` is used), so no
    CSS-originated request can reach the network.
  - Hashing every author `<style>` block is impractical (content varies per document)
    and inline `style=""` attributes are not hashable at all (only `'unsafe-inline'` or
    the deprecated/risky `'unsafe-hashes'` cover them). The cost/benefit is clearly in
    favor of `'unsafe-inline'` here. CSS-exfiltration defenses belong to content
    validation (Agent C), not CSP.
- `img-src data: blob:` — embedded images below 32 KiB as `data:` URIs, at/above 32 KiB
  as lazy `blob:` URLs (SPEC §13 / UI-04). Also governs CSS `url()` loads, closing CSS
  exfiltration. No network images.
- `font-src data:` — embedded OFL/Apache WOFF2 fonts. If the document uses `fonts.url`,
  append the declared `https:` origin(s) here and keep the existing
  `data-mdhtml-portable="false"` marking.
- `media-src data: blob:` — embedded audio/video, mirroring images.
- `connect-src 'none'` — no `fetch`/XHR/WebSocket/EventSource/beacon (and `a[ping]` in
  Chromium). The runtime performs no network I/O.
- `frame-src 'none'`, `object-src 'none'` — no iframes, `<object>`, `<embed>` (PRD §4
  threat list).
- `base-uri 'none'` — no `<base>` injection to redirect relative `data:`/`blob:` URLs.
  Note: `base-uri` does **not** fall back to `default-src`, so the 1.0 baseline CSP
  (`default-src 'none'` alone) leaves `<base>` unconstrained — this directive is a real
  tightening, not a restatement.
- `form-action 'none'` — no form submissions. Same fallback caveat as `base-uri`:
  `form-action` does not fall back to `default-src`, so it must be listed explicitly.

Absent intentionally:

- `frame-ancestors` — cannot be set via meta (CSP3) and is meaningless for a standalone
  file; the hosted header adds it.
- `sandbox` — cannot be set via meta, and would break the chrome anyway (see hosting
  section for why it is rejected).
- `report-to`/`report-uri` — cannot be set via meta and this project has no telemetry by
  design.

Failure mode: if JS is disabled, or the runtime block was hand-modified (hash mismatch),
the runtime does not mount and the `<noscript>` fallback renders the canonical source as
pre-formatted text — graceful degradation, not a broken page.

## Proposed CSP for hosted artifacts (HTTP headers)

The official hosting service computes `RUNTIME_HASH` server-side at publish time (it
already re-validates the document bytes per PRD §56) and serves the same immutable
artifact with:

```http
Content-Type: text/html; charset=utf-8
Content-Security-Policy: default-src 'none'; script-src 'sha256-<RUNTIME_HASH>'; style-src 'unsafe-inline'; img-src data: blob:; font-src data:; media-src data: blob:; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'
X-Content-Type-Options: nosniff
Referrer-Policy: no-referrer
X-Frame-Options: DENY
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Resource-Policy: same-origin
Origin-Agent-Cluster: ?1
Permissions-Policy: camera=(), microphone=(), geolocation=(), payment=(), usb=(), serial=(), hid=()
Strict-Transport-Security: max-age=63072000; includeSubDomains; preload
```

Because documents are immutable and content-addressed, per-document headers (with the
per-document runtime hash) are fully cacheable on the CDN.

### Exactly which directives differ between local and hosted, and why

| Directive / header | Local (`file://`, meta) | Hosted (header) | Why it differs |
|---|---|---|---|
| `default-src`, `script-src`, `style-src`, `img-src`, `font-src`, `media-src`, `connect-src`, `frame-src`, `object-src`, `base-uri`, `form-action` | identical values | identical values | Same bytes served; the policy that lives in the artifact must not change when hosted. |
| `frame-ancestors 'none'` | absent | present | Cannot be expressed in meta; only meaningful when the document is embeddable by other origins (hosted). Anti-clickjacking. |
| `sandbox` | absent | absent | Rejected in both (see below). |
| `report-to`/`report-uri` | absent | absent | No telemetry by design; only a staging `Content-Security-Policy-Report-Only` if ever needed. |
| `Content-Security-Policy-Report-Only` | impossible (meta cannot be report-only) | staging only | Verifies policy changes before rollout; no production endpoint. |
| `X-Content-Type-Options`, `Referrer-Policy`, `X-Frame-Options`, HSTS, COOP, CORP, `Origin-Agent-Cluster`, `Permissions-Policy` | absent | present | Transport/context headers exist only under HTTP; they harden delivery and isolate the browsing context. |

### Why `sandbox` is rejected for hosted documents (important finding)

The CSP `sandbox` directive is tempting as extra isolation (opaque origin even on the
hosting domain), but applied to a **top-level document** it breaks the runtime chrome:

- `sandbox allow-scripts` without `allow-same-origin` makes the document's origin
  opaque → the document is **not a secure context** (`isSecureContext` false), so
  `navigator.clipboard.writeText` is unavailable. `chrome.js` would silently degrade to
  the `execCommand` fallback (behavioral regression to verify live).
- Without `allow-downloads` (Chromium), the **Download button is blocked** — sandboxed
  top-level documents cannot trigger downloads.
- The document would also lose `crypto.subtle` and any future secure-context capability.

Isolation for hosted documents therefore comes from the dedicated origin plus COOP/CORP/
credentials policy (below), not from `sandbox`. Revisit only if the chrome is adapted
and the full feature set is verified in a sandboxed top-level document.

### Third-party static hosts

GitHub Pages, S3, plain nginx, etc. cannot all set the headers above (GitHub Pages, for
example, cannot set a custom CSP header). For those, **the artifact's own meta CSP is the
entire enforcement layer**. This is precisely why safety must be a property of the
artifact (PRD §1, Constraint 5): the meta CSP travels with the file and gives
third-party hosting the same script/style/resource lockdown as `file://`.

## Origin isolation strategy for hosting

PRD §22: user-generated documents MUST NOT share the authentication/security origin of
administrative applications.

### Recommendation: dedicated registrable domain for user content

```
mdhtml.example   → application / publish API / admin (auth, cookies, privileged APIs)
docs.example     → user-generated documents (read-only, static, no credentials)
```

- A separate registrable domain (different eTLD+1) is the strongest answer: the document
  origin shares **nothing** with the app origin — no cookies (even `Domain=`-scoped),
  no `localStorage`, no same-origin access to API endpoints.
- A different subdomain of the same registrable domain (`docs.mdhtml.example`) is a
  different *origin* but still shares `Domain=mdhtml.example` cookies. If that route is
  taken, the app MUST use host-only cookies with the `__Host-` prefix and
  `SameSite=Strict`, and never set `Domain=`-scoped cookies. PRD §22 explicitly
  considers a dedicated registrable domain feasible — prefer it.

### Layered isolation for a hosted document (defense in depth, PRD §7)

1. **Dedicated origin** — the document's origin hosts only immutable, read-only static
   content; it carries no auth, no admin surface, no server-rendered user input.
2. **CSP closes every network sink from inside the document** — `connect-src 'none'`,
   `frame-src 'none'`, `object-src 'none'`, `img-src data: blob:`, `font-src data:`:
   even a fully compromised document cannot exfiltrate or call the API.
3. **Cross-origin request failure** — the publish API lives on the app origin and must
   never return `Access-Control-Allow-Origin` for the docs origin; CORS blocks reads
   even if an injected script tried to fetch.
4. **Cookie policy** — auth cookies are `__Host-`, `SameSite=Strict`, `Secure`; a
   cross-site document cannot attach them to API requests.
5. **COOP `same-origin`** — the document's browsing context group is isolated from
   cross-origin popups/windows.
6. **CORP `same-origin`** — other origins cannot embed the document as a subresource
   (`<script src>`/`<img>`/iframe loads by third parties).
7. **`Origin-Agent-Cluster: ?1`** — opt into an origin-keyed agent cluster (Chromium),
   isolating the document's agent from other origins on the same site.
8. **`Permissions-Policy`** — camera, microphone, geolocation, payment, USB, serial,
   HID are disabled; WebRTC/sensors are not reliably governed by CSP, so this is the
   control that closes them.
9. **`Referrer-Policy: no-referrer` + `frame-ancestors 'none'`** — no referrer leakage,
   no embedding.
10. **Immutable storage + server-side re-validation at publish** — a published document
    cannot be tampered with after validation (PRD §56).

Out of scope for this node but required by the Tech Spec: rate limiting, size bounds,
abuse controls (Agent F), and the app origin's own hardening.

## Browser differences and caveats

Reliable, high-confidence knowledge; the live matrix is an open question (O1).

- **Chrome/Edge (Chromium)**
  - Full CSP3 support in meta, including hash sources; `'unsafe-inline'` is ignored for
    scripts when a hash/nonce is present.
  - Meta CSP outside `<head>` is ignored; multiple meta policies are additive per spec
    (all enforced) — some older engines honored only the first, so ship exactly one.
  - `file://`: clipboard API works with a synchronous gesture; `blob:` images and
    downloads work; `localStorage` is engine-dependent on `file://` (per-file
    partitioning in Chromium, historically shared in Firefox) — the runtime doesn't use
    it, so the policy does not depend on this behavior.
  - Sandboxed top-level documents block downloads without `allow-downloads` and lose
    secure-context status (the basis for rejecting `sandbox`).
  - `a[ping]` is governed by `connect-src`; with `connect-src 'none'` it is blocked.
- **Safari (macOS and iOS)**
  - Meta CSP is supported, including hashes, but WebKit has historically been sensitive
    to meta placement and to documents with a lot of preceding content — keep the meta as
    the very first element in `<head>` and verify live (O1/O9).
  - `<dialog>` requires Safari 15.4+; on older iOS the chrome degrades (runtime still
    renders, or the `<noscript>` fallback appears) — acceptable and worth an explicit
    support floor in the Tech Spec.
  - Clipboard API on `file://` has historically been limited in WebKit; `chrome.js`'s
    `execCommand` fallback exists precisely for this. Verify both paths on current iOS.
- **Firefox**
  - Meta CSP and hash sources are supported; fetch/modules are blocked on `file://`
    (CORS null origin) — the runtime relies on neither.
  - `file://` is treated as a potentially trustworthy origin; clipboard needs a gesture
    and permission. Older Firefox ignored `upgrade-insecure-requests` in meta — we do not
    use that directive, so no impact.
  - Historically Firefox did not apply `connect-src` to `a[ping]`; the residual is
    negligible (a ping leaks only a URL with no document data) and content validation
    (Agent B) strips `ping` attributes.
- **All engines**
  - The CSP hash is computed over the script element's parsed text content: LF
    normalization and entity decoding by the HTML parser mean the build must emit the
    runtime verbatim with LF line endings (see runtime integrity strategy).
  - Header-only directives (`frame-ancestors`, `sandbox`, `report-*`) are silently
    ignored in meta — never design local expectations around them.
  - `javascript:` URLs and inline event handlers are blocked only when `script-src` has
    no `'unsafe-inline'`; the hash-only policy guarantees that.
  - SVG embedded as an image (`<img src="data:image/svg+xml,...">`) does not execute
    scripts in any modern engine, but SVG-as-image external-reference behavior should be
    treated as an asset-validation responsibility (Agent B/C), not a CSP one.

## Assumptions and open questions requiring human/live verification

These must be verified against real artifacts and real browsers before the Tech Spec
freezes the policy:

- **O1 — Live matrix on `file://`.** Open a built artifact with the hash-only meta CSP
  in current Chrome, Edge, Firefox, Safari desktop, and iOS Safari. Verify: runtime
  mounts; Copy (primary clipboard and `execCommand` fallback), View, Download, theme,
  TOC and lightbox work; an injected `<script>`, `onclick=` handler, and `javascript:`
  URL are all blocked; `data:`/`blob:` images render.
- **O2 — Runtime surface.** Confirm the shipped runtime fragments contain no
  `eval`/`new Function`/`Function(...)` (then `'unsafe-eval'` is never needed) and no
  `innerHTML`/`insertAdjacentHTML` with author-derived strings (this determines whether
  Chrome-only Trusted Types hardening is feasible later). An audit assertion should
  enforce this.
- **O3 — Hash stability.** Confirm the build pipeline emits the runtime script
  byte-identically (LF, unescaped) and that the SHA-256 computed by the build matches
  what browsers hash, end to end. The existing reproducibility/drift checks
  (`runtime/build.mjs check`, SPEC §17) are the guardrail.
- **O4 — `fonts.url` under hosting.** Decide whether the hosting service accepts
  documents with external font origins and, if so, how `font-src` is extended at publish
  (allowlist the declared origin) versus rejecting such documents. Keep
  `data-mdhtml-portable="false"` semantics consistent.
- **O5 — Reporting.** Decide whether staging uses `Content-Security-Policy-Report-Only`
  and whether any production reporting endpoint exists. Default recommendation: none —
  no telemetry by design.
- **O6 — `sandbox`.** Rejected in this research (clipboard/secure-context and download
  breakage). Revisit only if the chrome is adapted; otherwise keep rejected.
- **O7 — SVG and asset types.** Confirm every engine blocks external-reference loads
  inside embedded SVG-as-image and that blob-hydrated images render under `file://` in
  all target browsers. Embedded-SVG restrictions are the sanitizer/asset validator's job
  (Agents B/C), not CSP's.
- **O8 — Email clients.** Confirm Gmail, Outlook, and Apple Mail treat attached
  `.md.html` files as non-executable attachments (they are downloaded and opened via
  `file://`, never rendered in the message body). Safety-by-default must not depend on
  this behavior, but it is worth recording.
- **O9 — Safari meta placement.** Verify the "meta CSP as first element in `<head>`"
  rule on current macOS/iOS Safari and confirm `<dialog>` support floors (Safari 15.4+).
- **O10 — Meta + header combination.** Confirm that serving an artifact whose meta CSP
  is byte-identical to the header CSP behaves as expected (additive policies,
  intersection = header) on all targets.
- **O11 — Single executable script.** Add a fixture/audit assertion that
  `script#mdhtml-runtime` is the only executable script element in every generated
  document, including documents with optional fragments and components.
