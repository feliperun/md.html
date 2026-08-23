# mdhtml Safe-by-Default & Public Hosting — Tech Spec

Status: Ready for ADR ratification and implementation

Project: mdhtml

Governed by: `docs/prd/mdhtml-safety-hosting-prd.md` §47

Inputs: PRD §5–17 and §18–59, `SPEC.md`, `docs/ARCHITECTURE.md`, `docs/ABSTRACTIONS.md`, ADRs 0001–0005, and research briefs `a-architecture-map.md`, `b-html-security.md`, `c-css-security.md`, `d-browser-security.md`, `e-hosting.md`, and `f-abuse.md`.

## Constraint binding

The eight critical product constraints in PRD §59 bind this pipeline as follows. Constraint 2 (exact byte-for-byte source recovery) and Constraint 3 (presentation freedom) are the two most direct constraints on the security design, and they are satisfied by making every safe-mode guard **validate and reject, never silently rewrite**, so the canonical `#mdhtml-source` bytes are never altered; the CSS guard parses and re-serializes only derived author CSS, not source, and keeps a broad visual allowlist. Constraint 1 (Markdown remains canonical) is preserved by server-side Option B building from source and content-addressing the source hash. Constraint 4 (no arbitrary executable code by default) is enforced by hash-pinned runtime CSP plus HTML/CSS/URL guards. Constraint 5 (safe everywhere) is met because the generated artifact carries its own meta CSP and validation result, independent of the hosting provider. Constraint 6 (agent-native) is met by stable diagnostics, JSON audit output, and a small structured API. Constraint 7 (hosting remains optional) is met because hosting is an add-on consumer of an already-safe portable artifact. Constraint 8 (simplicity) is met by one shared hosting application, one HTML parser family, one CSS parser, one storage provider, and no per-document deployment.

## Current architecture analysis

The current system has the following shape:

- `doc.md` → `mdhtml build` → `doc.md.html` → browser `file://` → runtime renders `#mdhtml-source`; `mdhtml extract` returns the Markdown byte-for-byte.
- The artifact embeds exactly one `script#mdhtml-source[type="text/markdown"]`, asset blocks (`application/octet-stream`), token/theme/user styles, and the selected classic runtime fragments.
- The Rust CLI is std-only (`crates/mdhtml/`), performs no Markdown rendering, and has no HTML parser today; the browser runtime (`runtime/src/`) is the sole HTML generator and escapes text and attributes.
- Raw HTML in Markdown is escaped by default and there is no raw-HTML opt-in in v1.0.
- Runtime embedding is governed by a committed fragment manifest (`mdhtml/manifest/1.0`) with per-fragment size and SHA-256; font embedding is governed by `fonts/catalog.json`.
- The existing CSP is meta-only and uses `script-src 'unsafe-inline'` and `style-src 'unsafe-inline'`.

Known gaps that this Tech Spec must close (from research brief A):

- Markdown link destinations are escaped but not scheme-validated, so `javascript:` links are currently possible.
- Local `.theme.css` and front-matter `tokens` are inlined without structural CSS validation.
- `fonts.url` flows into the CSP with no scheme/host/character validation.
- Asset path handling is asymmetric between build and extract; `extract` rejects `..`, build is looser.
- There is no source digest in the artifact and no runtime-integrity verification in `mdhtml check`.
- The origin inventory in `check` does not cover forms, iframes, objects, embeds, prefetch/preload, media, or SVG references.
- Embedded SVG is allowed as an image MIME with no script/external-reference validation.
- Front-matter `url`/`cover` values are concatenated and escaped but not structurally validated.

## Relevant SPEC.md constraints

