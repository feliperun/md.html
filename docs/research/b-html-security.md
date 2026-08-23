# Research B — HTML security: sanitizer/parser evaluation for mdhtml

> Node: research-b-html-security · PRD: `docs/prd/mdhtml-safety-hosting-prd.md` §4–8, 13–16, 47–51
> Scope: recommend, do not implement. Implementation is a later stream (PRD §49, Stream 1).
> **Verification status: this environment has no network access (DNS resolution blocked), so current library versions, release dates, maintenance status, and advisory IDs could not be verified against the web. Every claim about current library state below is an assumption from training knowledge and MUST be re-verified by a human with live access (crates.io, rustsec.org/RUSTSEC, GitHub) before the Tech Spec relies on it.** Claims verified from the repository's own files are marked *(verified)*.

## Context: what author-controlled HTML actually reaches the artifact today *(verified)*

The frozen v1.0 contract shapes how a guard can be introduced without breaking it:

- The artifact embeds the canonical source; the browser runtime renders it. The built `.md.html` contains no pre-rendered body HTML (SPEC.md FMT-01 skeleton: `<div id="mdhtml-app"></div>` is empty; the runtime fills it).
- The JS renderer is the only HTML generator and is safe by construction: `escapeText`/`escapeAttribute` in `runtime/src/render.js` escape all text and attribute values; the element set is closed (`p, a, img, br, code, pre, em, strong, del, sup, table, thead, tbody, tr, th, td, ul, ol, li, input[type=checkbox], blockquote, figure, figcaption, aside, div, span, details, summary, dl, dt, dd, meter, hr, section, h1–h6`).
- Raw HTML in Markdown is escaped by default and "never enabled unless explicitly configured" (SPEC.md §9, PARSE-02). There is no raw-HTML path in v1.0 (`runtime/src/markdown.js` and `runtime/src/render.js` contain none).
- The Rust CLI does not render Markdown; its analysis layer (`crates/mdhtml/src/analysis/containers.rs`, `section_components.rs`) is a scanner/classifier over `ContainerEvidence` and body-shape evidence. It contains **no HTML parser**. A guard would therefore be the *first* HTML parser in the Rust pipeline, not a second one.
- Author-controlled values that reach the DOM today: link targets (`[x](url)`, reference definitions), front matter `url`, heading `{#id}` overrides, bound section `class`, container arguments (rendered as escaped text), `language-*` classes, and image asset paths (`data-md-asset-path`, resolved to embedded `data:`/`blob:` only).
- Of these, **link URLs are the only live executable vector**: `renderInline` escapes `node.url` but does not validate its scheme, so `[x](javascript:alert(1))` ships as `<a href="javascript:alert(1)">`. The canonical CSP (SPEC.md FMT-03) includes `script-src 'unsafe-inline'` (the runtime needs it), and `javascript:` URL navigation is governed by `script-src`, so such a link can execute when clicked. This is a v1.0-relevant gap independent of any future raw-HTML feature.

Consequences for the guard design:

1. Safe-mode validation is primarily a **URL/resource and attribute-value guard at build time**, plus defense-in-depth parsing of rendered output — not a general "clean untrusted HTML" problem.
2. If a raw-HTML opt-in is ever added (PRD §6 `--unsafe`), that is the only path needing full sanitizer semantics, and it must be designed to avoid mutation XSS (see bypass classes below).
3. The guard must not become a second, incompatible HTML parser relative to containers/section components: today those are Markdown-shape validators with no HTML parse, so the guard introduces the *single* HTML parser, and it must be the *only* one.

## Candidate libraries

All candidates are Rust crates. "HTML5 conformance" means implementing the WHATWG parsing algorithm (tokenizer + tree construction, including the adoption agency algorithm and foreign-content rules) rather than regex or string rewriting. PRD §8 explicitly forbids regex-based sanitization.

