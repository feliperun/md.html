---
type: ADR
id: "0006"
title: "Security validation architecture"
status: proposed
date: 2026-08-22
---

## Context

mdhtml documents carry author-controlled HTML, CSS, URLs, and embedded
assets, which creates a security boundary: a malicious document can attempt
scripts, inline handlers, `javascript:` URLs, dangerous SVG, mutation XSS,
and CSS-based exfiltration (PRD §4). PRD §5–§7 require that `mdhtml build`
be safe by default — "reject rather than silently mutate" — that unsafe
content fails the build, that an explicit `--unsafe` escape hatch exist, and
that security use defense in depth: no single sanitizer, regex, CSP, or
parser is sufficient. The Tech Spec's current-architecture analysis (from
research brief `a-architecture-map.md`) lists the open gaps: link
destinations are escaped but not scheme-validated, theme CSS and tokens are
inlined without structural validation, `fonts.url` flows into the CSP
unvalidated, embedded SVG has no script/external-reference check, and there
is no runtime-integrity verification.

The Tech Spec (`docs/tech-spec/mdhtml-safety-hosting-tech-spec.md`,
"Threat model", "Trust boundaries", "Proposed security pipeline") decided
the pipeline that closes these gaps. This ADR records that decision (#1 in
PRD §46); the library selections, CSP strategy, and hosting decisions get
their own ADRs.

## Decision

**Safe-by-default is enforced by a single ordered, fail-closed validation
pipeline inside `mdhtml build`, where every guard validates and rejects —
never silently rewrites — and the artifact carries its own policy result,
so safety is a property of the artifact, not the hosting provider.**

- Build order (Tech Spec "Proposed security pipeline"): FMT-02
  script-termination gate → front-matter/analysis validation → asset
  ingestion validation (safe relative paths, closed MIME table, base64
  validation, SVG structural policy) → HTML/URL policy walk on the
  renderer-serialized output → CSS parse + policy + deterministic
  re-serialization of author CSS → runtime manifest integrity check and
  runtime hash computation → CSP assembly (`meta` hash policy; `fonts.url`
  only as explicit non-portable relaxation) → deterministic assembly and
  atomic write → safe-mode attestation in the artifact
  (`data-mdhtml-safe="true"` or equivalent) without storing secrets.
- Policy lives in-repo as fail-closed modules
  (`crates/mdhtml/src/security/`) over third-party parser engines —
  `html5ever` for HTML, `lightningcss` for CSS; the policy derives from the
  Tech Spec threat model, not from library defaults (PRD §8).
- The threat model names six adversaries (malicious author, compromised
  client, hand-editor, viewer, abusive publisher, future browser feature)
  and four trust boundaries (build, runtime, hosting publish, hosting
  delivery). Untrusted inputs: canonical source, front matter,
  `.theme.css`, `tokens`, linked URLs, `fonts.url`, metadata URLs, and
  embedded assets including SVG.
- Reject-don't-mutate preserves byte-exact source recovery: the canonical
  `#mdhtml-source` bytes are never altered, and a violation fails the
  build with the artifact left unwritten.
- `--unsafe` is one opt-in profile that disables all content-security
  guards (HTML, CSS, URL, resource) while keeping format, toolchain, and
  asset-integrity validations; it marks the artifact unsafe and official
  hosting rejects it. Selective per-guard disabling is deferred.
- The hosting path repeats the same pipeline server-side with the pinned
  canonical WASM toolchain; client validation is never trusted. `mdhtml
  audit` re-runs the same policy over existing artifacts.

## Options considered

- **Validate-and-reject pipeline (chosen)**: preserves deterministic bytes
  and exact source recovery, and fails loudly with a diagnostic instead of
  shipping a mutated document (PRD §5).
- **Sanitizer cleaning (ammonia-style strip/rewrite)**: rejected as the
  engine — cleaning re-serializes, which is exactly where its historical
  mutation-XSS advisories live, and mutated output breaks deterministic,
  byte-stable artifacts. Ammonia stays only as the documented future
  cleaner if a cleaning mode is ever added (research `b-html-security.md`).
- **Silent normalization**: rejected — PRD §5 requires reject rather than
  silently mutate; rewriting risks changing author intent.
- **Hosting-side-only validation**: rejected — safety must be a property
  of the artifact so `file://`, email attachments, GitHub Pages, and any
  static host get the same guarantees (PRD §36, Constraint 5).
- **Single-layer defense** (one sanitizer, or CSP alone): rejected — PRD §7
  mandates defense in depth; CSP complements the pipeline but cannot
  replace structural validation, and sanitizers have bypass history
  (research `d-browser-security.md`).
- **Regex filtering**: prohibited outright by PRD §8.

## Consequences

- Phase 2 (Core security pipeline) implements this directly: the
  `html5ever` HTML/URL guard in `crates/mdhtml/src/security/html/`, the
  `lightningcss` CSS policy in `crates/mdhtml/src/security/css/`, asset/SVG
  validation, and runtime-hash CSP emission. Phase 3 (`mdhtml audit`,
  `E-MDHSEC-*` diagnostics, `--unsafe`, deterministic build validation) and
  Phase 4 (server-side re-validation in hosting) reuse the same policy
  modules, and Phase 6 fuzzes its invariants (`safe build → audit always
  passes`; user-controlled input never creates an unauthorized executable
  node).
- The parser crates end ADR 0004's std-only CLI constraint and require a
  superseding ADR, with exact version pinning after `cargo audit`, RUSTSEC
  verification, Sentrux gate, `wasm32` compile check, and binary-size
  measurement against the 600 KiB release budget.
- Safe-by-default ships as an additive security profile over the frozen
  v1.0 contract: documents violating the new policy now fail rather than
  build (e.g., `{#id}` tightening to `[A-Za-z0-9_-]`), new `E-MDHSEC-*`
  codes are additive, and existing enumerated diagnostic codes are
  unchanged.
- The in-repo policy carries a maintenance burden: it must track new
  network-capable or executable browser constructs (the "future browser
  feature" adversary), exercised by the `fixtures/security/` corpus and
  cargo-fuzz.
- Companion ADRs record the specific selections this architecture
  references: HTML sanitizer selection, CSS sanitizer/parser selection,
  safe vs unsafe mode, and CSP strategy (PRD §46 items 2–5).