- FMT-01: exactly one `#mdhtml-source[type="text/markdown"]`; everything else is derived and must never become source.
- FMT-02: `</script` is forbidden in the canonical source; `build` rejects it.
- FMT-03: a portable document must use only inline `data:`/`blob:` subresources and declare `data-mdhtml-portable="true"`; `fonts.url` is the only non-portable relaxation.
- FMT-04: `<noscript>` must keep the canonical source readable without JavaScript.
- FMT-05: title is required; metadata is derived from front matter.
- PARSE-01/PARSE-02: the YAML subset and Markdown surface are closed; hostile text is escaped; raw HTML is not enabled by default.
- CLI-03: `extract` restores the source byte-for-byte and must reject unsafe asset paths, collisions, and overwrites.
- §16 Diagnostics: stable `E-`/`W-`/`I-<REQ-ID>` codes are public contract; implementations may add codes but must not change the enumerated set.
- §17 Runtime fragment manifest: `core`, `copy`, `toc`, `lightbox` are selected from closed evidence; builds are reproducible.
- §18 Byte budgets: content/runtime/fonts/images are reported; release CLI binary ≤ 3 MiB (raised from the v1.0 600 KiB ceiling by ADR 0019); fonts are OFL/Apache, `wght` only, no `opsz`, never re-subsetted.
- ADR 0004: std-only CLI and dependency-free runtime are frozen choices; adding a third-party crate requires a superseding ADR.
- ADR 0005: the four-fragment runtime lifecycle boundaries are frozen.

## Threat model

Adversaries and failure modes:

- **Malicious author**: crafts Markdown, front matter, local theme CSS, or embedded assets to execute code, exfiltrate data, spoof chrome, or trigger network requests.
- **Compromised client**: uploads arbitrary HTML/artifact bytes and claims mdhtml provenance.
- **Hand-editor**: modifies a built artifact after validation, including the runtime or CSP.
- **Viewer**: receives a document in a context where the document is embedded or where its origin has state.
- **Abusive publisher**: uses anonymous hosting for phishing, spam, malware instructions, illegal content, SEO abuse, or mass publishing.
- **Future browser feature**: a previously benign construct becomes executable or network-capable.

Explicitly out of scope as threats by design: arbitrary author JavaScript, forms/data exfiltration from the document, popups, auto-downloads, iframe injection, and same-origin access to admin state. These are blocked by the pipeline and by origin isolation.

## Trust boundaries

- **Trusted**: the canonical Rust toolchain, committed runtime fragments, built-in themes, font catalog, and the generated document skeleton. These are integrity-pinned and not subject to author policy.
- **Untrusted**: canonical source, front matter, local `.theme.css`, `tokens`, linked URLs, `fonts.url`, metadata URLs, and embedded assets including SVG.
- **Boundary 1 — build**: the Rust CLI validates untrusted author inputs against the policy and emits only a verified artifact or an error.
- **Boundary 2 — runtime**: the browser executes exactly one hash-pinned runtime script; author content is data only.
- **Boundary 3 — hosting publish**: the server re-runs the canonical build/audit and never trusts client validation.
- **Boundary 4 — hosting delivery**: user documents live on a dedicated registrable origin and are served as immutable static bytes with no access to admin credentials.

## HTML attack surface

Surfaces:

- Markdown link/reference destinations (`[text](url)`, `[text][id]`).
- Heading `{#id}` overrides and section-bound `class` values.
- Front-matter `url`, `cover`, and any future author-controlled metadata.
- Embedded SVG image assets.
- Any future raw-HTML opt-in.

Controls:

- Use `html5ever` as the single HTML parser and a validation-only allowlist guard.
- Reject, never re-serialize: a violation fails the build and leaves the artifact unwritten.
- Allowed renderer element set stays fixed; deny `script`, `style`, `iframe`, `object`, `embed`, `form`, `svg`, `math`, `template`, `noscript`, `base`, `meta`, `link`, and custom elements.
- Allow only safe attributes; deny all `on*` handlers, `style`, `srcdoc`, `sandbox`, `allow`, `ping`, and URL-bearing fetch attributes.
- URI schemes allow `http`, `https`, `mailto`, `tel`, relative, and fragment-only; deny `javascript`, `vbscript`, `data` in `href`, `file`, `blob` in `href`, and unknown schemes.
- Restrict `{#id}` to `[A-Za-z0-9_-]`; validate section/class tokens against the existing CSS-identifier contract.

Decision: **use `html5ever` directly with an in-repo reject-don't-mutate policy; ammonia is the documented future cleaner, not the engine.** Reason: validation preserves deterministic bytes and avoids the mutation-XSS re-serialization path. ADR: #2 (HTML sanitizer selection), with #1 (security validation architecture) recording the policy module boundary.

## CSS attack surface

Surfaces:

- Local `.theme.css` and front-matter `tokens`.
- `url()` in backgrounds, borders, cursors, masks, clip paths, list styles, `content`, and future URL-bearing functions.
- `@import`, author `@font-face`, unknown at-rules.
- Selectors that target hosting chrome, `:visited`, state-probing pseudo-classes, `!important`, fixed full-viewport overlays, and timing/scroll probes.

Controls:

- Parse author CSS with `lightningcss` (`error_recovery: false`) and walk the typed AST.
- Drop `@import`, `@namespace`, unknown at-rules, and network `url()`; allow only `data:`, embedded-asset-map references, and runtime `blob:` for asset-map lazy images.
- Allow author `@font-face` only when it resolves to an embedded catalog font; keep `fonts.url` as a document-level non-portable opt-in, not a sanitizer allowance.
- Preserve visual/layout properties, `@media`, `@container`, `@supports`, `@layer`, `@scope`, `@keyframes`, `@page`, and `@counter-style` without external symbols.
- Deny full-viewport chrome-covering overlay patterns; deny `!important` in hosted mode; keep chrome outside the document subtree.
- Re-serialize with fixed `PrinterOptions` for byte-stable derived CSS. Source bytes are untouched.

Decision: **depend directly on `lightningcss` and implement a small in-repo, fail-closed policy module; do not depend on `css-sanitizer` for v1.** Reason: full typed AST fidelity with a stable serialization path and no 0.x policy API churn. ADR: #3 (CSS sanitizer/parser selection).

## Runtime attack surface

- The only executable script is `script#mdhtml-runtime`; `#mdhtml-source` and asset blocks are non-executable script types.
- `mdhtml build` computes the SHA-256 of the selected runtime bytes and emits `script-src 'sha256-<RUNTIME_HASH>'`, removing `'unsafe-inline'` for scripts.
- The runtime remains dependency-free classic JS with no `type="module"`, no `fetch`, no `pushState`, no `eval`/`new Function`, and no author-derived `innerHTML` interpolation.
- `audit` recomputes the embedded runtime hash and compares it to the manifest and CSP value.
- If the runtime is modified by hand, CSP fails closed and the document degrades to the `<noscript>` fallback.
- SRI has no direct application because the portable artifact has no external resources; it is retained only for a future externalized-resource decision.

Decision: **CSP script hashes authorize the exact runtime; no nonce, no `'unsafe-eval'`, no `'unsafe-inline'` for scripts.** Reason: a static hash simultaneously authorizes and integrity-pins the only executable surface. ADR: #5 (CSP strategy).

## Resource/network attack surface

Inventory of network-capable mechanisms and their control:

- Scripts: none except the hash-pinned runtime; `script-src` blocks all other scripts.
- Images: `data:`/`blob:` only via asset embedding; `img-src data: blob:`.
- Fonts: embedded catalog `data:` only; `font-src data:`; `fonts.url` is the explicit non-portable exception.
- CSS URLs and `@import`: dropped by the CSS policy and closed by CSP.
- Iframes/objects/embeds: denied by HTML policy and `frame-src 'none'` / `object-src 'none'`.
- Forms: denied by HTML policy and `form-action 'none'`.
- Media: embedded `data:`/`blob:` only; `media-src data: blob:`.
- SVG references/external loads: asset validator rejects SVG containing script or external references.
- Prefetch/preload/redirect metadata: `ping` denied; `base-uri 'none'`; `connect-src 'none'`; no `<base>`.
- Browser metadata/URLs: `url`/`cover` are structurally validated as `http`/`https` only.

Safe mode therefore produces zero unexpected network requests; `check`/`audit` compute the portability verdict from the validated content.

## Sanitizer library evaluation

HTML candidates:

- `html5ever`: chosen; mature WHATWG parser, no policy layer, validation-only fits reject-don't-mutate.
- `ammonia`: rejected as the engine because cleaning re-serializes and its historical mutation-XSS advisories live in that path; retained as the future cleaner if a cleaning mode is ever added.
- `html5gum`, `lol_html`, `kuchiki`, `gumbo`: rejected as tokenizer-only, rewriting-focused, or unmaintained.
- Regex sanitizers: prohibited by PRD §8.

CSS candidates:

- `lightningcss`: chosen; production-grade typed AST parse/serialize, broad modern CSS surface.
- `css-sanitizer`: rejected for v1; young 0.x API, fail-open default trait, low adoption.
- `cssparser` + `selectors` raw primitives: fallback only if `lightningcss` violates the binary budget.
- Regex/hand-rolled: rejected.

