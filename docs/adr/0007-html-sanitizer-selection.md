---
type: ADR
id: "0007"
title: "HTML sanitizer selection"
status: proposed
date: 2026-08-22
---

## Context

mdhtml documents carry author-controlled HTML values — Markdown link
destinations, heading `{#id}` overrides, section `class` tokens, front-matter
`url`/`cover`, embedded SVG, and any future raw-HTML surface — and
`javascript:` link destinations already reach the artifact today (Tech Spec,
"Current architecture analysis"). PRD §8 requires a mature HTML5-aware
sanitizer/parser (naming Ammonia as the Rust candidate to evaluate), forbids
regex-based sanitization, and demands the final policy come from threat
analysis rather than library defaults; PRD §5 requires rejection over silent
mutation. The Tech Spec's "HTML attack surface" and "Sanitizer library
evaluation" sections, grounded in `docs/research/b-html-security.md`, selected
the engine and policy (PRD §46 item 2); this ADR records that decision.

## Decision

**Use `html5ever` directly — pinned to an exact version after `cargo audit` —
as the single HTML parser in the Rust CLI, wrapped in a validation-only,
reject-don't-mutate allowlist guard implemented in-repo. Ammonia is not the
engine; it remains the documented future cleaner if a cleaning mode is ever
added.** Validation preserves deterministic bytes and avoids the mutation-XSS
re-serialization path: a violation fails the build and leaves the artifact
unwritten. The guard's policy, from the Tech Spec's "HTML attack surface"
controls:

- The renderer's element set stays fixed; `script`, `style`, `iframe`,
  `object`, `embed`, `form`, `svg`, `math`, `template`, `noscript`, `base`,
  `meta`, `link`, and custom elements are denied.
- Only safe attributes are allowed; all `on*` handlers, `style`, `srcdoc`,
  `sandbox`, `allow`, `ping`, and URL-bearing fetch attributes are denied.
- URI schemes allow `http`, `https`, `mailto`, `tel`, relative, and
  fragment-only; `javascript`, `vbscript`, `data` in `href`, `file`, `blob`
  in `href`, and unknown schemes are denied.
- `{#id}` is restricted to `[A-Za-z0-9_-]`; section/class tokens are
  validated against the existing CSS-identifier contract.

## Options considered

- **`html5ever`** (chosen): mature WHATWG-conformant parser (tokenizer plus
  tree builder) with no policy layer of its own, so validation-only use fits
  reject-don't-mutate and shipped bytes stay deterministic
  (`docs/research/b-html-security.md`).
- **`ammonia`** (rejected as engine): mature allowlist sanitizer built on
  html5ever, but it cleans — parse, filter, re-serialize. Re-serialization
  changes bytes, and its historical mutation-XSS advisories live in exactly
  that path. Retained as the documented future cleaner.
- **`html5gum`, `lol_html`, `kuchiki`, `gumbo`**: rejected as tokenizer-only,
  rewriting-focused, or unmaintained.
- **Regex-based sanitizers**: prohibited outright by PRD §8.

## Consequences

- The guard (`crates/mdhtml/src/security/html/`) is the only HTML parser in
  the Rust CLI: the analysis layer stays on scanner evidence and the JS
  runtime remains the only HTML generator, so no parser can disagree with the
  renderer.
- This is the first third-party dependency in the std-only CLI (ADR 0004).
  Per the Tech Spec's dependency constraints, the crate lands only after
  exact version pinning, `cargo audit`, RUSTSEC verification, a Sentrux gate
  pass, a `wasm32` compile check, and a release binary-size measurement — all
  open until live verification.
- Safe mode never mutates author bytes: `extract(build(source)) == source`
  and deterministic builds are preserved by construction.
- Tech Spec Phase 2 task "Add `html5ever`-based validation-only HTML/URL
  guard" (`crates/mdhtml/src/security/html/`) implements this decision;
  Phase 6's cargo-fuzz targets and adversarial review gate exercise the same
  policy, and Phase 3's `E-MDHSEC-*` diagnostics report its rejections.
- If a raw-HTML mode beyond `--unsafe` is ever added and cleans instead of
  rejecting, the ammonia question reopens (Tech Spec open questions).
