# Architecture Decision Records

Architecture Decision Records (ADRs) for **md.html**.

## Format

Each ADR is markdown with YAML frontmatter:

```markdown
---
type: ADR
id: "0001"
title: "Short decision title"
status: proposed        # proposed | active | superseded | retired
date: YYYY-MM-DD
superseded_by: "0007"  # only if status: superseded
---

## Context
...

## Decision
**What was decided.**

## Options considered
...

## Consequences
...
```

### Status lifecycle

```
proposed → active → superseded
                 ↘ retired
```

## Rules

- One decision per file.
- Files named `NNNN-short-title.md` (monotonic numbering).
- Once `active`, never edit — supersede instead.
- [../ARCHITECTURE.md](../ARCHITECTURE.md) reflects active decisions only.

## Index

| ID | Title | Status |
|----|-------|--------|
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions | active |
| [0002](0002-root-managed-ai-guidance.md) | Root-managed AI guidance files | active |
| [0003](0003-sentrux-structural-quality-gates.md) | Sentrux structural quality gates | active |
| [0004](0004-mdhtml-format-and-toolchain.md) | mdhtml 1.0 format and toolchain | active |
| [0005](0005-runtime-fragment-boundaries.md) | Runtime fragment boundaries | active |
| [0006](0006-security-validation-architecture.md) | Security validation architecture | proposed |
| [0007](0007-html-sanitizer-selection.md) | HTML sanitizer selection | proposed |
| [0008](0008-css-sanitizer-selection.md) | CSS sanitizer/parser selection | proposed |
| [0009](0009-safe-vs-unsafe-mode.md) | Safe vs unsafe build mode | proposed |
| [0010](0010-csp-strategy.md) | Content Security Policy strategy | proposed |
| [0011](0011-hosting-architecture.md) | Hosting architecture | proposed |
| [0012](0012-source-vs-artifact-upload.md) | Publish payload: source vs artifact upload | proposed |
| [0013](0013-content-addressing.md) | Content addressing | proposed |
| [0014](0014-public-id-generation.md) | Public ID generation | proposed |
| [0015](0015-anonymous-publishing-policy.md) | Anonymous publishing policy | proposed |
| [0016](0016-isolated-user-content-origin.md) | Isolated user-content origin | proposed |
| [0017](0017-storage-provider-selection.md) | Storage provider selection | proposed |
| [0018](0018-abuse-takedown-model.md) | Abuse and takedown model | proposed |
