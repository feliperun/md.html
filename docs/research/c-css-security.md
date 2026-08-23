# CSS parsing/sanitization for author-controlled CSS — research

Scope: this document researches CSS parsing/sanitization technology for the Rust
implementation and recommends one concrete approach that validates author-controlled
CSS while preserving maximum styling freedom, per PRD section 9 and section 48 (Agent
C). It recommends only; implementation is PRD section 49, Stream 1. The final Tech
Spec is the synthesis node's job.

Sources read: `docs/prd/mdhtml-safety-hosting-prd.md` (§§4, 9–12, 47–51),
`SPEC.md` (§§2, 5, 13, 14, 18, 20), `docs/ABSTRACTIONS.md`,
`runtime/src/styles.js`, `runtime/build-styles.mjs`,
`crates/mdhtml/src/selection/fonts.rs`.

## Candidate libraries

| Library | Maturity | Maintenance | AST fidelity | Sanitization affordances | Notes / risk |
|---|---|---|---|---|---|
| `lightningcss` | Production-grade engine (Parcel ecosystem), still alpha-versioned (`1.0.0-alpha.72`, 2026-07-20) | Active; ~40 releases; built on `cssparser`, the parser Firefox uses | Full: typed AST for style rules, declarations, selectors, and at-rules (`@import`, `@font-face`, `@media`, `@container`, `@supports`, `@layer`, `@scope`, `@keyframes`, `@page`, unknown at-rules) | `StyleSheet::parse` with `error_recovery` and `ParserOptions`; stable `to_css` re-serialization; no built-in policy — policy is written by us | Large dependency; no 1.0; CLI currently ships zero third-party crates (SPEC §18) → binary-budget + ADR decision |
| `css-sanitizer` | Young (0.1.x 2026-03, 0.4.0 2026-08-09); low adoption | Active (several 0.x releases in 2026) | Re-exports `lightningcss`; adds per-node traversal over rules, properties, selectors, descriptors | `CssSanitizationPolicy` trait with `NodeAction::Continue`/`Drop`, `clean_stylesheet_with_policy`, `sanitize_stylesheet_ast`; default trait behavior is fail-open — the policy must explicitly drop everything unwanted | 0.x API churn; ~1.5k SLoC; example policies (e.g. style-only) are presets, not a safe default — the policy is ours either way |
| `cssparser` + `selectors` (raw primitives) | The Firefox-derived foundations `lightningcss` builds on; very battle-tested | Mature | Tokenizer/parser + selector parser only — no stylesheet-level AST, no serializer for the full rule surface | None; everything (rule model, traversal, serialization) is hand-built | Only worth it if the `lightningcss` dependency is unacceptable; significant engineering |
| Regex / hand-rolled sanitizer | n/a | n/a | None | None | Rejected: CSS grammar is contextual (strings, comments, escapes, `url()` tokens); regex cannot reliably distinguish values or re-serialize safely |

Versions and maintenance status were verified via web search on 2026-08-22; treat
exact-state claims as needing re-verification at pin time (see Assumptions).

## Recommended approach

**Primary: depend directly on `lightningcss` and implement a small in-repo policy
module. Do not depend on `css-sanitizer` for v1.**

Rationale:

- `lightningcss` gives full-fidelity parse + serialize of the entire modern CSS
  surface, so the sanitizer operates on typed AST nodes and never reconstructs or
  splices raw text. Rules, declarations, selectors, and `url()` values are modeled,
  which is exactly what a policy layer needs to make deny-by-default decisions.
- `css-sanitizer`'s real value is its policy-trait shape, not its code volume. Its
  0.x API churn and low adoption make it a risky direct contract for a security
  boundary; mirror its trait shape in-repo (a few hundred lines) so the policy is
  explicit, fail-closed, and unit-testable. Revisit this after human verification —
  if the API stabilizes, pinning `css-sanitizer` is the lower-code alternative.
- The policy layer itself (what to keep, what to drop) is application logic that must
  match the JS runtime's behavior for differential testing (PRD §51); it should not
  live inside a generic library.

Pipeline (build-time, applied to author-authored CSS only):

1. Parse with `StyleSheet::parse` and `error_recovery: false` — malformed author CSS
   fails closed and is reported by `mdhtml check`; warnings are collected, not
   swallowed.
2. Walk the typed AST and apply the policy below. Disallowed nodes are dropped;
   everything else is kept untouched.
3. Re-serialize with fixed `PrinterOptions` so output is byte-stable and
   deterministic across builds. Never splice original text.
4. Fuzz the policy and run differential tests between the Rust and JS
   implementations of the same policy on deterministic fixtures (PRD §51).