Dependency constraints: exact version pinning after `cargo audit`, RUSTSEC verification, Sentrux gate, `wasm32` compile check, and release binary-size measurement are mandatory before these crates land. These verifications are open questions until live network/pricing access is available.

## Proposed security pipeline

Build order:

1. `FMT-02` script-termination gate.
2. Front matter and analysis validation (existing errors).
3. Asset ingestion validation: safe relative paths, closed MIME table, base64 validation, and SVG structural policy.
4. HTML/URL policy walk on the renderer-serialized output and author-controlled attributes.
5. CSS parse + policy + deterministic re-serialization of author CSS.
6. Runtime manifest integrity check and runtime hash computation.
7. CSP assembly (`meta` hash policy; `fonts.url` only as explicit non-portable relaxation).
8. Deterministic assembly and atomic write.
9. Safe-mode attestation in the artifact (`data-mdhtml-safe="true"` or equivalent TBD) without storing secrets.

`--unsafe` profile:

- Disables all content-security guards (HTML, CSS, URL, resource) while keeping format, toolchain, and asset-integrity validations.
- Marks the artifact unsafe and is rejected by official hosting.
- Selective per-guard disabling is deferred; the MVP supports one profile, not a guard matrix.

Hosting path repeats steps 1–8 with the pinned canonical WASM toolchain and independently audits before storage.

## CLI changes

- `mdhtml build <in.md> [-o out] [--watch] [--no-fonts] [--unsafe]`: adds safe-by-default validation, runtime hash CSP, and the unsafe escape hatch.
- `mdhtml audit <file.md.html> [--json]`: validates structure, source integrity, HTML/CSS/runtime policy, external resources, and returns deterministic exit codes.
- `mdhtml publish <source>`: runs local build/audit, uploads source plus referenced assets, prints the public URL; never uploads `--unsafe` artifacts.
- Diagnostics: new stable `E-MDHSEC-*` codes with category, severity, source location where available, offending construct, explanation, and remediation.
- `--json` audit schema: `{ "safe": bool, "specVersion": "1.0", "sourceIntegrity": bool, "html": "pass", "css": "pass", "runtime": "pass", "externalResources": [] }`.

## API contract

`POST /v1/documents`

- Content type: `multipart/form-data`.
- Required field: `source` — the UTF-8 Markdown file.
- Optional repeated field: `asset` — referenced local assets; each multipart `filename` is the relative path (e.g. `images/photo.png`) and must satisfy the extraction-safe path predicate.
- Server: rate limit → size check → rebuild in temp directory with pinned toolchain → audit → hash → store → create ID.

Success response:

```json
{
  "id": "H7zPm",
  "url": "https://<docs-domain>/H7zPm",
  "sha256": "<sha256(canonical source)>",
  "mdhtmlVersion": "1.0"
}
```

Error response is structured and self-sufficient:

```json
{
  "error": {
    "code": "MDHSEC012",
    "message": "Unsafe URI scheme",
    "line": 87,
    "column": 14
  }
}
```

Authentication is not required for MVP anonymous publishing.

Decision: **upload model Option B — source plus assets, server-side build.** Reason: only a server-side canonical build can prove the artifact was produced by mdhtml. ADR: #7 (source vs artifact upload).

## Hosting architecture

- One shared Vercel project; publishing never creates a per-document deployment.
- Publish API runs in a Vercel Function and invokes the pinned CLI compiled to `wasm32`.
- Immutable artifacts are stored in Vercel Blob and served through the docs origin; raw public Blob URLs are not used because Blob forces HTML `Content-Disposition: attachment`.
- `/d/<toolchain>/<sha256>` is the content-addressed long URL; `/<shortId>` resolves once via a cached 308.
- Steady-state views are CDN-served with zero server-side compute.

Decision: **Vercel as the MVP provider, single shared application, Pro as the realistic launch floor after beta.** Reason: it matches the recommended zero-per-document-deployment architecture with low operational cost at the expected scale. ADR: #6 (hosting architecture) and #12 (storage provider).

## Storage model

Private Vercel Blob store with three key spaces:

```
sources/{sha256(source)}        canonical Markdown
docs/{toolchainId}/{sha256}     built .md.html
ids/{publicId}                  { sha256, toolchainId, createdAt }
```

