---
type: ADR
id: "0011"
title: "Hosting architecture"
status: proposed
date: 2026-08-22
---

## Context

The PRD requires an official hosting service that is simple, inexpensive to
operate, agent-friendly, and abuse-resistant (PRD §18), prescribes a preferred
initial shape — Publish API → object storage → CDN — with Vercel as the initial
deployment target pending verification of pricing, limits, and security
characteristics (PRD §19), and mandates that a document MUST NOT require an
individual Vercel deployment: documents are stored as immutable objects and
served through a shared hosting application (PRD §20). A document view should
ideally require zero server-side compute after publication (PRD §42, §43).

Research brief `docs/research/e-hosting.md` verified Vercel pricing and limits
(Hobby pause behavior, Blob's `Content-Disposition: attachment` on HTML, Edge
Request and egress accounting) and costed the recommended architecture at 1k /
10k / 100k documents and 1M views/month. The Tech Spec's "Hosting
architecture" section records the resulting decision.

## Decision

**Vercel is the MVP hosting provider, deployed as one shared application —
publishing never creates a per-document deployment.** The Publish API runs in
a Vercel Function and invokes the pinned mdhtml CLI compiled to `wasm32`.
Immutable artifacts are stored in a private Vercel Blob store and served
through the docs origin; raw public Blob URLs are never used as document URLs
because Blob forces `Content-Disposition: attachment` on HTML.
`/d/<toolchain>/<sha256>` is the content-addressed long URL and `/<shortId>`
resolves once via a cached 308. Steady-state views are CDN-served with zero
server-side compute. Vercel Pro is the realistic launch floor after a
Hobby-only beta.

## Options considered

- **One shared Vercel project + Blob + CDN** (chosen): matches the PRD §19/§20
  architecture; `docs/research/e-hosting.md` estimates ~$0–1/month at 1k–10k
  documents and ~$20–24/month on Pro at 100k documents / 1M views/month, with
  the Pro platform fee as the dominant cost.
- **Per-document Vercel deployments**: rejected — explicitly forbidden by PRD
  §20; consumes a deployment per document and makes views deployment-bound
  instead of cache-bound.
- **Raw public Vercel Blob URLs as document URLs**: rejected — Blob sets
  `Content-Disposition: attachment` on HTML (the browser downloads instead of
  rendering) and blob URLs are not custom-domainable, breaking short public
  URLs and origin isolation (`docs/research/e-hosting.md`, Vercel-specific
  risks).
- **Cloudflare R2 + Workers/Pages**: rejected for MVP — zero egress is
  attractive but Vercel already fits the budget at the expected scale, and a
  second platform model adds operational complexity contrary to PRD §44;
  retained in the research as the documented fallback if Vercel limits bite.
- **Fully static hosting (e.g., GitHub Pages) as the official service**:
  rejected — no serverless compute means no Publish API and no server-side
  canonical build; static hosting remains a supported third-party destination
  (PRD §36), not the official host.

## Consequences

- Publishing a document never triggers a deployment; only code changes deploy
  the shared application.
- The canonical Rust CLI must remain compilable to `wasm32`, because the
  Publish API invokes the pinned toolchain server-side. This binds Tech Spec
  Phase 4 — "Publish the canonical toolchain as pinned WASM", "Implement
  `POST /v1/documents`", "Implement Blob storage layout and short-ID/long-URL
  resolution", and "Apply isolated origin, security headers, robots/indexing,
  and takedown controls" — and is consumed by Phase 5 ("Implement
  `mdhtml publish <source>`") and Phase 7 observability/budget alerting.
- Zero-compute steady-state views depend on the caching model (immutable
  `max-age=31536000` content objects, cached 308 short-ID resolution); CDN
  cache TTLs bound takedown propagation latency, so takedown must purge.
- The service launches as a beta on Vercel Hobby with explicit limits and
  alerts, then upgrades to Pro before sustained anonymous traffic (Hobby
  pauses instead of billing overage).
- Storage-provider specifics, the upload model, content addressing, ID
  generation, and the isolated user-content origin are recorded as separate
  decisions per PRD §46; this ADR fixes only the overall service shape.
