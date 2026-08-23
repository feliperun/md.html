---
type: ADR
id: "0009"
title: "Safe vs unsafe build mode"
status: proposed
date: 2026-08-22
---

## Context

mdhtml gives authors extensive control over presentation — HTML, CSS, URLs,
embedded assets — which creates a security boundary (PRD §4). The product
principle is that `mdhtml build` must produce an artifact that passes the
mdhtml security policy by default, failing the build on unsafe content rather
than silently mutating it (PRD §5). At the same time, advanced users may
intentionally need behavior the safe profile prohibits, so the PRD requires an
explicit escape hatch and leaves the Tech Spec to determine whether unsafe mode
disables all guards, disables particular guards, or supports both models
(PRD §6). The Tech Spec resolved this in its "Proposed security pipeline"
(`--unsafe` profile) subsection, its "CLI changes" section, and its
"Migration/compatibility" section. This ADR records that resolution.

## Decision

**`mdhtml build` is safe by default, and the single escape hatch is the explicit
`--unsafe` flag, which disables all content-security guards at once (HTML, CSS,
URL, resource) while keeping format, toolchain, and asset-integrity
validations.**

- The default build runs the full security pipeline and rejects unsafe content;
  it never silently rewrites it.
- `--unsafe` requires explicit opt-in, never defaults on, marks the artifact
  unsafe, and that artifact is rejected by official hosting.
- Safe-mode builds record an attestation in the artifact
  (`data-mdhtml-safe="true"` or equivalent), so unsafe artifacts are
  distinguishable by `mdhtml audit`.
- Selective per-guard disabling is deferred: the MVP ships one profile, not a
  guard matrix.

## Options considered

- **One `--unsafe` profile disabling all content-security guards** (chosen):
  satisfies PRD §6's explicit-opt-in requirement with the smallest CLI surface;
  the Tech Spec records selective disabling as deferred, not planned.
- **Per-guard selective disabling** (e.g. allow-listing a single construct):
  rejected for MVP — it turns one escape hatch into a guard matrix that
  multiplies policy combinations, audit semantics, and hosting rejection rules
  without a demonstrated need.
- **Both models (global flag plus per-guard overrides)**: rejected for MVP for
  the same reason; re-evaluated only after real demand exists.
- **No escape hatch at all**: rejected — PRD §6 requires that advanced users be
  able to intentionally exceed the safe profile; a real escape hatch is what
  keeps the safe policy itself strict.

## Consequences

- The CLI contract gains `--unsafe` on `mdhtml build`
  (`mdhtml build <in.md> [-o out] [--watch] [--no-fonts] [--unsafe]`); unsafe
  builds happen only with the explicit flag.
- `--unsafe` is not a spec bypass: format, toolchain, and asset-integrity
  validations still run; only the content-security guards (HTML, CSS, URL,
  resource) are disabled.
- The CLI displays an appropriate warning on unsafe builds, and agents must not
  use `--unsafe` unless explicitly instructed by a human user (PRD §6, §34).
- `mdhtml audit` reports unsafe artifacts as unsafe, official hosting rejects
  them, and `mdhtml publish` never uploads them.
- Tech Spec Phase 3 ("Implement `--unsafe` profile and artifact marking",
  `crates/mdhtml/src/cli.rs` and `crates/mdhtml/src/build/mod.rs`) implements
  this decision; Phase 4 (hosting-side rejection) and Phase 5 (`mdhtml publish`
  never uploading unsafe artifacts, agent-skill security guidance) depend on the
  marked-artifact contract defined here. PRD §54 lists "unsafe mode requires
  explicit opt-in" as part of the Definition of Done for safe build.
- Introducing per-guard profiles later requires a superseding ADR, not an edit
  to this one.