- `sha256` is the SHA-256 of the canonical source.
- `toolchainId` pins the toolchain binary/build configuration so old IDs keep serving the exact bytes they were published with.
- Identical sources deduplicate; re-publishing the same source and toolchain returns the existing object.

Decision: **content-address by canonical source hash.** Reason: it gives deduplication, integrity verification, and hash-level takedown for free. ADR: #8 (content addressing).

## Caching model

- Content objects: `Cache-Control: public, max-age=31536000, immutable, s-maxage=31536000`.
- Short-ID resolution: `Cache-Control: public, max-age=3600, s-maxage=86400`; use `stale-while-revalidate`/`stale-if-error` for recoverability.
- Takedowns must delete the ID object and purge CDN content; TTLs bound propagation latency.

## ID generation

Decision: **12-character random base64url NanoID-style ID, server-side CSPRNG, no sequential counter.** Reason: 72 bits of unguessable entropy keeps IDs short, durable, and enumeration-resistant. ADR: #9 (public ID generation).

## Rate limiting

Starting limits:

- Source upload: 2 MiB.
- Built artifact: 8 MiB.
- Publish: 10/min, 50/hour, 200/day per IP; IPv6 counted per /64; 2 concurrent in-flight publishes.
- Lifetime storage per IP: optional 500 MiB backstop.
- Views are not rate-limited at MVP; CDN caching is the view-side control.

## Abuse controls

- Indexing: all anonymous documents default to `noindex, nofollow`; served with `X-Robots-Tag: noindex, nofollow, noarchive`, matching meta robots, and `robots.txt` `Disallow: /`.
- Public-by-default privacy stance with no implied secrecy from the URL.
- Abuse reports on the admin origin, rate-limited, with a private review queue.
- Takedown: deny check → delete object → purge CDN → append source hash to deny list → log. Hash deny lists prevent republishing the same source.
- `fonts.url` documents are rejected by hosting at MVP because they leak viewer IPs and complicate per-document CSP.

Decision: **anonymous, immutable, public, no-custom-slug publishing with report-driven takedown and hash deny lists.** Reason: frictionless publishing while closing the highest-probability abuse classes. ADR: #10 (anonymous publishing) and #13 (abuse/takedown model).

## CSP

Local artifact meta CSP (first element in `<head>`):

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

Hosted headers add:

```http
Content-Security-Policy: <same policy>; frame-ancestors 'none'
X-Content-Type-Options: nosniff
Referrer-Policy: no-referrer
X-Frame-Options: DENY
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Resource-Policy: same-origin
Origin-Agent-Cluster: ?1
Permissions-Policy: camera=(), microphone=(), geolocation=(), payment=(), usb=(), serial=(), hid=()
Strict-Transport-Security: max-age=63072000; includeSubDomains; preload
```

`sandbox` is rejected because it breaks clipboard/secure-context/download behavior in top-level documents. `report-to`/`report-uri` are absent by design.

## Isolated-origin strategy

Decision: **use a separate registrable domain for user content (`docs.example`) from the app/API/admin origin (`mdhtml.example`).** Reason: it shares no cookies, storage, or same-origin API access with privileged state. ADR: #11 (isolated user-content origin).

Defense in depth:

- App cookies use `__Host-`, `Secure`, `SameSite=Strict`, never `Domain=`-scoped.
- Publish API returns no CORS allowlist for the docs origin.
- COOP/CORP/`Origin-Agent-Cluster`/`Permissions-Policy` isolate the browsing context.

## Test strategy

- Dedicated `fixtures/security/` suites for HTML, CSS, SVG, URLs, malformed markup, mutation-XSS, runtime, and external resources.
- Every fixture records input, expected result, and expected diagnostic.
- Differential tests ensure Rust validation and browser-rendered DOM security interpretation do not diverge.
- Deterministic E2E over `file://` verifies the hash-only CSP, clipboard fallback, TOC, lightbox, and `<noscript>` degradation.
- Runtime surface audit asserts no `eval`/`new Function` and no author-derived `innerHTML` insertion.

## Fuzzing strategy

