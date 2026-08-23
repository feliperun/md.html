# Research: Vercel hosting for immutable mdhtml documents (Agent E)

Research node output for the hosting layer of `mdhtml-safety-hosting`. Scope: PRD sections 18–25, 39, 42–44, 47–49, and 56 (especially §20 no per-document deployment, §21 upload model, §23 public URL design, §24 content addressability, §25 immutable publishing, §42 zero-compute views, §43 caching, §48 Agent E).

Method: I read `docs/prd/mdhtml-safety-hosting-prd.md`, `docs/ARCHITECTURE.md`, and `docs/VISION.md`, and verified Vercel pricing/limits from the public Vercel docs (browsing worked from this environment). Every figure sourced from Vercel docs is marked **verified 2026-08-22** with the source URL; every other number is an **assumption** and is flagged as such. Nothing here provisions infrastructure; the Tech Spec (synthesis node) owns the final decision.

## Recommended architecture

One shared hosting application, no per-document deployments. Documents live as immutable objects in object storage; the only serverless compute is a thin, cacheable resolution/serve path that runs on CDN cache misses. Steady-state views are served from the CDN.

```
 CLI / Agent ──POST /documents─▶ Publish API (Vercel Function)
                                     │  rate limit, validate, build (canonical
                                     │  toolchain via WASM), audit, hash
                                     ▼
                        ┌───────────────────────────────┐
                        │  Vercel Blob (private store)  │
                        │  docs/{toolchain}/{sha256}    │  immutable .md.html
                        │  sources/{sha256}             │  canonical Markdown
                        │  ids/{publicId}               │  id → {toolchain, sha256}
                        └───────────────────────────────┘
                                     │  (only on cache miss)
                                     ▼
                docs.example/<shortId> ──▶ Vercel Function (resolve, 308)
                                     │
                                     ▼
                docs.example/d/<toolchain>/<sha256> ──▶ CDN (Vercel edge cache)
                                     │
                                     ▼
                             immutable, cacheable, zero server compute
```

Key properties:

- **docs.example is an isolated origin** (§22): a dedicated registrable domain with no authentication cookies, no admin session, no application localStorage; strict CSP and security headers applied via platform headers config. User HTML never executes server-side.
- **`/d/<toolchain>/<sha256>` is the canonical long URL**: a stable, content-addressed object URL that is cached with `immutable` semantics. The short URL is a durable alias that resolves to it once and is itself cacheable.
- **Deployments only for code changes**: the app is a single Vercel project. Publishing a document never creates a deployment (§20).
- **Portability is preserved** (Constraint 7, VISION: hosting optional): the served object is still a normal self-contained `.md.html` that works from `file://`; the hosted copy is a convenience, not a dependency.

## Upload model: Option A vs B vs C, and recommendation

**Option A — upload the built `.md.html` artifact.**

- Pros: no server build; publish compute ≈ 0; client can build locally; simplest API.
- Cons: the server cannot prove the artifact was produced by the canonical mdhtml toolchain. A compromised or modified client can upload arbitrary HTML and claim mdhtml provenance — exactly what §21 forbids. Content addressing would be keyed on the artifact, not the canonical source (§24), and the audit step would run on unverifiable input. The security requirement fails.
- Verdict: **rejected.**

**Option B — upload the canonical Markdown source, build server-side.**

