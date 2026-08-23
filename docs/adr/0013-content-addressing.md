---
type: ADR
id: "0013"
title: "Content addressing"
status: proposed
date: 2026-08-22
---

## Context

The hosting MVP stores every published document as an immutable object served
through one shared application (PRD §20, §25; Tech Spec "Hosting
architecture"). PRD §24 asks that documents be content-addressed where
practical — `SHA256(canonical source)` mapping to object storage — with public
short IDs referencing the content object, to get deduplication, integrity
verification, immutable artifacts, simpler caching, reproducibility, and room
for future verification features. PRD §23 lists content-derived identifiers
among the URL options to evaluate, PRD §27 relies on content hashes as an
abuse defense, and PRD §43 assumes immutable, aggressively cacheable objects.
The Tech Spec's "Storage model" section settles the scheme, informed by the
hosting research brief `docs/research/e-hosting.md` named in the Tech Spec
inputs.

## Decision

**Content-address published documents by the SHA-256 of the canonical
Markdown source.** A private Vercel Blob store holds three key spaces:

```
sources/{sha256(source)}        canonical Markdown
docs/{toolchainId}/{sha256}     built .md.html
ids/{publicId}                  { sha256, toolchainId, createdAt }
```

- `sha256` is always the SHA-256 of the canonical source — never of the
  built artifact.
- `toolchainId` pins the toolchain binary/build configuration so old IDs keep
  serving the exact bytes they were published with.
- Identical sources deduplicate; re-publishing the same source and toolchain
  returns the existing object.
- `/d/<toolchain>/<sha256>` is the content-addressed long URL; `/<shortId>`
  resolves once via a cached 308 (Tech Spec "Hosting architecture"). Public
  IDs themselves stay random — 12-character base64url from a server-side
  CSPRNG — per the ID-generation decision.
- Content objects are served with
  `Cache-Control: public, max-age=31536000, immutable` (Tech Spec "Caching
  model").
- Takedown appends the source hash to a deny list, which prevents
  republishing the same source (Tech Spec "Abuse controls").

## Options considered

- **Content-address by canonical source hash** (chosen): gives
  deduplication, integrity verification, and hash-level takedown for free,
  and attaches identity to the canonical document (PRD Constraint 1) rather
  than to one particular build. Combined with random public IDs, this is the
  hybrid PRD §23 asked to be evaluated.
- **Content-address by built artifact hash**: rejected — artifact bytes
  change with every toolchain update, so one document would multiply objects
  and long URLs across versions, and identity would follow presentation, not
  source. The `docs/` key space still partitions by `toolchainId` so each
  pinned build stays byte-stable, but the primary key is the source hash.
- **Opaque storage keys (store under the public ID, no content addressing)**:
  rejected — identical sources would create duplicate objects, integrity
  could not be verified from the API response alone, and the hash deny-list
  takedown path would not exist.
- **Content-derived public IDs (public ID = truncated hash)**: rejected in
  the ID-generation decision — public IDs must be enumeration-resistant
  random IDs; content addressing stays internal and short IDs reference the
  content object (PRD §24).

## Consequences

- The storage layer must implement the three key spaces exactly, and every
  `sha256` in the system means SHA-256 of the canonical source bytes.
- Publish responses expose that hash (the `sha256` field of
  `POST /v1/documents`), so clients can verify integrity end to end.
- Deterministic builds (Phase 3) are what make
  `docs/{toolchainId}/{sha256}` stable; `toolchainId` must be bumped
  whenever the pinned toolchain binary or build configuration changes.
- Long-URL responses can be cached as immutable forever; short-ID resolution
  is cached separately, and takedowns must delete the ID object and purge
  CDN content.
- Hash deny lists only work because identity is the source hash; the
  takedown tooling depends on this decision.
- Tech Spec "Phases and Tasks" items that depend on it: Phase 4 —
  "Implement Blob storage layout and short-ID/long-URL resolution" (exit:
  content deduplicates and short IDs resolve), "Implement
  `POST /v1/documents`" (the hash step of the server pipeline), and "Apply
  isolated origin, security headers, robots/indexing, and takedown controls"
  (hash deny list); Phase 5 — `mdhtml publish`, whose response carries the
  public URL and source hash.
- Status stays `proposed` until Phase 4 implements it.