- Use `cargo-fuzz` on the HTML validator, CSS policy, front matter, extraction, and build→audit pipeline.
- Invariants to fuzz:
  - `extract(build(source)) == source`.
  - `safe build` → `audit` always passes.
  - user-controlled input never creates an unauthorized executable node.
  - CSS sanitizer output contains no network-capable `url()`/`@import`.

## Migration/compatibility

- Safe-by-default is treated as an **additive security profile over the frozen v1.0 contract**, not a rewrite of it; valid v1.0 documents continue to build where they do not violate the new policy.
- The existing `script-src 'unsafe-inline'` baseline is superseded by the hash-only policy; `'unsafe-inline'` remains for styles only.
- **This CSP change is not additive — it is a format revision, v1.1, of the FMT-03 contract.** The root's `data-mdhtml` attribute stays `"1.0"` (frozen by SPEC §1 and audited by `E-MDHSEC-017`), so an old artifact is distinguished not by a version string but by the CSP it carries (SPEC §5 now lists the hash policy as canonical). The profile is additive for *sources* — every canonical v1.0 `.md` source rebuilds unchanged under v1.1, because the runtime hash is derived, not authored — but a *built artifact* from the v1.0 toolchain (with `script-src 'unsafe-inline'`) does not satisfy the v1.1 contract. `mdhtml check` on such an artifact fails deterministically with `E-FMT-03` (declared CSP ≠ recomputed canonical CSP; the hash is recomputed from the embedded runtime, never trusted from the meta), and `mdhtml audit` fails with `E-MDHSEC-016` (contradictory CSP) — a clear rejection, never a crash or a false pass. Migration is therefore "rebuild the artifact from its canonical source", not "edit the document": byte-exact `extract` guarantees the source is always recoverable from the old artifact.
- `{#id}` tightening to `[A-Za-z0-9_-]` is a small frozen-surface behavior change and requires a fixture update.
- No existing `Diagnostic` code changes; new `E-MDHSEC-*` codes are additive.
- `extract` and byte-exact round-trip invariants are preserved by the reject-don't-mutate design.
- `--unsafe` is opt-in, marks the artifact, and is rejected by hosting.

## Deployment plan

1. Land the Rust WASM build and security pipeline in CI with size/audit gates.
2. Create the two domains (`mdhtml.example`, `docs.example`) and configure headers.
3. Deploy the shared Vercel project with Publish API, Blob storage, and CDN routes.
4. Wire observability: publish counts, validation failures, storage/egress, rate-limit events, and takedown counters.
5. Enable hash deny-list and takedown tooling on the admin origin before public launch.

## Rollout strategy

1. Roll out safe-by-default `build` and `audit` first, without hosting.
2. Run the adversarial security review gate and turn every finding into a fixture before any public hosting.
3. Beta the hosting service on Hobby with explicit beta limits and alerts; upgrade to Pro before opening to sustained third-party/anonymous traffic.
4. Keep anonymous publishing, immutable IDs, `noindex`, and report-driven takedown as launch defaults.
5. Do not open public indexing, custom slugs, private documents, or `fonts.url` hosting until separate decisions are made.

## Open questions

- Verify live crate versions, maintenance, and RUSTSEC status for `html5ever` and `lightningcss` before pinning (the `url` crate is not a dependency: ADR 0007 records the in-repo RFC 3986 scheme-splitting decision).
- Native release binary-size impact is measured and the budget decided (ADR 0019: 3 MiB, dependencies-only measurement 1,882,704 bytes); `wasm32` size and compatibility remain unmeasured.
- ~~Verify `url` crate parsing parity with browser attribute parsing~~ — superseded by ADR 0007: scheme validation is the in-repo RFC 3986 splitter; its parity is pinned by `fixtures/security/url-*.json` instead.
- Decide runtime parity for hand-edited documents: source-side policy only for MVP versus an embedded client-side policy walk.
- Confirm whether any raw-HTML mode exists beyond `--unsafe`, and if so whether it rejects or cleans via ammonia.
- Confirm audit scope is source/artifact policy only, with browser-based rendered-DOM verification only in E2E.
- Confirm `{#id}` restriction and its fixture migration.
- Confirm hosting chrome is outside the document subtree; otherwise enforce reserved-host-hook selector bans.
- Confirm exact `!important`/overlay local-versus-hosted policy split.
- Decide where SVG structural validation lives: asset pipeline versus HTML/CSS guard.
- Validate browser matrix O1–O11 from research D, especially Safari meta placement and hash stability.
- Re-verify Vercel Edge Request/egress accounting, plan binding, region choice (`iad1` vs `gru1`), and Blob same-region egress.
- Confirm case-sensitive 12-char IDs remain acceptable versus the case-insensitive 14-char variant.
- Decide `fonts.url` hosting rejection versus allowlist with privacy notice after launch telemetry.
- Determine whether proactive illegal-content detection (PhotoDNA/NCMEC-style) is in scope for MVP.
- Decide 404 vs 410 and deny-list expiry for non-illegal takedowns.
- Decide future indexing opt-in reputation signal if an authenticated tier arrives.
- Decide progressive enforcement thresholds (captcha/proof-of-work) if anonymous abuse spikes.

