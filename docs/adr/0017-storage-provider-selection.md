---
type: ADR
id: "0017"
title: "Storage provider selection"
status: proposed
date: 2026-08-22
---

## Context

The hosting MVP (PRD §18–§19) stores published documents as immutable objects
behind a CDN — never one deployment per document (PRD §20) — and
content-addresses them by source hash (PRD §24). PRD §19 names Vercel as the
initial deployment target with Vercel Blob as a potential component and
requires the Tech Spec to verify current pricing, limits, and operational
implications before implementation.

The Tech Spec's "Hosting architecture" and "Storage model" sections make that
call: immutable artifacts live in Vercel Blob inside the single shared Vercel
project, with Vercel Pro as the realistic launch floor after a Hobby beta. The
verified cost estimates behind the decision are in `docs/research/e-hosting.md`
(all Vercel figures verified 2026-08-22).

## Decision

**Vercel Blob is the storage provider for immutable published documents: one
private Blob store inside the single shared Vercel project, beta on the Hobby
plan, Pro as the realistic launch floor before sustained third-party traffic.**

- Objects live under the content-addressed key spaces (`sources/`,
  `docs/{toolchainId}/`, `ids/`) defined by ADR 0013.
- Bytes are served through the docs origin (ADR 0016): a Vercel Function
  streams the Blob object on CDN cache miss. Raw public Blob URLs are never
  the document URL because Blob forces `Content-Disposition: attachment` on
  HTML, which would download instead of render.
- The store uses only simple GET/PUT operations; advanced Blob operations
  (billed at $5.00/1M) are avoided by design.

## Options considered

- **Vercel Blob (chosen)**: integrated with the already-chosen Vercel hosting
  project (ADR 0011); verified pricing — storage $0.023/GB-mo, simple ops
  $0.40/1M, data transfer $0.05/GB — yields ~$0/month at 1k docs, ~$0–1 at 10k
  on Hobby, ~$20–24/month at 100k docs and ~$20–21/month at 1M views/month on
  Pro, dominated by the $20 platform fee (`docs/research/e-hosting.md`).
- **Cloudflare R2 + Workers/Pages**: zero egress makes it the low-cost
  fallback, but it fragments the single-provider architecture and is not
  needed at MVP scale (`docs/research/e-hosting.md`, "Alternatives").
- **GitHub Pages**: fully static with no serverless at all, but the Publish
  API requires a server-side canonical build/audit (PRD §21, ADR 0012), which
  static hosting cannot run.
- **Raw public Blob URLs as document URLs**: rejected within Vercel — Blob
  sets `Content-Disposition: attachment` on HTML, so it cannot be the serving
  path.

## Consequences

- Storage binds to Vercel Blob's limits: 100 stores (Hobby) / 500 (Pro), 20
  simple ops/s on Hobby — publish bursts need backoff and the design stays on
  simple operations only.
- Hobby pauses features instead of billing overage and is not viable for
  sustained 1M views/month (100 GB transfer, 1M Edge Requests); the beta→Pro
  upgrade is a launch gate, not an option.
- Takedown propagation is bounded by CDN TTLs; the takedown flow from ADR
  0015 must delete the ID object and purge CDN content.
- The format stays portable (PRD Constraint 7): provider lock-in exists only
  at the service level — the stored object is a normal `.md.html` — so a
  future move to R2 or another provider is a service migration, not a format
  change.
- Before launch, re-verify Edge Request/egress accounting for cache hits,
  plan binding, region choice (`iad1` vs `gru1`), and same-region Blob
  egress (Tech Spec "Open questions").
- Dependent Tech Spec Phases and Tasks: Phase 4 — "Implement Blob storage
  layout and short-ID/long-URL resolution" and the `POST /v1/documents` task
  that stores after build/audit; deployment plan step 3 — "Deploy the shared
  Vercel project with Publish API, Blob storage, and CDN routes"; Phase 7 —
  observability of storage usage and egress.
