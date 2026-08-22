---
type: ADR
id: "0004"
title: "mdhtml 1.0 format and toolchain"
status: active
date: 2026-08-20
---

## Context

mdhtml 1.0 freezes a single-file document format — Markdown as the canonical
source, HTML as presentation — plus a toolchain around it. The decisions span
the format contract (canonical source, CSP, runtime artifacts, diagnostics)
and the toolchain (a dependency-free classic JavaScript runtime, a std-only
Rust CLI, and two development-only dependencies: esbuild and Playwright). The
format must open from `file://` with zero network access, and the same core
must later compile to WASM for a hosted product.

`SPEC.md` is the normative authority; this ADR records the cross-cutting
choices that SPEC.md implements.

## Decision

1. **Format.** One canonical `#mdhtml-source[type="text/markdown"]` per
   document; hash-based section navigation; a closed CSP for portable
   documents; a closed front-matter YAML subset; semantic containers and
   section components that always degrade to prose; byte-exact round-trip via
   `extract`.
2. **Runtime.** Dependency-free classic JavaScript embedded inline. No
   `type="module"`, `fetch`, or `pushState` (all three break under `file://`).
   The runtime ships as classic IIFE fragments plus a committed **Runtime
   fragment manifest**; `mdhtml build` concatenates only the fragments the
   document requires; `runtime.min.js` is the full reference bundle, not the
   mandatory embedding unit.
3. **CLI.** Rust stable, std only — argument parsing, front matter, base64,
   HTML and watch mode are hand-written. The front matter parser is the
   reference implementation of the grammar in SPEC.md. The CLI never runs a
   Markdown parser; it only scans what `build`/`check` need.
4. **Development-only dependencies.** `esbuild` generates the committed
   runtime artifacts (`cargo build` does not need Node); `playwright` provides
   deterministic E2E over `file://`. Both live in dev tooling and CI, never in
   the released CLI or in documents.
5. **Shared fixtures.** `fixtures/` is the executable contract between the two
   implementations; a construct accepted by one and rejected by the other is a
   spec violation.

**No runtime dependencies are added** because the document contract forbids
external scripts and network requests; the only conceivable runtime dependency
would be an embedded parser, and the enumerated Markdown surface (SPEC §9) is
small enough to implement directly while staying dependency-free and within
byte budgets. **No Rust crates are added** because the needed surface (args,
the front matter subset, base64, HTML escaping, polling) is small, hand-rolled
code keeps the binary at ~300–450 KB, and std-only keeps the WASM path open
for the future hosted product.

## Options considered

- **Single bundled runtime file** vs fragments + manifest (chosen): fragments
  cut per-document byte budgets and make "components included only when used"
  mechanical; the full bundle remains committed as reference.
- **TypeScript runtime compiled by esbuild** vs classic JS (chosen): the
  runtime is authored and committed as classic JS; esbuild only minifies.
  Simpler, no sourcemaps in artifacts, trivially auditable.
- **Rust CLI with clap/serde** vs std only (chosen): fewer dependencies, a
  smaller binary, and no build-time dependency tree for a ~200-line parser.
- **Runtime parser library (marked/remark)** vs hand-written: every document
  would embed the library and the full CommonMark surface, not the enumerated
  subset; the format's portability promise depends on staying dependency-free.
- **No E2E automation** vs Playwright (chosen): the `file://` constraints and
  the clipboard/lightbox behaviors need deterministic browser coverage;
  Safari/iOS remain manual smoke tests because CI cannot prove a real gesture.

## Consequences

- SPEC.md is the single normative authority; requirements map to sections and
  to stable `Diagnostic` codes shared by both implementations.
- Runtime artifacts are committed and reproducible; `runtime/build.mjs check`
  detects drift.
- Both implementations pass the same fixtures; an extension accepted by one
  and rejected by the other is a spec violation.
- Node is required for development and CI (build script, tests, E2E), never
  for the released CLI.
- Adding a runtime or Rust dependency later requires a superseding ADR.