## Phases and Tasks

### Phase 1 — Ratify ADRs and freeze shared contracts

- Write the 13 required ADRs listed in PRD §46, one per decision above, without editing any active ADR. Primary files: `docs/adr/`, `docs/ARCHITECTURE.md`. Exit: each ADR exists, active decisions are reflected in `docs/ARCHITECTURE.md`, and Sentrux gates pass.
- Freeze the diagnostic code list and `--json` audit schema. Primary files: `SPEC.md` addendum/planning note, `docs/tech-spec/mdhtml-safety-hosting-tech-spec.md`. Exit: every implementation task can reference stable codes/schema.
- Freeze the security fixture contract (`input/expected/diagnostic`). Primary files: `fixtures/security/README.md`, this spec. Exit: fixtures have a deterministic harness contract.

### Phase 2 — Core security pipeline

- Add `html5ever`-based validation-only HTML/URL guard. Primary files: `crates/mdhtml/src/security/html/`. Exit: all HTML/URL fixtures pass and unsafe builds fail without output.
- Add `lightningcss`-based CSS policy and deterministic author-CSS re-serialization. Primary files: `crates/mdhtml/src/security/css/`. Exit: all CSS fixtures pass; portable docs have zero network `url()`.
- Add asset validation incl. SVG script/external-reference rejection and symmetric safe-path handling. Primary files: `crates/mdhtml/src/build/assets.rs`, `crates/mdhtml/src/extract/`. Exit: SVG and path fixtures pass; extract round-trip unchanged.
- Compute runtime hash and emit hash-only CSP. Primary files: `crates/mdhtml/src/build/mod.rs`, runtime manifest code. Exit: generated artifacts carry the proposed meta CSP and `runtime/build.mjs check` stays green.

### Phase 3 — CLI audit, diagnostics, and unsafe mode

- Implement `mdhtml audit [--json]`. Primary files: `crates/mdhtml/src/commands/audit.rs`, `crates/mdhtml/src/cli.rs`. Exit: artifact fixtures return deterministic pass/fail and JSON matches schema.
- Add `E-MDHSEC-*` diagnostics with source locations where available. Primary files: `crates/mdhtml/src/diagnostic*`, security modules. Exit: malformed/unsafe fixtures assert exact codes and caret locations.
- Implement `--unsafe` profile and artifact marking. Primary files: `crates/mdhtml/src/cli.rs`, `crates/mdhtml/src/build/mod.rs`. Exit: unsafe artifacts build only with explicit flag, are marked, and `audit` reports unsafe.
- Run deterministic build validation. Primary files: build/audit tests. Exit: same source/toolchain/config produces byte-identical artifact.

### Phase 4 — Hosting, API, and storage

- Publish the canonical toolchain as pinned WASM. Primary files: `crates/mdhtml/src/lib.rs`, CI. Exit: serverless host can invoke build/audit with no native binary.
- Implement `POST /v1/documents` with multipart source/assets, validation, and structured errors. Primary files: hosting project function. Exit: CLI, curl, and CI fixtures publish successfully; invalid docs return structured errors.
- Implement Blob storage layout and short-ID/long-URL resolution. Primary files: hosting function/storage modules. Exit: content deduplicates and short IDs resolve.
- Apply isolated origin, security headers, robots/indexing, and takedown controls. Primary files: platform config, admin function. Exit: hosted docs have no admin cookie/state access and takedown removes objects.

### Phase 5 — CLI publish and agent experience

