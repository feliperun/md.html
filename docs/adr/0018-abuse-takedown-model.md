---
type: ADR
id: "0018"
title: "Abuse and takedown model"
status: proposed
date: 2026-08-22
---

## Context

PRD §27 requires an explicit abuse model for public anonymous hosting:
phishing, spam, malware distribution, illegal content, SEO abuse, storage
abuse, automated mass publishing, and denial of service are expected threats,
and the system MUST support administrative document removal even though
public documents are conceptually immutable. PRD §28 sets the privacy stance
(public by default, no secrecy implied by the URL), and PRD §29 requires an
explicit indexing decision, recommending `noindex, nofollow` for anonymous
documents. Safe-by-default construction removes the code-execution abuse
classes, so the residual surface is content and link abuse plus publish-API
resource exhaustion (docs/research/f-abuse.md). This ADR records the abuse
model decided in the Tech Spec's "Rate limiting" and "Abuse controls"
sections, on top of the anonymous publishing policy (ADR 0015) and the
content-addressed, immutable storage model (ADR 0013).

## Decision

**Anonymous, immutable, public, no-custom-slug publishing with report-driven
takedown and hash deny lists — frictionless publishing while closing the
highest-probability abuse classes** (Tech Spec "Abuse controls"):

- **Size limits**: 2 MiB source upload and 8 MiB built artifact; the cap is
  enforced on the upload before parsing and again on the built artifact.
- **Publish rate limits**: 10 publishes/min, 50/hour, 200/day per IP, IPv6
  counted per /64, at most 2 concurrent in-flight publishes, and an optional
  500 MiB lifetime-storage-per-IP backstop. Views are not rate-limited at
  MVP; CDN caching is the view-side control.
- **Indexing**: all anonymous documents default to `noindex, nofollow`,
  served with `X-Robots-Tag: noindex, nofollow, noarchive`, a matching meta
  robots tag, and `robots.txt` `Disallow: /` on the user-content origin.
- **Privacy**: public by default, with no implied secrecy from an
  unguessable URL.
- **Reporting**: abuse reports are accepted on the admin origin only,
  rate-limited, feeding a private review queue.
- **Takedown**: deny check → delete object → purge CDN → append the source
  hash to the deny list → log. Hash deny lists prevent republishing the same
  source, and short-ID TTLs bound propagation latency (Tech Spec "Caching
  model").
- **`fonts.url` documents are rejected by hosting at MVP** because they leak
  viewer IPs and complicate per-document CSP.

## Options considered

- **Hash deny lists + report-driven takedown** (chosen): content addressing
  gives each takedown a stable, exact fingerprint that survives deletion and
  deduplication and blocks republication at publish time
  (docs/research/f-abuse.md).
- **Proactive illegal-content detection (PhotoDNA/NCMEC-style scanning)**:
  rejected for MVP as a legal/partnership decision beyond engineering scope;
  report-driven takedown with a publish-time hash deny-list check is the
  minimum viable detection (docs/research/f-abuse.md).
- **Allowing indexing now or per-document opt-in**: rejected — it would turn
  the service into a free SEO-hosting network and increase the phishing blast
  radius; indexing opt-in is deferred to a possible future authenticated tier
  (PRD §29, docs/research/f-abuse.md).
- **Rate-limiting document views**: rejected — immutable objects served with
  `Cache-Control: immutable` cost zero origin compute per view; the real cost
  surface is publish-side compute and CDN egress, covered by the publish
  limits and budget alerts (docs/research/f-abuse.md).
- **`fonts.url` allowlist or allow-with-privacy-note**: rejected for MVP —
  `fonts.url` is the format's only network capability and leaks viewer IPs;
  outright hosting rejection is the simplest fail-safe.
- **Link-destination deny lists for phishing domains**: deferred — optional
  hardening to adopt only where report volume justifies it (PRD §27).

## Consequences

- A takedown is permanent for that source hash; a corrected document is a new
  publish with a new hash, and the old one stays down. The deny list is
  append-only metadata, kept small.
- Takedown tooling and the deny list live on the admin origin, isolated from
  the user-content origin (ADR 0016); denied documents get an opaque response
  that signals nothing to scanners.
- The publish API must enforce the size and rate limits, and the delivery
  path must apply the robots headers — launch-blocking controls, not
  telemetry.
- Takedowns are logged and counted in observability (PRD §41).
- Tech Spec Phase 4 item "Apply isolated origin, security headers,
  robots/indexing, and takedown controls", Phase 5 item "Document hosting
  website and public privacy/takedown policy", and Phase 7 item "Wire
  observability and budget/abuse alerts" depend on this decision; PRD §56's
  "documents can be administratively removed" criterion is satisfied by it.
- Retained as open by the Tech Spec, not decided here: 404 vs 410 and
  deny-list expiry for non-illegal takedowns, progressive enforcement
  (captcha/proof-of-work) if anonymous abuse spikes, and the future indexing
  opt-in reputation signal.
