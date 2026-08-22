---
type: ADR
id: "0005"
title: "Runtime fragment boundaries"
status: active
date: 2026-08-21
---

## Context

ADR 0004 established classic IIFE runtime fragments and a committed manifest, but
its initial implementation still treated the complete runtime as one `core`
artifact. T09b freezes the lifecycle boundaries needed for deterministic
per-document selection while keeping the parser, renderer, assets, styles, and
components together.

## Decision

The runtime has exactly four manifest fragments, in fixed order: `core`, `copy`,
`toc`, and `lightbox`. Core owns parsing, rendering, component context, image
hydration, styles, and the shared successful-render evidence. Copy, TOC, and
lightbox are optional lifecycle surfaces.

Core writes one shared state at `globalThis[Symbol.for("mdhtml.runtime.1")]`.
Optional fragments read only that state, require successful core evidence, and
are harmless when executed alone or when the document has no evidence for their
surface. `runtime.min.js` is the exact byte concatenation of the four committed
fragment files in manifest order; selection consumes closed analysis evidence,
not Markdown parsing.

This ADR supersedes only ADR 0004's runtime fragment granularity detail. ADR
0004 remains active and unchanged for its format, classic-runtime, manifest,
toolchain, dependency, and fixture decisions.

## Options considered

- Per-component and highlight fragments: rejected because they duplicate or
  expose parser/render context and require a larger cross-IIFE protocol.
- One full runtime fragment: rejected because every document would embed all
  lifecycle surfaces and selection would not reduce runtime bytes.
- Multiple cross-fragment globals: rejected because they make ordering and
  failure behavior harder to audit than one private protocol symbol.

## Consequences

The CLI can select the smallest lifecycle set using headings, TOC config, and
image evidence. Each fragment has independently reproducible size and SHA-256
metadata, and the full runtime has a simple concatenation invariant. Bootstrap
remains the ESM composition used by tests and development; browser artifacts
come only from the four classic entry files.