- Implement `mdhtml publish <source>` and asset discovery/multipart upload. Primary files: `crates/mdhtml/src/commands/publish.rs`. Exit: `mdhtml publish document.md` returns a public URL.
- Update the official agent skill with build/audit/publish flow and security guidance. Primary files: `skills/mdhtml-author/`. Exit: agent can publish and fix diagnostics without undocumented API knowledge.
- Document hosting website and public privacy/takedown policy. Primary files: site/docs. Exit: user-facing copy explains creation, publish, inspection, and public-by-default status.

### Phase 6 — Security fixtures, fuzzing, and adversarial gate

- Populate the complete security fixture corpus. Primary files: `fixtures/security/`. Exit: PRD §15 fixture categories are all covered.
- Add cargo-fuzz targets for the invariants above. Primary files: `fuzz/`. Exit: fuzz targets run in CI and preserve invariants.
- Run the PRD §50 adversarial review and convert findings to regression fixtures. Primary files: `fixtures/security/`. Exit: no un-fixed critical/high finding before launch.

### Phase 7 — Launch readiness and rollout

- Wire observability and budget/abuse alerts. Primary files: hosting config. Exit: publish, storage, egress, rate-limit, and takedown metrics are visible.
- Run the launch checklist and security sign-off. Primary files: this spec + PRD definitions of done. Exit: PRD §54–57 definitions of done are met and CI is green.

## Addendum — Frozen security diagnostic codes (Phase 1 deliverable)

This addendum freezes the `E-MDHSEC-*` code list (Phase 1, "freeze the
diagnostic code list"). Codes are additive to the SPEC §16 registry, never
reused, and follow the `E-MDHSEC-NNN` shape (the PRD §14 display examples
`MDHSEC001`/`MDHSEC012` map to `E-MDHSEC-001`/`E-MDHSEC-012`). The
`--json` audit schema is the one in "CLI changes" above, unchanged.

| Code | Category | Meaning |
|------|----------|---------|
| `E-MDHSEC-001` | html | executable event handler (`on*` attribute) detected |
| `E-MDHSEC-002` | html | element outside the renderer's fixed element set |
| `E-MDHSEC-003` | html | denied attribute (`style`, `srcdoc`, `sandbox`, `allow`, `ping`, URL-bearing fetch attributes) |
| `E-MDHSEC-004` | html | heading `{#id}` override or section/class token outside the identifier contract |
| `E-MDHSEC-005` | url | front-matter `url`/`cover` is not an absolute `http`/`https` URL |
| `E-MDHSEC-006` | url | `fonts.url` origin invalid (not `https`, malformed host, or control characters) |
| `E-MDHSEC-007` | css | author CSS fails to parse (fail closed) |
| `E-MDHSEC-008` | css | denied at-rule (`@import`, `@namespace`, unknown at-rules) |
| `E-MDHSEC-009` | css | network `url()` denied |
| `E-MDHSEC-010` | css | denied CSS construct (external `@font-face`, hosted-mode `!important`/full-viewport overlay) |
| `E-MDHSEC-011` | svg | SVG contains script or other executable content |
| `E-MDHSEC-012` | url | unsafe URI scheme in a link/reference destination |
| `E-MDHSEC-013` | svg | SVG external reference (external `href`/`xlink:href`) |
| `E-MDHSEC-014` | path | unsafe asset path (`..`, absolute path, drive letter, backslash escape) |
| `E-MDHSEC-015` | runtime | runtime manifest integrity or runtime hash mismatch |
| `E-MDHSEC-016` | csp | CSP assembly violation (missing or contradictory policy) |
| `E-MDHSEC-017` | artifact | structure or mutation violation in a built artifact (audit) |
| `E-MDHSEC-018` | unsafe | artifact is marked unsafe (audit reports it; hosting rejects it) |
| `W-MDHSEC-019` | unsafe | `--unsafe` build warning: security guards disabled and artifact marked unsafe (CLI stderr, ADR 0009) |

Guard semantics note: wherever ADR 0006/0008 say a construct is "dropped",
the implementation rejects it with the corresponding code above — silent
removal of author content is prohibited by PRD §5 ("reject rather than
silently mutate"). Deterministic re-serialization applies only to author CSS
that already passed the policy, as a stable output format, never as a
mutation of rejected content. The security fixture contract is
`fixtures/security/README.md`.
