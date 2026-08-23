---
type: ADR
id: "0014"
title: "Public ID generation"
status: proposed
date: 2026-08-22
---

## Context

Published documents are addressed by a short public URL such as
`https://<docs-domain>/H7zPm` (PRD §18, §23). PRD §23 requires URLs that are
short, durable, copyable, case-insensitive if practical, and difficult to
enumerate at scale, and forbids sequential IDs. Internally the service is
content-addressed by `SHA256(canonical source)` (PRD §24; ADR 0013), so the
public ID exists purely as the stable, shareable alias users actually see.
Because anonymous documents are immutable (PRD §25), an ID is never reused or
recycled once created. The Tech Spec's "ID generation" section records the
decision this ADR ratifies.

## Decision

**Every published document receives a 12-character random ID over the URL-safe
base64url alphabet `[A-Za-z0-9_-]` — a NanoID-style ID generated server-side
with a CSPRNG at publish time, with no sequential counter and no
client-provided IDs.** 12 characters at 6 bits per character yield 72 bits of
unguessable entropy, keeping IDs short, durable, and enumeration-resistant; on
the astronomically rare collision (≈10⁻¹² at 100k live IDs), the server
regenerates. The public ID is an opaque alias: it resolves through the
`ids/{publicId}` key space to `{ sha256, toolchainId, createdAt }` and serves
via a cached 308 to the content-addressed long URL `/d/<toolchainId>/<sha256>`
(Tech Spec "Storage model" and "Hosting architecture").

## Options considered

- **12-char random base64url, server-side CSPRNG** (chosen): 64-char alphabet
  gives exactly 6 bits/character, so 12 chars reach 72 bits with a collision
  probability ≈10⁻¹² at 100k live IDs while staying copyable
  (`docs/research/e-hosting.md`, "Public ID scheme").
- **Sequential integer IDs**: rejected — trivially enumerable at scale, which
  PRD §23 forbids and the abuse model treats as a scanner/bandwidth threat
  (`docs/research/f-abuse.md`).
- **Content-derived public IDs**: rejected — the ID must be an opaque alias,
  not the storage address; deriving it from content would leak content
  equality through URLs and change the URL on re-publish under a new
  toolchain. Content addressing lives one layer down (ADR 0013).
- **base58 / base62 alphabets, ≥8 chars** (the shape suggested in
  `docs/research/f-abuse.md`): viable but base58 sacrifices bits per
  character to avoid lookalike characters; base64url keeps full density and
  both `-` and `_` are URL-safe.
- **Shorter IDs** (10 chars = 60 bits, or the PRD's 5-char example `H7zPm`):
  rejected — 12 chars buy ~1000× collision margin for free
  (`docs/research/e-hosting.md`).
- **Case-insensitive 14-char base32 variant (~70 bits)**: kept as an explicit
  open question in the Tech Spec, not chosen — case-insensitivity is "if
  practical" (PRD §23), not required, and the case-sensitive default is
  shorter.

## Consequences

- ID generation lives only in the Publish API, after validation, audit, hash,
  and store; the CLI and agents never choose, predict, or customize IDs (no
  custom slugs at MVP).
- IDs are permanent for the document's lifetime; takedown removes
  `ids/{publicId}` and purges CDN entries (Tech Spec "Caching model"), and the
  ID is never reissued.
- Collision handling (regenerate on the vanishingly rare repeat) and the
  cacheable 308 resolution contract are part of this decision.
- Binds Tech Spec Phase 4 — "Implement Blob storage layout and
  short-ID/long-URL resolution" and the `POST /v1/documents` pipeline, which
  ends in "create ID" — and Phase 5, where `mdhtml publish <source>` returns
  the URL formed by this scheme.
- Switching later to the case-insensitive 14-char variant changes ID length
  and entropy and requires a superseding ADR, not an edit here.