- Pros: the server runs the canonical toolchain (the CLI compiled to WASM) and audits the output before publishing, so provenance is guaranteed by construction; a compromised client can only upload Markdown, which the server then validates, builds, and audits. Content addressing falls out naturally: `SHA256(canonical source)` (§24). Determinism and reproducibility are enforceable by pinning the toolchain version (§52). Client is minimal (a single upload).
- Cons: publishes consume server compute (small: ~0.5–2 s per doc) and require the toolchain to be embeddable (WASM build — already aligned with the project's portable runtime philosophy). Build cost scales with publish volume, which is trivially small at the scales estimated below.
- Verdict: **recommended.**

**Option C — upload both source and artifact, verify correspondence.**

- Pros: in principle, the server can compare the uploaded artifact against a canonical build of the source.
- Cons: a correspondence check that adds trust requires the server to rebuild — which is exactly Option B's cost, plus the upload size of Option A. Any weaker check (hash comparison against a client-supplied claim) proves nothing against a compromised client. C is "belt and suspenders" that cannot add trust beyond B: the client's artifact claim is untrustworthy by assumption.
- Verdict: **rejected** as the primary model. One B-compatible optimization stays: build results are cached keyed by source hash, so re-publishing an already-built source skips the rebuild (automatic dedup, §24).

**Recommendation: Option B**, with a pinned, versioned canonical toolchain and a transactional publish pipeline: validate → build → audit → hash → store → create ID. Custom assets are build inputs (they participate in the source hash), and the toolchain version is recorded per document, satisfying §21's "deterministic builds, runtime versions, custom assets, compatibility" considerations.

## Public ID scheme

Recommendation: **random NanoID-style ID, 12 characters from the URL-safe alphabet `[A-Za-z0-9_-]` (64 chars ≈ 6 bits/char ⇒ 72 bits), generated server-side with a CSPRNG** (`crypto.getRandomValues` on the server runtime; no sequential counter).

- **Short**: 12 chars (PRD example `H7zPm` is 5; 12 is a pragmatic floor for collision margin while staying copyable).
- **Durable**: never reused, never recycled; immutable documents keep the ID forever (§25).
- **Hard to enumerate**: random IDs make sequential enumeration impossible; there is no meaningful order to guess (§23).
- **Collision safety**: expected collisions at 100k live IDs ≈ (10^5)² / 2^73 ≈ 10^-12 — effectively zero; even at 10M docs ≈ 10^-8. On the (astronomically rare) collision, regenerate. Note: 10 chars would only give 60 bits (~4×10^-9 at 100k) — acceptable but 12 chars buys 1000× margin for free.
- **Case sensitivity**: base64url is case-sensitive. If product decides case-insensitive copyability wins (§23 "case-insensitive if practical"), use 14 lowercase chars from a 32-char alphabet (Crockford/base32, ~70 bits) — same entropy, slightly longer. Decision belongs to the Tech Spec; default is the 12-char base64url ID.
- **Not content-derived**: public IDs are independent of content (see below); the short ID is an opaque alias. This avoids leaking content equality through URLs and keeps IDs stable across toolchain upgrades.

## Storage layout and content addressing

Single private Vercel Blob store, three key spaces:

```
sources/{sha256(source)}        # canonical Markdown, immutable
docs/{toolchainId}/{sha256}     # built .md.html, immutable per toolchain
ids/{publicId}                  # { "sha256", "toolchainId", "createdAt" }  (~250 B)
```

- **Content addressing**: the canonical source is the addressable unit (`SHA256(canonical source)`), per §24. Identical sources dedup automatically — the second publish of the same source with the same toolchain returns the existing object.
- **Toolchain versioning**: the built artifact must not be overwritten when the toolchain changes (immutability, §25). Keying the artifact by `docs/{toolchainId}/{sha256}` keeps old short IDs serving the exact bytes they were published with, while a re-publish of the same source under a new toolchain creates a fresh artifact and a fresh short ID. `toolchainId` = hash of the pinned toolchain binary + build configuration. If the toolchain is frozen for MVP, this degenerates to `docs/{sha256}`.
- **Public-ID lookup**: `ids/{publicId}` is a tiny JSON object; resolution is a single Blob read. Because IDs never change, the resolution response is aggressively cacheable (next section).
- **Lookup direction**: public ID → hash is a separate lookup (as required); the content object is never addressable by public ID directly, so hash-keyed objects are effectively unguessable.
- **Integrity**: serving by `sha256` plus a `Digest: sha-256=...` header (where platform headers config allows) gives clients a verification path; the Tech Spec should decide whether to add this.

## Caching strategy

All published documents are immutable (§25), which makes caching trivial (§43):

- **Content objects** (`/d/<toolchain>/<sha256>`): `Cache-Control: public, max-age=31536000, immutable` with `s-maxage=31536000`. Vercel's proxy supports `s-maxage` up to 31,536,000 s (1 year) — **verified 2026-08-22** (vercel.com/docs/caching/cache-control-headers). After the first fetch, zero server-side compute per view (§42).
- **Short-ID resolution** (`/<shortId>` → 308): cacheable because IDs are immutable — `Cache-Control: public, max-age=3600, s-maxage=86400`. The 308 is cheap and the CDN absorbs repeat resolutions; browsers revalidate at most hourly.
- **`stale-while-revalidate` / `stale-if-error`**: consumed by Vercel's proxy — **verified 2026-08-22**. Use them on the resolution and content routes to ride out transient origin failures and keep the service "simple, recoverable, inexpensive" (§44).
- **Effective result**: with ~90–97% CDN hit rate on public immutable content, only first-touch (and revalidation) requests reach the origin Function; steady-state view traffic is CDN egress only.
- **Takedowns** (§56): deleting `ids/{publicId}` breaks resolution immediately; content bytes may remain in CDN/Blob until cache TTL/purge. Document this bound in the Tech Spec and evaluate a purge step for abuse cases.

## Cost estimates at 1k / 10k / 100k docs and 1M views/month, with assumptions

All prices are Vercel Blob/compute/CDN at `iad1`, **verified 2026-08-22** from vercel.com/docs (pricing pages for plans, Blob, Fluid compute, data transfer, and cache-control docs). Regional note: São Paulo (`gru1`) Fluid compute is higher ($0.221/CPU-hr vs $0.128/CPU-hr, and $0.0183/GB-hr memory) — **verified 2026-08-22**; all estimates use `iad1`.

**Assumptions (all flagged, must be re-validated at Tech Spec time):**

- Average built artifact: **150 KB** (assumption; range 50–450 KB against the release budget). Average stored document incl. source + id metadata: **155 KB**.
- Publish volume ≈ stored doc count for the compute column (worst case: 1k docs = 1k publishes in the month).
- Server build per publish: **1 s active CPU** (assumption; 0.5–2 s range), 128 MB provisioned memory.
- Views: **1M/month**; CDN hit rate **95%** (assumption; 90–97% sensitivity below).
- Cache hits count toward Fast Data Transfer and Edge Requests (assumption — **open question**, see last section).

**Storage (cumulative, per month):** `N × 155 KB × $0.023/GB`

| Docs | Stored | Blob storage cost |
|---|---|---|
| 1k | 0.155 GB | ~$0.004 (Hobby: $0, first 1 GB included) |
| 10k | 1.55 GB | ~$0.04 (Hobby: ~$0.01 after 1 GB free) |
| 100k | 15.5 GB | ~$0.36 |

**Publish compute (Option B, per month):** `N × 1 s / 3600 × $0.128/CPU-hr` + invocations `N × $0.60/1M` + Blob ops `~4 × N × $0.40/1M`

| Publishes/mo | Active CPU | CPU cost | Invocations | Blob ops |
|---|---|---|---|---|
| 1k | 0.28 CPU-hr | $0.04 (Hobby 4 CPU-hr included → $0) | $0.001 | $0.002 |
| 10k | 2.8 CPU-hr | $0.36 (inside Hobby 4 CPU-hr, tight) | $0.01 | $0.02 |
| 100k | 27.8 CPU-hr | ~$3.6 (Hobby would pause; Pro credit covers) | $0.06 | $0.16 |

**Views (1M/month):** misses = `1M × (1 − hit rate)`

| Component | Formula | Cost |
|---|---|---|
| Fast Data Transfer | `1M × 150 KB = 150 GB` | Hobby 100 GB limit → **pause risk**; Pro 1 TB → $0 |
| Edge Requests | ~1M views | Hobby limit 1M → **at-limit risk**; Pro 10M → $0 |
| Invocations (misses @95%) | `50k × $0.60/1M` | ~$0.03 |
| Blob reads (misses) | `50k × $0.40/1M` | ~$0.02 |
| Blob data transfer (misses) | `7.5 GB × $0.05/GB` | ~$0.38 |

**Bottom line:**

- **1k docs**: ~$0/month on Hobby (storage, build, and traffic all inside free tiers).
- **10k docs**: ~$0–1/month on Hobby (build ~2.8 CPU-hr is inside the 4 CPU-hr limit; watch the boundary if docs are large or publishes bursty).
- **100k docs**: ~$20–24/month on Pro (platform fee $20 + ~$0.4 storage + ~$3.6 build + ~$0.3 ops; all within the included $20 usage credit — effectively just the platform fee).
- **1M views/month**: ~$20–21/month on Pro ($20 platform + ~$0.5 traffic). On Hobby it is nominally $0 but exceeds the 100 GB transfer and sits at the 1M Edge Request limit → **Hobby is not viable for sustained 1M views/month**; Pro is the realistic floor.
- **Sensitivity**: at 90% vs 97% hit rate, miss traffic is 15 GB vs 4.5 GB — Blob egress swings from ~$0.75 (15 GB at 90% hits) to ~$0.23 (4.5 GB at 97% hits). At 450 KB docs, data transfer at 1M views is ~450 GB (still inside Pro 1 TB) and storage at 100k docs is ~$1.0/month. The dominant lever is the $20 Pro platform fee, not usage.

## Vercel-specific risks and limits

- **Hobby pauses instead of billing overage** — **verified 2026-08-22**: exceeding Hobby limits pauses the affected features for 30 days. A public, donation-supported service (§37) can't silently pause; treat Hobby as beta-only, with alerts (PRD §41/§42) and an upgrade path.
- **Hobby is personal/non-commercial** (fair use) — **verified 2026-08-22**. If the service takes donations or hosts third-party content, Pro is the safer legal/ToS posture; confirm at Tech Spec time.
- **Blob limits** — **verified 2026-08-22**: 100 stores (Hobby) / 500 (Pro); rate limits 20 simple ops/s and 15 advanced ops/s on Hobby; advanced operations bill at $5.00/1M (avoid by keeping the design to simple GET/PUT). Publishing bursts need client-side backoff/queueing.
- **Blob storage/egress model** — **verified 2026-08-22**: storage $0.023/GB-mo, Blob data transfer $0.05/GB, Fast Origin Transfer $0.06/GB, Simple Ops $0.40/1M (cache hits do not count toward ops). Reads from Functions are Blob ops + egress — another reason to keep the miss path small.
- **Edge Requests on Blob/public URLs**: every public blob access is an Edge Request (Hobby 1M/month) — **verified 2026-08-22**. Whether CDN cache hits on the custom domain consume Edge Requests must be confirmed; at 1M views this is the make-or-break limit for Hobby.
- **No per-document deployments**: the architecture above never deploys per document (§20); only code changes redeploy. This is also the main protection against deployment-quota pain.
- **Blob forces `attachment` for HTML** — **verified 2026-08-22**: Vercel Blob's public URLs set `Content-Disposition: attachment` on HTML content ("This also prevents hosting HTML pages on Vercel Blob"), so a raw `<store-id>.public.blob.vercel-storage.com` URL would download the file instead of rendering it in the browser. This is the hard reason the serving path must be the `docs.example` origin (Function streams the blob on cache miss, CDN caches the response) and raw public Blob URLs cannot be the document URL.
- **Blob URLs are not custom-domainable directly**: `*.public.blob.vercel-storage.com` URLs include store/path suffixes and signatures; serving bytes through the `docs.example/d/...` route (Function streams the blob on miss, CDN caches) keeps the public URL short and stable (§23). The Function must not execute user HTML and must set strict CSP/security headers — user content never shares an origin with privileged app state (§22).
- **Takedown latency**: cache TTLs bound takedown propagation (up to 1 year for content, 1 day for IDs); plan a cache purge for abuse cases (§56).
- **Open Source Program** — **verified 2026-08-22** (vercel.com/docs/open-source): exists, quarterly applications, requires active OSS + impact; **not guaranteed** — never budget on it.
- **Alternatives** (if Vercel's model or limits bite): Cloudflare R2 (zero egress) + Workers/Pages as the low-cost fallback; GitHub Pages (§36) for fully static hosting with no serverless at all. Not needed for the MVP estimates above.

## Assumptions and open questions requiring live pricing/limits verification before launch

Checked directly from Vercel docs on **2026-08-22** (plans page, Blob pricing, Fluid compute pricing, Fast Data Transfer/Edge Requests pricing, cache-control-headers docs, open-source docs). Items below are **assumptions** or **must-reverify** items; none are guaranteed:

1. **Edge Request accounting for CDN cache hits** — do cached responses on a custom domain count toward the Hobby 1M Edge Request limit? Determines whether Hobby survives 1M views.
2. **Data transfer accounting for cache hits** — does CDN-served cached traffic count toward Fast Data Transfer (assumed yes)? Determines the 100 GB Hobby ceiling.
3. **Compute plan model** — Fluid vs legacy compute on Hobby/Pro; the 4 CPU-hr Hobby figure and $0.128/CPU-hr Pro rate were verified, but the plan binding at launch must be re-confirmed.
4. **Blob egress from same-region Functions** — whether Function→Blob reads in `iad1` incur Blob Data Transfer/Fast Origin Transfer as modeled.
5. **Average document size** — 150 KB assumed; final number depends on the release budget (50–450 KB) and typical content. Re-run the tables at Tech Spec time.
6. **Build cost per publish** — 1 s active CPU assumed; measure the WASM build on representative fixtures before locking limits (§42 max document size and request limits).
7. **Publish volume vs stored docs** — the tables assume publishes ≈ stored docs per month; realistic growth curves change the compute column.
8. **Region** — `iad1` rates used; `gru1` (São Paulo) compute is ~1.7× and must be chosen for latency vs cost.
9. **Open Source Program grant** — existence verified; approval not assumed.
10. **Case-insensitive IDs** — product decision (§23 "if practical") that changes ID length; default 12-char base64url stands unless product picks the 14-char base32 variant.
11. **`Digest` header / integrity verification** — whether Vercel's headers config can emit `Digest: sha-256=...` on the content route; otherwise skip for MVP.
12. **Takedown purge mechanics** — exact purge API/TTL behavior for the shared deployment must be validated before the abuse controls (§27, §56) are speced.
