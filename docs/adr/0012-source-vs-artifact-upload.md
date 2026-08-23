---
type: ADR
id: "0012"
title: "Publish payload: source vs artifact upload"
status: proposed
date: 2026-08-22
---

## Context

The hosting Publish API accepts documents from untrusted clients — the mdhtml
CLI, curl, AI agents, CI. PRD §21 ("Publishing Source vs Generated Artifact")
requires an explicit choice between three payload models — **Option A** (upload
the built `.md.html` artifact), **Option B** (upload Markdown and build
server-side), and **Option C** (upload both and verify correspondence) —
prioritizing security and reproducibility without unnecessarily increasing
infrastructure complexity. The choice is a trust boundary, not a convenience:
the Tech Spec's threat model includes a *compromised client* that "uploads
arbitrary HTML/artifact bytes and claims mdhtml provenance", and its trust
boundary 3 states the server "re-runs the canonical build/audit and never
trusts client validation" (Tech Spec "Threat model" and "Trust boundaries").
The decision is recorded in the Tech Spec's "API contract" section (decision
#7 of the PRD §46 ADR list), with server-side build feasibility established in
its "Hosting architecture" section and research brief `e-hosting.md`.

## Decision

**Upload model Option B — the Publish API receives only canonical Markdown
source plus referenced assets; the server builds the artifact itself.**
`POST /v1/documents` takes `multipart/form-data` with a required `source`
field (the UTF-8 Markdown file) and optional repeated `asset` fields (each
multipart `filename` is the asset's relative path, e.g. `images/photo.png`,
and must satisfy the extraction-safe path predicate). The server pipeline is:
rate limit → size check → rebuild in a temp directory with the pinned canonical
toolchain (the Rust CLI compiled to `wasm32`) → audit → hash → store → create
ID. Content addressing keys off `sha256(canonical source)`, never off
client-supplied artifact bytes. `mdhtml publish <source>` still runs local
build/audit first for fast failure, then uploads the same source-plus-assets
payload; it never uploads `--unsafe` artifacts.

## Options considered

- **Option B — upload Markdown, build server-side** (chosen): only a
  server-side canonical build can prove the artifact was produced by mdhtml,
  and it preserves PRD Constraint 1 (Markdown remains canonical) by
  content-addressing the source hash.
- **Option A — upload the `.md.html` artifact** (rejected): a compromised or
  modified client could upload arbitrary HTML while claiming mdhtml
  provenance; the server would be trusting client-side validation, which
  PRD §30 forbids.
- **Option C — upload both source and artifact, verify correspondence**
  (rejected): verification still requires the server to rebuild from source,
  so the uploaded artifact adds payload size and a comparison path with no
  additional assurance over Option B — exactly the unnecessary complexity
  PRD §21 rules out.

## Consequences

- The Publish API contract is source-only: `source` and `asset` multipart
  fields; no endpoint accepts artifact bytes from clients.
- The server must be able to run the canonical toolchain. Phase 4 items
  "Publish the canonical toolchain as pinned WASM", "Implement
  `POST /v1/documents` with multipart source/assets, validation, and
  structured errors", and "Implement Blob storage layout and short-ID/long-URL
  resolution" depend on this decision, as does the Phase 5 item "Implement
  `mdhtml publish <source>` and asset discovery/multipart upload".
- Validation runs twice by design: locally in the CLI for fast feedback,
  authoritatively on the server. Client-side validation is never trusted.
- Storage and IDs key off `sha256(canonical source)` and `toolchainId` (Tech
  Spec "Storage model"): identical sources deduplicate, and old IDs keep
  serving the exact bytes they were published with. Republishing determinism
  depends on Phase 3's deterministic-build validation.
- Server-side builds add compute per publish (bounded by the 2 MiB source
  limit and publish rate limits), never per view — steady-state views remain
  CDN-served static bytes (PRD §42).
- `--unsafe` documents cannot be published through this contract: the CLI
  refuses to upload them and hosting only ever builds and serves the safe
  profile.