Scope of the sanitizer: author-authored CSS — local `.theme.css`, front-matter
`tokens`, and any future custom-presentation source — is sanitized before being
inlined into the artifact. Trusted runtime CSS (`runtime/src/styles.js`, built-in
theme presets) is never run through the sanitizer; it is the toolchain's own code.

## Policy: url()/@import/@font-face/selectors

Default stance: preserve everything that is purely visual/layout for the document's
own content; drop anything network-capable, execution-capable, or able to reach
hosting chrome. Deny by default, allow by explicit rule — with a broad styling
allowlist.

Keep without action:

- Style rules and properties: layout, typography, color, transforms, filters,
  animations/transitions, counters, and similar.
- At-rules that only affect rendering of the artifact's own content: `@media`,
  `@container`, `@supports`, `@layer`, `@scope`, `@starting-style`, `@keyframes`,
  `@page`, `@counter-style` (without external symbols).

Drop:

- `@import` in any form (relative, absolute, protocol-relative, `data:`).
- `@namespace` and `@-moz-document`, plus any unknown at-rule.
- Any declaration whose value contains a network-capable `url()` (below).
- Author `@font-face` unless its `src: url(...)` resolves to an embedded font asset
  block through the document asset map (below).

`url()` policy — applies to `background-image`, `border-image`, `cursor`, `mask`,
`clip-path`, `shape-outside`, `list-style-image`, `content`, `image-set()`,
`cross-fade()`, and any future URL-bearing function:

- Allow `data:` URIs and references resolved against the embedded asset map (for
  example `url(./fonts/...)`, `url(assets/...)`), rewritten at build to the same
  `data:` / `blob:` form the runtime uses.
- Allow `blob:` only for asset-map-resolved references at runtime (large lazy
  images); never for arbitrary input.
- Drop `http:`, `https:`, protocol-relative `//`, `ftp:`, `file:`, and any unknown
  scheme. Relative paths outside the asset map are unresolved and dropped.
- Reject `url()` tokens that render nothing (for example an empty URL in a property
  that would otherwise have a printable fallback) when they appear without a
  fallback.

`@font-face`:

- Author `@font-face` is allowed only when `src: url(...)` points at an embedded
  font asset block from the closed, license-checked catalog (same fail-closed
  mechanism as `crates/mdhtml/src/selection/fonts.rs`: closed key set, exact
  byte/hash validation, OFL/Apache only, `wght` axis only, license notice in the
  artifact). This preserves the Fontshare/ITF non-embedding restriction and matches
  `font-src data:` in the CSP.
- Google Fonts via `fonts.url` (SPEC §5, non-portable variant) remains an explicit
  documented opt-in at the document level — it is not a sanitizer allowance.

Selectors:

- Sanitization operates on the artifact's own document subtree. Hosting chrome must
  live outside that subtree (hosting app shell or separate iframe), so author
  selectors cannot match it. If a hosting design puts chrome in the same DOM, deny
  it by reserved convention: author selectors may not target elements with reserved
  host hooks (for example `#mdhtml-toolbar`, `#mdhtml-toc`,
  `[data-mdhtml-host]`).
- One consistent policy for hosted and local `.md.html` files: keep self-document
  selectors; drop the deceptive full-page overlay pattern (selectors on
  `:root`/`html`/`body` combined with `position: fixed` + full-viewport geometry);
  in hosted mode, additionally drop `!important` and high-specificity escalation.
  The exact local-vs-hosted split is an open question (see Assumptions).

## CSS exfiltration and host-interference vectors and how the recommendation addresses each

- Network fetch via CSS — `url()` in `background`, `border-image`, `cursor`, `mask`,
  `clip-path`, `shape-outside`, `list-style-image`, `content`, `@import`, external
  `@font-face`: dropped by the URL allowlist; the artifact contains no live
  `http(s)` reference. The CSP (`default-src 'none'`) is defense-in-depth only —
  `style-src 'unsafe-inline'` is required for the runtime style mount, so
  sanitization is the enforcement boundary.
- External font loading: `@font-face` with a network URL is dropped; only embedded
  catalog fonts are reachable, plus the explicit `fonts.url` opt-in.
- Attribute/selector-based leaks (`:has()`, `:visited`, `input:checked`,
  attribute-substring selectors, sibling combinators probing state): with zero
  network capability these have no exfiltration channel. The selector policy
  additionally strips state-probing pseudo-classes where a concrete probe is
  identified; visual rules using `:checked`/`:hover` styling are preserved.