| Crate | Maturity / role | Maintenance | HTML5 conformance | Known bypass history |
|---|---|---|---|---|
| **html5ever** | De-facto reference HTML5 parser for Rust (Servo lineage); parse-only, no policy layer | Historically active with regular releases *(assumption: verify current state)* | Full: tokenizer + tree builder, foreign-content and adoption-agency handling | A parser, not a sanitizer — no sanitizer policy to bypass; watch for spec-compliance parser bugs. No notable sanitizer-track CVEs on record *(assumption)* |
| **ammonia** | Mature allowlist sanitizer built on html5ever; parse → filter → re-serialize; used by mdBook (closest analog: author HTML inside Markdown books) | Historically active *(assumption: verify current state)* | Full (inherits html5ever); re-serializes with the html5ever serializer | Multiple historical mXSS advisories (e.g. a 2021 mutation-XSS fix in 3.1.0; later fixes involving SVG/MathML foreign content). All known XSS-class issues require explicitly allowing `svg`/`math` or raw-text elements — none are allowed by default. Exact advisory IDs/versions unverified *(assumption)* |
| **html5gum** | Tokenizer-only WHATWG fork of html5ever's tokenizer; no tree builder | Maintained, low churn *(assumption)* | Tokenizer only — no tree, no foreign-content integration points | n/a |
| **lol_html** | Cloudflare streaming HTML rewriter; selector-based rewriting | Active (Cloudflare-backed) *(assumption)* | Not a spec-conformant tree builder for validation purposes; a rewriting tool | Not a sanitizer; no policy layer to evaluate |
| **kuchiki / kuchikiki** | html5ever-based DOM crates | Dormant / low activity *(assumption)* | Full parser, but unmaintained | n/a — reject on maintenance alone |
| **gumbo / gumbo-rs** | Bindings to Google Gumbo parser | Unmaintained *(assumption)* | Older HTML5 snapshot, no longer maintained | Reject on maintenance alone |
| **regex-based sanitizers** | — | — | None | Prohibited by PRD §8 |

Assessment:

- **html5ever** is the stable, spec-conformant core; it is a parser, not a sanitizer, so the policy layer is ours.
- **ammonia** is the mature sanitizer the PRD names as primary candidate; its real value is its allowlist *policy model*, which we should adopt. The argument against using the ammonia crate as the engine is its **model**: ammonia *cleans* (parse → filter → re-serialize). mdhtml requires *reject, don't mutate* (PRD §5) and deterministic, byte-identical builds (PRD §52). Re-serialization changes bytes, and ammonia's historical mXSS bugs live precisely in that clean/serialize path.
- **html5gum, lol_html, kuchiki, gumbo** are rejected: tokenizer-only (no tree-construction fidelity), rewriting-focused, or unmaintained.
- If the Tech Spec ever adds a raw-HTML *cleaning* mode, ammonia remains the documented candidate — it shares html5ever, so no second parser family would be introduced.

## Recommended approach

**Use html5ever (pinned exact version, chosen after `cargo audit`) directly as the single HTML parser in the Rust CLI, wrapped in a validation-only, reject-don't-mutate allowlist guard. Do not use the ammonia crate as the engine; adopt its policy model (element/attribute/URL-scheme allowlists) as the blueprint.** Keep ammonia documented as the candidate *cleaner* if a future product decision adds a cleaning mode.

Design rules:

