---
type: ADR
id: "0016"
title: "Isolated user-content origin"
status: proposed
date: 2026-08-22
---

## Context

PRD §22 ("Isolated Origin") requires that user-generated documents never share
the authentication/security origin of administrative applications: hosted
documents must have no access to authentication cookies, application
`localStorage`, privileged APIs, or admin sessions, so that a future sanitizer
bypass is contained. The Tech Spec's threat model rules same-origin access to
admin state out of scope by design, and its trust Boundary 4 (hosting delivery)
places user documents on a dedicated registrable origin served as immutable
static bytes with no access to admin credentials. The browser-security research
brief (`docs/research/d-browser-security.md`, "Origin isolation strategy for
hosting") established that only a real HTTPS origin enables this split and
weighed the alternatives below.

## Decision

**Serve hosted user documents from a separate registrable domain for user
content (`docs.example`) distinct from the app/API/admin origin
(`mdhtml.example`).** A different eTLD+1 shares no cookies (even
`Domain=`-scoped), no storage, and no same-origin API access with privileged
state.

Defense in depth on top of the dedicated origin, per the Tech Spec's
"Isolated-origin strategy" section:

- App cookies use `__Host-`, `Secure`, `SameSite=Strict`, never `Domain=`-scoped.
- The Publish API returns no CORS allowlist for the docs origin.
- COOP/CORP/`Origin-Agent-Cluster`/`Permissions-Policy` isolate the browsing
  context.

## Options considered

- **Separate registrable domain** (chosen): the strongest split — the document
  origin shares nothing with the app origin, including `Domain=`-scoped cookies
  (`docs/research/d-browser-security.md`, "Origin isolation strategy for
  hosting").
- **Subdomain of the same registrable domain** (`docs.mdhtml.example`): a
  different origin but still inside `Domain=` cookie scope; would force the app
  onto host-only `__Host-` cookies permanently. Strictly weaker; rejected.
- **CSP `sandbox` on the hosting origin**: rejected — applied to a top-level
  document it makes the origin opaque, breaking the secure context
  (`navigator.clipboard`), downloads, and `crypto.subtle`, per the same
  research brief. Isolation comes from the dedicated origin plus
  COOP/CORP/cookie policy, not `sandbox`.

## Consequences

- Deployment is bound to two domains; the Tech Spec's deployment plan step
  "Create the two domains (`mdhtml.example`, `docs.example`) and configure
  headers" implements this decision.
- Tech Spec Phase 4 item "Apply isolated origin, security headers,
  robots/indexing, and takedown controls" depends on this ADR; its exit
  criterion — hosted docs have no admin cookie/state access — verifies it.
- Cookie discipline (`__Host-`, `Secure`, `SameSite=Strict`, no `Domain=`
  scoping) becomes a standing constraint on the app origin, including any
  future authenticated tier.
- The Publish API must never grant the docs origin CORS read access.
- Admin surfaces (abuse reports, review queue, takedown tooling) stay on the
  app origin; the docs origin serves immutable static bytes only.
- Local `file://` artifacts and third-party static hosting are unaffected —
  their safety is a property of the artifact (PRD §36, Constraint 5).
- Status stays `proposed` until the Tech Spec's hosting phases implement it.
