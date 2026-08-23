# Research F — Abuse Model for Anonymous mdhtml Hosting

Inputs: PRD sections 18, 22, 26–29, 41–44, 47–48; `docs/ARCHITECTURE.md`.
Scope: policy recommendations for the Tech Spec. This node defines the abuse
model; it does not implement rate limiting, moderation tooling, or takedown
infrastructure.

## Realistic abuse surface for this specific product

mdhtml documents are safe-by-default by construction: canonical server-side
build, no arbitrary author JavaScript, CSP `default-src 'none'`, no forms, no
arbitrary external requests, assets embedded as base64. That removes whole
abuse classes — no drive-by downloads, crypto-mining, keylogging, in-page
credential capture, popups, or JS redirects. The abuse surface that *remains*
is what a publisher can still do with static Markdown-rendered content:

- **Phishing / impersonation via content, not code.** A document can render
  arbitrary text, styled boxes, base64-embedded screenshots, and hyperlinks.
  A phisher can produce a pixel-faithful clone of a login or help page whose
  only interactive element is a link. CSP blocks the *document* from fetching,
  but **user-initiated top-level navigation on `<a href>` is not blocked by
  CSP** — malicious and lookalike links fully work. The platform's own short
  URLs carry reputation, so abuse hosted here is more credible to victims than
  the same content on a throwaway domain (the classic pastebin/GitHub Pages
  pattern).
- **Link-text vs href spoofing.** `[secure.example.com](https://evil.example)`
  renders the trusted text and navigates to the attacker host. The sanitizer
  must whitelist URI schemes (`https`/`mailto` and reject `data:`, `javascript:`,
  etc. — the PRD's "Unsafe URI scheme" diagnostic), but it cannot verify the
  host a link points to. No technical control short of link-scanning/deny
  lists removes this.
- **Redirect-style phishing without JS.** The document cannot auto-redirect,
  but a static "you will be redirected — click here" page with a link is
  equally effective social engineering.
- **Spam / SEO link farms.** Documents can contain many outbound links. Even
  unindexed, spam content is annoying and can carry UGC-style abuse; if it
  were indexed, the service would become a free SEO network. `noindex,
  nofollow` (section below) is the primary control; link destination deny
  lists are a secondary, optional hardening.
- **Malware distribution — reduced, not zero.** Raw JS payloads are blocked
  at build time and there is no file upload path: the publish API accepts
  Markdown source only and the server builds the artifact with the canonical
  implementation (PRD section 21, Option B). Residual surface: (a) documents
  that *instruct* victims to download malware from an external URL;
  (b) base64-embedded images carrying malicious content (e.g. weaponized
  images, CSAM, copyright material); (c) the platform being used as a
  "landing page" that links to attacker-hosted binaries. The `.md.html`
  artifact itself is not an executable vector; it is the *content* and the
  *links* that need moderation.
- **Illegal content.** Anonymous, permanent, unauthenticated publishing of
  CSAM, hate content, or copyright material is fully possible within the safe
  format. Detection relies on abuse reports plus hash-based deny lists; the
  content-addressed storage model makes hash deny lists unusually effective
  (see takedown section).
- **Automated mass publishing and publish-API DoS.** The publish API is an
  anonymous, unauthenticated compute path: each request validates, builds,
  and audits a document server-side. Without limits, a single IP can flood
  the API (CPU/function cost), fill storage, and generate unbounded egress.
  This is the highest-probability technical abuse and the one with concrete
  mitigations that must ship with the MVP.
- **Resource exhaustion via oversized documents.** Source size bounds build
  CPU/memory; base64 embedding inflates size by ~4/3 and images dominate
  artifact size. Without caps, one large document is cheap, but many large
  documents are a storage and egress problem (compounded by mass publishing).
- **Enumeration / bandwidth via short IDs.** Non-sequential random IDs
  (base58/base62, ≥ 8 chars ≈ 10^14 space) make enumeration impractical, and
  aggressive `Cache-Control: public, max-age=…, immutable` (PRD section 43)
  means a document view costs zero origin compute. Residual risk: crawlers
  or scanners probing random URLs generate CDN misses; caching short-ID
  resolution mitigates; alert on elevated miss rates.
- **Viewer-IP leakage via `fonts.url`.** Documents that relax the CSP for a
  declared `fonts.url` (marked `data-mdhtml-portable="false"`) cause viewer
  browsers to fetch that origin, disclosing viewer IPs to the font host.
  This is the only in-document network capability the format allows. The
  hosted service should either reject `fonts.url` documents or restrict the
  origin to an allowlist; at minimum these documents need an explicit
  privacy note.
- **Reputation damage and platform liability.** Even with no technical
  exploit, a well-known domain hosting phishing pages is itself the harm:
  takedown latency, brand damage, and potential domain-blocklisting. This
  justifies the moderation and takedown machinery even though the format is
  safe-by-default.

Deliberately *not* in scope as threats (blocked by design): arbitrary script
execution, forms/data exfiltration from the document, iframe-based attacks,
popups, auto-downloads, and cross-origin access to admin/application state —
the latter is additionally contained by serving user content on an isolated
origin (`docs.example` vs `mdhtml.example`, PRD section 22).

## Recommended limits

Concrete starting points; the Tech Spec validates exact numbers against the
hosting provider's current plan limits (Agent E's research) and tunes after
launch telemetry (PRD section 41).