1. **Validate, never mutate.** Parse the string with `html5ever::parse_fragment` using a `<div>` context element (the runtime mounts rendered content into `div#mdhtml-app`, so a div context matches the browser's tree construction for that subtree), walk the tree against the allowlist policy, and reject the build with a diagnostic on the first violation. The shipped artifact contains the original bytes; output is deterministic (PRD §52).
2. **Validate exactly what the browser will parse.** The guard operates on the renderer's serialized output string and on any author-HTML fragment a future feature admits — the same bytes, the same fragment context. It never validates Markdown, and it never re-derives container/component structure from HTML: that stays in `crates/mdhtml/src/analysis/*` on scanner evidence. This keeps one HTML parser for validation and one (JS) generator for HTML, so no parser can disagree with the renderer.
3. **Deny the divergence-prone classes outright**, even though none are in the renderer's element set: `svg`, `math`, `template`, `noscript`, `noembed`, `noframes`, `xmp`, `plaintext`, `listing`, `iframe`, `object`, `embed`, `applet`, `form`, `select`, `style`, `script`, `base`, `meta`, `link`. This neutralizes the known mXSS families and the scripting-flag divergence: `<noscript>` content parses as markup only when scripting is enabled, so the guard must set `TreeBuilderOpts.scripting_enabled` explicitly to match the browser — and denies the element anyway, so the flag can never become a divergence point.
4. **URL policy on the parsed, entity-decoded value.** html5ever decodes character references before the guard sees attributes, so `jav&#x61;script:` is already `javascript:` at validation time. Scheme extraction goes through the WHATWG URL parser (the `url` crate — the reference Rust implementation of the URL Standard), which handles case-insensitivity, ASCII tab/newline stripping, and backslash-as-slash in special schemes. Never regex a raw attribute string.
5. **Reject, don't sanitize, in safe mode.** Safe `build` fails with a stable `E-MDHSEC-*` diagnostic (PRD §14 format). No silent removal, no fallback rewriting. Where the guard runs on toolchain-generated output, a violation is a programming error and must fail loudly.
6. **Raw-HTML mode (`--unsafe`, PRD §6)** — if it exists: still validate with the same parser and policy. Either reject wholesale or re-serialize through the guard; never ship raw author bytes based on a single parse without the mXSS fixture suite. `--unsafe` artifacts are flagged and rejected by hosting (PRD §6).
7. **Defense in depth (PRD §7).** The guard is one layer: build-time gate. The embedded CSP (`default-src 'none'`, SPEC.md FMT-03) is the browser-side enforcement backstop; runtime integrity (PRD §11) and the security fixture suite + fuzzing + differential tests (PRD §15, §16, §51) are the remaining layers.

## Policy: elements, attributes, URI schemes

The policy is a strict allowlist: anything absent is denied. It lives in one module (see integration) and is the single source of truth for `build`, `check`, and `audit`. Container/component wrappers are renderer-owned and fixed; this policy governs author-controlled values and any future author-HTML surface.

### Elements

**Base set — everything the renderer emits today, always allowed:**

`a, aside, blockquote, br, code, dd, del, details, div, dl, dt, em, figcaption, figure, h1, h2, h3, h4, h5, h6, hr, img, input, li, meter, ol, p, pre, section, span, strong, sup, summary, table, tbody, td, th, thead, tr, ul`

**Opt-in set — conservative additions only if the Tech Spec enables a raw-HTML mode:**

`abbr, address, article, b, bdi, bdo, caption, cite, col, colgroup, data, dfn, footer, header, hgroup, i, ins, kbd, main, mark, nav, q, rp, rt, ruby, s, samp, small, sub, time, u, var, wbr`

**Denied always (even in `--unsafe` unless the Tech Spec explicitly relaxes):**

`script, style, iframe, object, embed, applet, form, button, select, textarea, svg, math, template, slot, noscript, noembed, noframes, xmp, plaintext, listing, frame, frameset, base, meta, link, video, audio, source, track, canvas, portal, marquee, blink, dialog, input (except the renderer's checkbox), custom elements (any tag with a hyphen), XML declarations`

Rationale: every denied element is either executable, fetch-capable, a raw-text/scripting-flag divergence point, an mXSS integration point, or outside the renderer's closed vocabulary. Custom elements are denied unless a named, toolchain-registered component exists — the renderer emits none today.

### Attributes

**Global (allowed on any allowed element):**

- `id` — restricted to `[A-Za-z0-9_-]`; the renderer's slugified ids already satisfy this, but the `{#id}` override path must be restricted at the source (today it only lowercases and replaces whitespace).
- `class` — CSS identifier list, reusing the existing `CLASS_RE` contract from `render.js` (`[A-Za-z_][A-Za-z0-9_-]*`).
- `title`, `lang`, `dir`, `hidden`, `role`, `aria-*` (accessibility-safe).
- `data-*` — only the toolchain's own names: `data-md-section`, `data-md-asset-path`, `data-md-footnotes`. Author `data-*` denied in opt-in HTML.

**Element-specific:**

- `a[href]` — scheme-checked (below). `target` restricted to `_blank` and only with `rel="noopener noreferrer"` enforced by the guard; `download` denied.
- `img[src, alt, title, width, height, data-md-asset-path]` — `width`/`height` positive integers only; `src` (opt-in mode) restricted to `data:` image payloads or a relative path that `build` resolves to an embedded asset; `data-md-asset-path` must be a relative, `..`-free path (the existing extraction predicate, SPEC.md CLI-03, reused).
- `input[type=checkbox, checked, disabled]` — renderer-emitted only; `type` must be exactly `checkbox`; no `form*` attributes.
- `meter[min, max, value, low, high, optimum]` — finite numbers.
- `ol[start]` — positive integer.
- `td, th[align]` — `left|right|center|justify` only.
- `details[open]` — boolean.
- Structural wrappers (`table, section, figure, aside, div, span, blockquote, dl, ul, ol, li`) — no element-specific executable attributes.

**Denied on all elements:**

All `on*` event handlers (`onclick, onerror, onload, onpointer*, onanimation*, ontransition*, ontoggle, onmouseover, onfocus, onscroll`, …; attribute names are case-insensitive in HTML, so `OnClick` is also an event handler), `style`, `srcdoc`, `sandbox`, `allow`, `formaction`, `action`, `ping`, `srcset`, `sizes`, `poster`, `autofocus`, `contenteditable`, `tabindex`, `draggable`, `dropzone`, `usemap`, `ismap`, `referrerpolicy`, `http-equiv`, `charset`, `xmlns`, `xmlns:*`, `xlink:*`, `xml:*`, `slot`, `part`, `nonce`, `integrity`, `crossorigin`, `fetchpriority`, `loading`, `name` (legacy fragment anchors).

### URI schemes

Applied to every URL-bearing attribute value **after** parsing (entities decoded by html5ever; value parsed with the WHATWG `url` crate):

- **Allowed:** `http`, `https`, `mailto`, `tel`, relative references (resolved inside the document), and fragment-only (`#…`).
- **Denied:** `javascript`, `vbscript`, `data` (in `href` — `data:text/html` is a navigable document; `data:` image payloads are an asset-embedding mechanism, not an author URL), `file`, `blob` (in `href` — runtime-created blob URLs are opaque and cannot be validated), `about`, `chrome`, `filesystem`, `ftp`, `ws`, `wss`, `gopher`, `ms-*`, and **any scheme not on the allowlist** (default-deny).
- `data:` in `img[src]` (opt-in mode) restricted to safe raster types (`image/png`, `image/jpeg`, `image/gif`, `image/webp`); `data:image/svg+xml` denied (SVG can carry script).
- Empty, whitespace-only, or control-character-leading values fail validation.
- `audit` additionally reports every URL that would initiate a network request (PRD §10): safe artifacts must make zero unexpected external requests; the same check runs over the embedded asset table and the CSP declarations.

## Known bypass classes and how the recommendation addresses each

| Bypass class | Example vectors | How the recommendation addresses it |
|---|---|---|
| **Mutation XSS (mXSS)** — sanitizer output re-parses into executable markup | Attribute stripping changing parse context (e.g. ammonia's `annotation-xml` encoding-strip class); `<noscript>`/raw-text re-parse; comment-based mutations | Validation-only guard never re-serializes; shipped bytes are the renderer's own escaped output. All divergence-prone elements (`svg`, `math`, `template`, `noscript`, raw-text tags) are denied. Future raw-HTML mode must re-serialize through the guard (or reject), documented as the mXSS condition for that mode; fixture suite (PRD §15) carries historical mXSS patterns |
| **SVG/MathML foreign content** | `<svg onload=…>`, `<a xlink:href="javascript:…">`, `<foreignObject>`, `<svg><animate attributeName="href" to="javascript:…">`, `<math><annotation-xml encoding="text/html">`, `<svg><style>` | `svg`/`math` denied at element level; foreign-namespace attributes (`xlink:*`, `xmlns:*`) denied globally; this is the class behind ammonia's known XSS advisories, and the reason they cannot reach mdhtml |
| **Dangerous URI schemes** | `javascript:`, `vbscript:`, `data:text/html`, `file:`, entity-obfuscated `jav&#x61;script:`, control-character-padded schemes | html5ever decodes entities first; the `url` crate parses the value per the WHATWG URL Standard (case-insensitive schemes, tab/newline stripping, backslash handling); strict scheme allowlist with default-deny; no regex on raw strings |
| **Inline event handlers** | `onerror`, `onload`, `onpointerenter`, `onanimationstart`, `ontoggle` on any element; case-shifted `OnClick` | All `on*` attributes denied globally; the HTML spec's case-insensitive attribute handling makes this complete for current handler names, and default-deny covers future `on*` attributes |
| **Script / iframe / object / embed / applet / base / meta / link / form** | `<script src>`, `<iframe srcdoc>`, `<object data>`, `<embed src>`, `<base href>`, `<meta http-equiv=refresh>`, `<link rel=preload|stylesheet>`, `<form action>` | All denied at element level; CSP `default-src 'none'` + `frame-src`/`object-src` restrictions back this in the browser |
| **Raw-text and scripting-flag divergence** | `<noscript>` content parsed as markup only when scripting is enabled; `<plaintext>`, `<xmp>`, `<title>`, `<textarea>` RCDATA/raw-text contexts | All raw-text/RCDATA elements denied; html5ever's scripting-disabled parse cannot diverge from the browser on any element the guard allows |
| **HTML parsing edge cases** | Adoption agency algorithm (`<b><i></b></i>`), foster parenting (`<table>` mis-nesting), character references, null bytes, tag/attribute case normalization, mis-nested formatting elements | The guard uses the full tree builder, so its view is the browser's tree; the deny list removes the elements where tree-building quirks become security-relevant; fixtures cover malformed/adversarial input (PRD §15) |
| **Namespace confusion** | SVG `<a>` vs HTML `<a>` attribute semantics, `xmlns` injection, `annotation-xml` HTML integration points | `svg`/`math` and all `xmlns:*`/`xlink:*`/`xml:*` attributes denied; no foreign namespace can reach the allowlist |
| **Attribute smuggling** | `srcdoc`, `sandbox`, `allow`, `ping`, `formaction`, `srcset`, `poster`, `download`, `referrerpolicy`, `target` without `noopener` | Globally denied except the explicitly allowed `a[target=_blank]` with enforced `rel="noopener noreferrer"` |
| **CSS-in-HTML vectors** | `style` attribute with `url()`, `<style>` blocks, legacy `expression()`/`behavior:` | `style` attribute and `<style>` element denied in author HTML; CSS customization flows through the dedicated theme path handled by Agent C's CSS research; CSP restricts style sources |
| **Future browser features** | New `on*` attributes, new URL schemes, custom elements, new fetch-capable elements | Default-deny policy on elements/attributes/schemes is future-proof by construction; security review gate (PRD §50) turns each new finding into a regression fixture |

## Integration points in the existing Rust pipeline

- **New module:** `crates/mdhtml/src/security/` — e.g. `html.rs` (policy walk over the html5ever tree), `url.rs` (WHATWG scheme validation), `policy.rs` (the single allowlist source of truth), `skeleton.rs` (toolchain-owned artifact policy for `audit`). Invoked from the `build` flow (`crates/mdhtml/src/build`) after the front matter and analysis passes and before atomic write (SPEC.md CLI-01). Exact call-site names must be confirmed against the real build module during implementation — this node read only the listed files.
- **What safe-mode `build` validates today:** every author-controlled URL (inline link targets, reference-definition URLs, front matter `url`), `{#id}` charset, bound section `class` (reuse `CLASS_RE`), and — as defense in depth — a fragment parse of the renderer's serialized output wherever the Rust side serializes one. Raw HTML stays disabled (unchanged from SPEC PARSE-02), so no author HTML string reaches the artifact.
- **Future raw-HTML mode (`--unsafe`, PRD §6):** the same module validates the raw HTML fragment against the full allowlist and either rejects or re-serializes it (the only path where the guard serializes); `--unsafe` artifacts are flagged and rejected by hosting.
- **`check` and `audit` (PRD §13):** reuse the same policy walk on the built artifact, with a separate generated-skeleton policy for toolchain-owned parts (`#mdhtml-source` script, runtime script, `<style>` blocks, asset blocks, CSP meta, noscript fallback) so `audit` distinguishes "author content violates policy" from "artifact structure is valid".
- **Diagnostics:** new stable codes `E-MDHSEC-*` with category, severity, and source location where available (the SPEC §16 registry permits codes beyond the enumerated table; the Tech Spec must enumerate them). Example: `E-MDHSEC-001 unsafe URI scheme` with a caret pointing at the offending attribute (PRD §14 format). html5ever's tokenizer sink receives a line number per token, but the standard tree builder drops positions; carrying them onto tree nodes requires custom sink plumbing, whose feasibility must be validated early in Stream 1 (fallback: report the nearest tokenizer line at the first violation).
- **No second parser:** html5ever is the only HTML parser in the Rust CLI. The analysis layer (`containers.rs`, `section_components.rs`) stays on scanner evidence and never consults the guard, and the guard never re-derives container/component structure. The JS renderer remains the only HTML generator; fixture parity and differential tests (PRD §51) keep the Rust guard's verdict and the JS runtime's DOM in agreement.
- **Dependency and build impact:** this introduces the first third-party crate set in the std-only CLI (`html5ever`, `markup5ever`, `tendril`, `string_cache`, `url`). Requires an ADR (new dependency, security-critical surface), a Sentrux gate pass, `cargo audit`/deny in CI (PRD §17), exact version pinning, and measurement against the 450 KiB CLI binary budget and the `wasm32` target (ARCHITECTURE.md) — html5ever is expected to compile on wasm32 *(assumption: verify)*.
- **Determinism:** validation-only means output bytes are unchanged (PRD §52); `extract(build(source)) == source` remains trivially true.

## Assumptions and open questions requiring human/live verification

1. **All library-state claims.** Versions, release dates, maintenance status, and advisory IDs for html5ever/ammonia/html5gum/lol_html/kuchiki/gumbo in the table above are assumptions from training knowledge. Re-verify with live crates.io, rustsec.org/RUSTSEC, and GitHub lookups before the Tech Spec; pin exact versions after `cargo audit`.
2. **html5ever advisory record.** Whether any current RUSTSEC advisories exist for `html5ever` is unverified; confirm via the advisory database when the dependency lands.
3. **wasm32 and binary size.** html5ever on `wasm32-unknown-unknown` and the release-mode size impact on the 450 KiB budget are unmeasured; validate early in Stream 1.
4. **`url` crate parity.** Verify that the `url` crate's parsing matches browser behavior for attribute values (control-character trimming, backslash handling, scheme case), and confirm its maintenance/advisory state. Add fixture coverage for URL edge cases.
5. **Exact build call sites.** This node read only the listed files; the exact module names and invocation points inside `crates/mdhtml/src/build` must be confirmed against the real build flow during implementation (Agent A's architecture map, PRD §48).
6. **Runtime parity (client-side enforcement).** The JS runtime has no sanitizer today and re-renders from the canonical source in the browser, including for hand-edited documents that skip `build` (SPEC.md §3). If author HTML is ever admitted, the Tech Spec must decide how the same policy is enforced client-side: an embedded JS policy walk, build-time validation stamps with runtime degrade-to-prose fallback (the COMP-02 pattern), or a restricted author-HTML surface that is safe by construction. This is the largest open question and directly feeds PRD §51 differential testing.
7. **`{#id}` restriction.** Tightening the heading `{#id}` override to `[A-Za-z0-9_-]` is a small behavior change to a frozen spec surface; it needs a fixture update and explicit Tech Spec sign-off.
8. **Raw-HTML mode decision.** Whether any raw-HTML opt-in will exist at all (PRD §6), and whether it re-serializes (guard) or rejects wholesale, is a product decision for the Tech Spec; the re-serialization path is the one with real mXSS surface and must not ship without the mXSS fixture suite.
9. **`audit` scope.** Audit of a rendered DOM is impossible without a headless browser (the artifact has no pre-rendered body); the Tech Spec must decide whether `audit` validates source-side policy only (recommended for MVP) or adds a browser-based E2E check.
10. **Container/component surface.** Containers and section components are Markdown-shape-driven with fixed renderer-owned wrappers *(verified)*, so the guard has no surface there today; confirm the Tech Spec does not add author-HTML options to them without revisiting this policy.