- Timing/scroll tricks (scrollbar-width styling, `scroll()`/`view()` animations used
  to time rendering): no request can fire, so timing leaks lose their channel;
  scroll-driven styling of the document itself is preserved. Scrollbar-style
  probing that could measure rendering is dropped in hosted mode if measurable.
- Host-chrome interference (high-specificity/`!important` rules, `position: fixed`
  full-viewport overlays, selectors matching host hooks): addressed by chrome living
  outside the document subtree, the reserved-hook ban, denial of the overlay pattern,
  and `!important` denial in hosted mode.
- Deceptive full-page overlays (PRD §10 concern): decorative full-page styling (body
  background, gradients) is preserved; the overlay pattern that covers chrome is
  denied. The residual call is partly content/hosting policy and is flagged for the
  Tech Spec.
- SVG-as-image script execution: embedded SVG used as a CSS background image does not
  execute scripts under normal loading, but asset ingestion must validate MIME and
  reject SVG containing script so a future rendering change cannot make it
  executable. Open question on where that validation lives.
- `content: attr(...)` / `::before`/`::after` injection: visual only, no code
  execution; coordinate with the HTML sanitizer (Agent B) so content escaping stays
  consistent.
- Future network-capable constructs to audit on every renderer upgrade:
  `@property` registered descriptors with `url()` values, `@scroll-timeline` /
  `@view-timeline` sources, scroll-driven animation timing functions,
  `anchor()`/`position-try`, root-scoped `view-transition-name`, `image-set()` /
  `cross-fade()` with remote images, and new pseudo-classes (for example `:state()`)
  that could probe host UI. Rule: anything that can fetch, store, or execute outside
  pure rendering is dropped unless explicitly allowed.

## Interaction with existing self-contained embedding (fonts, images)

- Images: <32 KiB are embedded as `data:` URIs; ≥32 KiB are lazy `blob:` URLs via
  `IntersectionObserver`. Author CSS `url()` referencing an embedded image must
  resolve through the same asset map (`data-md-asset-path`) so the sanitizer rewrites
  it to the same `data:`/`blob:` form the runtime uses. This is why the URL policy is
  an asset-map allowlist rather than a raw `data:` allow-all: raw allow-all would
  admit schemes and payloads that should never appear, and the asset map keeps
  legitimate embedded assets working.
- Fonts: a closed committed woff2 catalog, OFL/Apache only, `wght` axis only, license
  notice carried in the artifact. Author `@font-face` is allowed only for these
  blocks, and `font-src data:` in the CSP already covers them.
- CSP alignment: the policy module is the single source of truth; the CSP is kept in
  sync as defense-in-depth (`style-src 'unsafe-inline'`, `img-src data: blob:`,
  `font-src data:`, `default-src 'none'`). CSP alone cannot be the enforcement
  boundary because the runtime must mount inline styles and toggle themes.
- Trusted runtime CSS (`runtime/src/styles.js`, theme presets) bypasses the
  sanitizer; only author-authored CSS is sanitized at build time before inlining.

## Assumptions and open questions requiring human/live verification

- Library state: `lightningcss` `1.0.0-alpha.72` and `css-sanitizer` `0.4.0` claims
  were verified via web search on 2026-08-22 only. Pin exact versions and re-verify
  before implementation.
- `css-sanitizer` decision: direct dependency vs in-repo policy module. Revisit after
  checking API stability and adoption; the recommendation currently avoids the 0.x
  dependency.
- Binary budget: adding `lightningcss` to a CLI that ships with zero third-party
  crates (SPEC §18, release CLI ≤600 KiB) needs measurement and an ADR. Options:
  accept the budget change, feature-gate the sanitizer, or fall back to `cssparser`
  if budget wins.
- Hosting-chrome topology: confirm hosting chrome is guaranteed outside the artifact
  subtree (app shell/iframe). Single-DOM hosting would force a stricter
  selector/`!important` policy.
- `!important` and overlay policy: the exact rule set (deny in hosted mode vs always)
  needs a product decision with the hosting service (PRD §§48–49).
- `blob:` at runtime: whether author CSS may reference large lazy images via `blob:`
  URLs, and how the policy expresses that allowance.
- Differential testing harness (PRD §51): the fixture corpus and the JS-vs-Rust
  parity target need scoping before Stream 1 implementation.
- SVG-as-CSS-image: confirm whether SVG content validation lives in the asset
  pipeline (Asset agent / Agent B) or must be added to this policy.
- Sanitizer entry point: confirm that front-matter `tokens`, local `.theme.css`, and
  any future custom-presentation sources all flow through one sanitizer path.

