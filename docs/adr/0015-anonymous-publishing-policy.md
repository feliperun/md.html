---
type: ADR
id: "0015"
title: "Anonymous publishing policy"
status: proposed
date: 2026-08-22
---

## Context

The hosting MVP promises gist-like publishing (`mdhtml publish` → short URL),
and frictionless publishing is a core product objective (PRD §26). Public
anonymous hosting of rendered documents nonetheless attracts phishing, spam,
malware distribution, illegal content, SEO abuse, storage abuse, and automated
mass publishing (PRD §27), and the PRD delegated the final policy and exact
limits to the Tech Spec ("do not require accounts unless operational/security
analysis shows that anonymous publishing is impractical"). Research brief
`docs/research/f-abuse.md` threat-modeled the anonymous surface: the
safe-by-default format already removes the code-execution abuse classes, and
the residual content-level abuse (phishing text, illegal content, automated
mass publishing) is manageable with size caps, per-IP rate limits, `noindex`,
report-driven takedown, and hash deny lists. The Tech Spec sections
"Rate limiting" and "Abuse controls" record the resulting decision, grounded
in PRD §25 (immutable anonymous documents) and §26 (anonymous MVP publishing).

## Decision

**Anonymous, immutable, public, no-custom-slug publishing with report-driven
takedown and hash deny lists; no accounts or authentication in the MVP.**
Per the Tech Spec ("Rate limiting", "Abuse controls", "API contract"):

- `POST /v1/documents` requires no authentication; anonymous publishing is
  the MVP default.
- Documents are immutable: publishing creates a permanent ID, updates create
  new documents, and identical sources deduplicate to the existing object.
- Public by default: no private, unlisted, or password-protected documents
  and no custom slugs in the MVP.
- Size limits: 2 MiB source upload and 8 MiB built artifact, enforced before
  parsing and again on the built artifact.
- Publish rate limits per IP (IPv6 counted per /64): 10/min, 50/hour,
  200/day, with at most 2 concurrent in-flight publishes; an optional 500 MiB
  lifetime storage per IP backstop. Views are not rate-limited at MVP — CDN
  caching is the view-side control.
- All anonymous documents default to `noindex, nofollow`, served with
  `X-Robots-Tag: noindex, nofollow, noarchive` (matching meta robots) plus
  `robots.txt` `Disallow: /` on the user-content origin.
- Takedown flow: deny-list check → delete object → purge CDN → append source
  hash to deny list → log; hash deny lists block republication of the same
  source.
- Documents using `fonts.url` are rejected by hosting at MVP (viewer-IP
  leakage and per-document CSP complexity).

## Options considered

- **Anonymous publishing with rate/size limits and abuse controls** (chosen):
  matches the frictionless objective; `docs/research/f-abuse.md` concluded
  the residual abuse surface is manageable, and mass publishing — the
  highest-probability technical abuse — has concrete MVP mitigations.
- **Require accounts or API tokens for publishing**: rejected for MVP — PRD
  §26/§31 keep accounts out unless anonymous publishing proves impractical;
  authentication adds friction and infrastructure while phishing and illegal
  content occur with accounts too. Future `mdhtml login` remains possible;
  the architecture must not prevent it.
- **Mutable documents (`PUT` semantics)**: rejected — immutability (PRD §25)
  removes edit authentication and versioning, and enables content-addressed
  dedup and aggressive immutable caching.
- **Custom slugs, private/unlisted documents, expiration**: rejected for MVP
  (PRD §28, §38) — future possibilities, not launch scope.
- **Proactive enforcement (captcha/proof-of-work) at launch**: deferred —
  triggered only if anonymous abuse spikes, per the open questions in
  `docs/research/f-abuse.md` and the Tech Spec.

## Consequences

- The MVP Publish API has no auth path; `mdhtml publish` works without login.
- Rate limiting, size caps, noindex headers, and the robots policy must ship
  with the hosting MVP, binding Tech Spec Phase 4 ("Implement
  `POST /v1/documents` with multipart source/assets, validation, and
  structured errors" and "Apply isolated origin, security headers,
  robots/indexing, and takedown controls") and Phase 7 ("Wire observability
  and budget/abuse alerts", which counts rate-limit events and takedowns).
- Hash deny lists and publish-time dedup depend on content addressing
  (ADR 0013); enumeration resistance depends on random public IDs (ADR 0014).
- The takedown/deny-list machinery itself is the abuse/takedown-model
  decision (PRD §46 item 13) and gets its own ADR; this ADR records only the
  anonymous-publishing policy.
- Future authenticated tiers (indexing opt-in, custom slugs, private
  documents) must remain additive without reopening this decision (Tech Spec
  rollout step 5).
