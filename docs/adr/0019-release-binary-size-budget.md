---
type: ADR
id: "0019"
title: "Release binary size budget"
status: proposed
date: 2026-08-23
---

## Context

The mdhtml CLI has been dependency-free (`crates/mdhtml/Cargo.toml` had no
`[dependencies]` entries) and CI (`.github/workflows/quality.yml`) enforced a
600 KiB (614400 byte) release-binary ceiling. The baseline release binary was
519168 bytes — about 95 KiB (15%) of headroom.

ADR 0007 (HTML sanitizer selection) and ADR 0008 (CSS sanitizer/parser
selection) choose html5ever and lightningcss as the structural HTML/CSS
guards for the safe-by-default security pipeline, per the Tech Spec's open
question: "Measure wasm32 compatibility and release binary-size impact of
both parser dependencies against the 600 KiB budget."

That question was answered empirically before Phase 2 implementation began,
by adding each library with a real (non-dead-code-eliminated) parse call and
measuring `target/release/mdhtml` after `cargo build --locked --release`
(strip + LTO + `opt-level = "z"`, matching the existing release profile):

| Configuration | Size (bytes) | Delta vs. baseline |
|---|---|---|
| Baseline (no dependencies) | 519,168 | — |
| + html5ever 0.29 (real parse+walk) | 949,520 | +430,352 |
| + html5ever 0.29 + lightningcss 1.0.0-alpha.72 (real parse) | 1,882,704 | +1,363,536 |

html5ever alone already exceeds the 600 KiB CI limit by 335,120 bytes (54%
over budget). With lightningcss added, the binary is roughly 3.06x the old
budget. lightningcss also has no stable 1.0 release yet (latest is
`1.0.0-alpha.72`), which is a separate risk tracked as an open question, not
a reason by itself to change library choice here.

## Decision

**Raise the CI release-binary-size limit from 600 KiB (614400 bytes) to
3 MiB (3145728 bytes)**, keeping html5ever and lightningcss as decided in
ADR 0007 and ADR 0008. The new limit gives roughly 40% headroom over the
1,882,704-byte measurement above to absorb the actual security-pipeline
code (policy tables, diagnostics, CLI wiring) that Phase 2 adds on top of
the bare parser dependencies measured here.

This decision does not reopen ADR 0007 or ADR 0008: the library choices
stand. It revises the unrelated build-size constraint those choices land on.

## Options considered

- **Raise the CI budget to 3 MiB** (chosen): keeps the sanitizer libraries
  the research/Tech Spec already selected for their maturity and structural
  (non-regex) parsing guarantees (PRD section 8 explicitly prohibits
  regex-based HTML sanitization). Accepts a real, permanent regression in
  the "tiny self-contained binary" property the project optimized for
  through v1.0.
- **Choose a smaller HTML/CSS validation approach, superseding ADR
  0007/0008**: would preserve binary size but requires new research into a
  structural (not regex) parser small enough to fit 600 KiB — no such
  candidate was identified in docs/research/b-html-security.md or
  docs/research/c-css-security.md. Deferred: no evidence yet that one
  exists with comparable HTML5/CSS conformance.
- **Feature-gate the sanitizers out of the default `mdhtml build` binary**
  (e.g., audit-only or a separate binary): rejected — it would break
  "safe by default": the same binary that builds must also validate, or
  unsafe artifacts become producible by simply using the smaller build.
- **Keep 600 KiB and accept a much weaker, hand-rolled validator**: rejected
  outright — this is exactly the regex/ad-hoc sanitization PRD section 8
  forbids, and known to be bypassable by HTML parsing edge cases (mutation
  XSS, adoption-agency quirks) that only a real parser handles correctly.

## Consequences

- `.github/workflows/quality.yml`'s "Release binary size limit" step moves
  from 614400 to 3145728 bytes.
- The project's marketing/positioning around a tiny, dependency-free CLI
  changes materially; `docs/ARCHITECTURE.md` and any public messaging about
  binary size should be updated once Phase 2 lands the real number (this
  ADR's 1,882,704-byte measurement is dependencies-only, not the full
  security pipeline).
- Tech Spec Phase 2 (core security pipeline: html5ever guard, lightningcss
  policy, asset/SVG validation, runtime-hash CSP) is now unblocked to start
  from an accurate size budget instead of one that would fail CI on the
  first sanitizer alone.
- The Tech Spec's open question "measure wasm32 compatibility ... against
  the 600 KiB budget" is answered for native release size; wasm32 size and
  compatibility remain open and unmeasured.
