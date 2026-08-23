---
type: ADR
id: "0008"
title: "CSS sanitizer/parser selection"
status: proposed
date: 2026-08-22
---

## Context

CSS customization is a core mdhtml feature that must survive the safe-by-default
work: PRD §9 forbids solving CSS security by eliminating arbitrary styling and
requires structural validation by a real Rust CSS parser, with the Tech Spec
explicitly evaluating the maturity and security properties of the candidates
(PRD §46, item 3). Author-controlled CSS — local `.theme.css`, front-matter
`tokens`, and future custom-presentation sources — is untrusted input (Tech
Spec, "Trust boundaries") whose attack surface spans network-capable `url()`,
`@import`, author `@font-face`, unknown at-rules, chrome-targeting selectors,
state-probing pseudo-classes, `!important` escalation, and full-viewport
spoofing overlays (PRD §9–§10; Tech Spec, "CSS attack surface"). Sanitization,
not CSP, is the enforcement boundary: the runtime must mount inline styles, so
`style-src 'unsafe-inline'` stays and CSP is defense-in-depth only.

## Decision

**Depend directly on `lightningcss` and implement a small in-repo, fail-closed
policy module in the Rust CLI; do not depend on `css-sanitizer` for v1.**
Reason: full typed-AST fidelity with a stable serialization path and no 0.x
policy-API churn (Tech Spec, "CSS attack surface" and "Sanitizer library
evaluation").

The policy module:

- Parses author CSS with `lightningcss` `StyleSheet::parse` and
  `error_recovery: false` — malformed author CSS fails closed — then walks the
  typed AST; raw text is never spliced.
- Drops `@import` in any form, `@namespace`, and unknown at-rules.
- Drops network `url()`; allows only `data:`, embedded-asset-map references,
  and runtime `blob:` for asset-map lazy images.
- Allows author `@font-face` only when its `src` resolves to an embedded
  catalog font; `fonts.url` remains a document-level non-portable opt-in, not a
  sanitizer allowance.
- Preserves visual/layout properties and `@media`, `@container`, `@supports`,
  `@layer`, `@scope`, `@keyframes`, `@page`, and `@counter-style` without
  external symbols.
- Denies full-viewport chrome-covering overlay patterns; denies `!important` in
  hosted mode; keeps hosting chrome outside the document subtree.
- Re-serializes with fixed `PrinterOptions` for byte-stable derived CSS; the
  canonical `#mdhtml-source` bytes are never touched.
- Applies to author-authored CSS only; trusted runtime CSS bypasses the
  sanitizer.

## Options considered

- **`lightningcss`** (chosen): production-grade typed AST parse/serialize over
  the broad modern CSS surface, built on the Firefox-derived `cssparser`; ships
  no built-in policy, which fits an explicit in-repo allowlist
  (`docs/research/c-css-security.md`).
- **`css-sanitizer`** (rejected for v1): wraps `lightningcss` in a policy
  trait, but it is a young 0.x API with low adoption and a fail-open default
  trait — a risky direct contract for a security boundary. Its trait shape is
  mirrored in-repo instead; revisit if the API stabilizes.
- **`cssparser` + `selectors` raw primitives** (fallback only): mature
  foundations, but no stylesheet-level AST and no full-rule serializer — rule
  model, traversal, and serialization would all be hand-built. Used only if
  `lightningcss` violates the release binary budget.
- **Regex / hand-rolled sanitizer** (rejected): CSS grammar is contextual
  (strings, comments, escapes, `url()` tokens); regex cannot reliably
  distinguish values or re-serialize safely.

## Consequences

- The Rust CLI gains its first third-party crate; ADR 0004 froze a std-only
  CLI, so landing this dependency requires superseding that choice in the
  implementation commit.
- Exact version pinning after `cargo audit`, RUSTSEC verification, the Sentrux
  gate, a `wasm32` compile check, and release binary-size measurement against
  the 600 KiB budget are mandatory before the crate lands; if the budget fails,
  the documented fallback (`cssparser` + `selectors`) supersedes this ADR.
- Tech Spec Phase 2, "Add `lightningcss`-based CSS policy and deterministic
  author-CSS re-serialization" (`crates/mdhtml/src/security/css/`), implements
  this decision; its exit criteria — all CSS fixtures pass and portable docs
  have zero network `url()` — are this ADR's acceptance test.
- Tech Spec Phase 6 fuzzing must maintain the invariant "CSS sanitizer output
  contains no network-capable `url()`/`@import`".
- The policy splits visual customization (kept — PRD Constraint 3) from
  execution, external communication, and host interference (dropped). Malformed
  CSS fails the build; disallowed nodes are dropped from derived CSS only and
  source bytes are untouched, preserving byte-perfect extraction (PRD
  Constraint 2).
- Status stays `proposed` until the Phase 2 implementation lands.