| Limit | Recommended value | Rationale |
|---|---|---|
| Source upload size | **2 MiB** (2,097,152 B) per document | Bounds server-side parse/build/audit CPU and memory; generous for technical documents with embedded images; keeps base64 inflation predictable. |
| Built artifact size | **8 MiB** per document | Source 2 MiB → embedded assets inflate ~4/3 plus runtime/fonts; 8 MiB artifact cap prevents pathological image-heavy docs from dominating storage and CDN egress. Reject at publish time with a structured error naming the offending category (content/runtime/fonts/images). |
| Publish rate (per IP; per IPv6 /64) | **10 publishes/min burst, 50/hour, 200/day**; max **2 concurrent in-flight publishes** | Anonymous API is a compute path; bounds CPU, storage growth (~400 MiB/day worst case per IP at the 2 MiB cap), and mass-publishing. CI/agents burst within the hourly window; retry with `Retry-After` on 429. |
| Publish dedup | Content-hash dedup: identical canonical source returns the existing URL/object | PRD section 24. Same content re-published N times costs one object — removes the storage-abuse-by-duplication vector and makes hash deny lists effective. |
| Per-IP lifetime storage | Optional hardening: **500 MiB lifetime per IP**, enforced at publish | Backstop for NAT-rotation and distributed abuse; revisit if it hurts the agent/CI use case. |
| Global storage budget | Alerts at **50% / 80% / 95%** of plan quota; hard stop at 100% | Free hosting must fail safe on cost (PRD section 42); a hard stop beats surprise bills. |
| Egress/bandwidth | CDN-cached views; plan-level budget alerts at **50% / 80% / 95%**; alert on single-doc outlier popularity | Document views should cost zero origin compute (PRD section 42); the cost surface is CDN egress, monitored, not per-view metered. |
| Short-ID length/space | Random, non-sequential; base58/base62, **≥ 8 chars** | Makes enumeration-at-scale impractical (~10^14 space); no sequential IDs (PRD section 23). |
| Rate-limit events | Count and log `429`s and takedowns (PRD section 41) | Feeds abuse-detection thresholds and shows whether limits need tightening. |

Operational notes:

- Enforce the size cap on the upload *before* any parsing, and again on the
  built artifact — the artifact is what actually lands in object storage.
- Rate limits live in front of the publish API (not the CDN); document
  *views* are not rate-limited at MVP — CDN caching is the view-side control.
- Do not log document contents (PRD section 41); log IDs, sizes, hashes,
  IPs (short retention), and error codes only.

## SEO/indexing policy and rationale

**Policy:** all anonymous documents default to `noindex, nofollow`:

- `X-Robots-Tag: noindex, nofollow, noarchive` set at the CDN/edge for every
  served document object (header, not just in-body meta — it survives HTML
  parsing quirks and applies even to cached copies).
- A matching `<meta name="robots" content="noindex, nofollow">` in the served
  page for defense in depth.
- `robots.txt` with `Disallow: /` for the user-content origin, so crawlers do
  not even discover document IDs.

**Rationale** (PRD section 29):

- Removes the SEO value of the service, collapsing the spam-link-farm and
  keyword-stuffing incentive that makes free hosting attractive to spammers.
- Reduces phishing value: a takedown is moot if a phishing page already ranks
  in search results and is indexed in caches; unindexed content has a much
  smaller blast radius.
- Avoids becoming a free SEO-hosting network — the platform does not want to
  subsidize outbound PageRank or long-tail rankings for anonymous,
  unverified content.
- Nothing is lost for the primary audience: developers and agents share
  documents by URL, not by search engine discovery. If indexing is wanted,
  a future authenticated/verified tier can opt in per document, which
  attaches reputation to the indexed content.

`nofollow` matters independently of `noindex`: it prevents the platform from
passing link equity to arbitrary external targets even before (or without)
removal, and it signals to link-farm operators that links here are valueless.

## Privacy stance (public-by-default, no secrecy implied by URL)

Policy (PRD section 28):

- **Public by default.** Every published document is world-readable at its
  URL. The hosting website and the publish-API response must state this
  plainly: "Anyone with the URL can view this document; do not publish
  private or personal data." No unlisted/private/password-protected modes in
  MVP.
- **An unguessable URL is not secrecy.** Short IDs are random, but they are
  not access control. Documents are discoverable by anyone who receives or
  guesses the URL, and they may be linked, shared, archived, or mirrored
  without the publisher's control. The platform must never describe the
  service in terms that imply otherwise.
- **No content logging.** Operationally, contents are not logged (PRD
  section 41); telemetry covers publish counts, sizes, hashes, error codes,
  rate-limit events, and takedowns — not document bodies.
- **Viewer privacy.** Serving is passive static-object + CDN; the platform
  does not track views per user. Caveat: `fonts.url` documents leak viewer
  IPs to the declared font origin (see abuse surface) — the hosted service
  should reject `fonts.url` or restrict it to an allowlist, and any
  relaxation must carry a visible privacy note.
- **Repo rule alignment.** No personal or production-derived data in
  fixtures, tests, or docs — applies to the hosting service's synthetic test
  data as well.

## Abuse reporting mechanism

- **Public report surface.** A report form on the *application/website*
  origin (`mdhtml.example/report` — never on the user-content origin), plus
  a documented abuse contact. Fields: document URL, category
  (phishing/spam/malware/illegal/other), optional contact email, optional
  evidence description. No account required; reporting must be frictionless
  for victims.
- **Report rate limiting.** Cap reports per IP (e.g., 5/hour) with
  validation of the submitted URL (must be a real document ID on the
  user-content origin) — report flooding is itself an abuse vector.
- **Processing queue.** Reports land in a small, private review queue on the
  admin origin, separate from document storage; an operator reviews and
  acts (see takedown). Track report → action latency; alert if the queue
  grows or nothing is processed.
- **Feedback loop.** Where an email was provided, send a minimal
  confirmation/outcome notice (no content, no internal detail). Structured
  takedown counts feed observability (PRD section 41).
- **Proactive detection (minimum viable).** Hash-based deny list checked at
  publish time (see takedown) for known-bad content that has already been
  identified. Full proactive scanning (image hashing against known CSAM
  databases, e.g. PhotoDNA/NCMEC-style) is a legal/partnership decision —
  listed in Open Questions; do not block MVP on it.

## Administrative takedown mechanism for content-addressed immutable documents

"Immutable" means immutable by the publisher, not impossible for the service
operator to remove (PRD section 27). Content addressing is an asset here, not
an obstacle — it gives the operator a stable, exact fingerprint of the
offending content.

Mechanism, in order of operation:

1. **Origin deny check on the serving path.** Before serving, resolve the
   short ID through a deny list keyed by **content hash** (and short ID).
   Denied documents return `404` (opaque; avoids signaling detail to
   scanners) with a small, generic body. The check is cheap (hash lookup)
   and works for the whole lifespan of an object.
2. **Delete the object.** Remove the artifact from object storage and purge
   the CDN cache entry so cached copies do not survive the takedown
   (`Cache-Control: immutable` means browsers and CDNs will hold copies —
   purge is mandatory).
3. **Hash-level deny list blocks re-publication.** Because publication is
   content-addressed (same canonical source → same SHA256), any future
   attempt to publish the same content is rejected at publish time with the
   same opaque error — the takedown survives deletion and deduplication.
   This is the core reason content addressing *helps* takedown rather than
   hindering it.
4. **Admin isolation.** Takedown tooling and the deny list live on the
   admin/API origin (`mdhtml.example`), separate from the user-content
   origin — no admin state is reachable from served documents (PRD
   section 22).
5. **Process and logging.** Takedowns are manual, human-signed actions on
   reviewed reports (or confirmed illegal content): review → delete object →
   purge CDN → add hash to deny list → log the event (takedown counter in
   observability). A short public policy page documents what the platform
   will remove and how to report. Keep the deny list append-only and small;
   it is metadata, not content.
6. **Takedown of the URL, not of future edits.** Documents cannot be edited;
   a takedown is permanent for that hash. If a publisher wants a corrected
   version, they publish a new document with a new hash — the old one stays
   down. This is consistent with the immutability contract and keeps the
   model simple.

Boundary: this is the *capability* the Tech Spec must specify; operator
process, jurisdiction, and policy wording (what constitutes illegal content
vs policy violation) are product/legal decisions, not engineering scope.

## Open questions

- **Rate-limit tuning.** Are 10/min–200/day per IP right for the agent/CI
  audience, given shared egress IPs (GitHub Actions, cloud NATs) will be
  pooled? Should IPv6 be counted per /64? Confirm against real publishing
  telemetry after launch.
- **`fonts.url` on the hosted service.** Reject outright, allowlist
  origins, or allow with a privacy notice? This is the only network
  capability in the format and it leaks viewer IPs.
- **Proactive illegal-content detection.** Is PhotoDNA/NCMEC-style image
  hashing in scope/cost for the MVP, or is report-driven takedown the
  honest minimum? Jurisdiction of the operator matters here.
- **404 vs 410 and tombstone vs hard delete.** Opaque 404 for everything
  avoids signaling; 410 communicates deliberate removal. Does the deny
  list need to persist forever, or can non-illegal takedowns expire?
- **Indexing opt-in.** If a verified/authenticated tier arrives, what is
  the reputation signal (email, GitHub login, API token) that unlocks
  indexing without recreating the SEO-abuse surface?
- **Progressive enforcement.** If anonymous abuse spikes, do we add
  captcha, proof-of-work, or short-lived tokens to the publish path — and
  what thresholds trigger it without breaking the frictionless UX?
- **Deny-list scope.** Link-destination deny lists for phishing domains
  (shared blocklists) — worth it at MVP, or only after report volume
  justifies it?
- **Provider ceilings.** Exact size/request/egress numbers must be
  re-validated against the hosting provider's current plan limits
  (Agent E's research); the values in this document are starting points.
