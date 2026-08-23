# PRD — mdhtml Safe-by-Default & Public Hosting

Status: Draft
Project: mdhtml
Target: Post-v1.0
Repository: feliperun/md.html
Audience: Engineering agents responsible for technical specification, architecture, implementation, testing, security review, and documentation.

---

## 1. Executive Summary

mdhtml is a spec-first document format that turns Markdown into a single, self-contained `.md.html` artifact while preserving the exact original Markdown source byte-for-byte.

The project currently optimizes for four fundamental properties:

1. Human-readable: the HTML renders as a rich, customizable document.
2. Machine-readable: the canonical Markdown source remains embedded and recoverable.
3. Self-contained: runtime, fonts, CSS, assets, and source are contained in one file.
4. Portable: documents can be opened directly through `file://` without requiring a server.

The next phase introduces two closely related capabilities:

- Safe-by-default document generation
- Official public hosting for mdhtml documents

Security must not be implemented merely as a hosting concern.

A document produced by the official mdhtml toolchain should be safe by default regardless of whether it is:

- opened locally;
- attached to an email;
- published through GitHub Pages;
- served from S3, Vercel, Cloudflare, nginx, etc.;
- published through the future official mdhtml hosting service.

The fundamental design principle is:

> Safety is a property of the generated artifact, not a property of the hosting provider.

At the same time, mdhtml must preserve one of its defining capabilities:

> Authors must retain extensive control over the visual presentation of their documents using HTML and CSS supported by the mdhtml specification.

Security therefore MUST NOT be implemented by reducing mdhtml to a limited collection of predefined UI components, introducing a proprietary layout DSL, or otherwise changing Markdown into an application framework.

Instead, mdhtml must validate author-controlled HTML, CSS, URLs, embedded resources, and executable content and refuse to produce unsafe artifacts by default.

The second part of this project is a minimal public publishing service that allows humans, CLI tools, and AI agents to publish valid mdhtml documents and receive short public URLs.

Example:

```
mdhtml publish architecture.md
✓ built
✓ security audit passed
✓ published
https://<public-domain>/7Km2x
```

The hosting service should initially be free, extremely simple, inexpensive to operate, agent-friendly, abuse-resistant, and optionally supported through donations.

---

## 2. Problem

### 2.1 Documents increasingly have two consumers

Modern technical documents increasingly need to be consumed by both:

- humans;
- software agents / LLMs.

Markdown is excellent for:

- LLM context;
- version control;
- editing;
- diffs;
- portability;
- structured text.

HTML is excellent for:

- typography;
- visual hierarchy;
- layout;
- sharing;
- browser rendering;
- presentation.

Traditional publishing systems normally separate these concerns:

```
source.md
    ↓
renderer
    ↓
generated website / HTML
```

The generated artifact is no longer the canonical source.

mdhtml deliberately keeps them together.

---

## 3. Current mdhtml Philosophy

A generated artifact:

```
document.md.html
```

contains both:

```
canonical Markdown source
+
presentation/runtime
```

The canonical Markdown MUST remain recoverable exactly.

Conceptually:

```
mdhtml build document.md
        ↓
   document.md.html
        ↓
mdhtml extract document.md.html
        ↓
   document.md
```

with:

```
SHA256(original) == SHA256(extracted)
```

This invariant must remain unchanged.

---

## 4. New Problem Introduced by Custom Presentation

mdhtml intentionally allows documents to significantly customize their presentation.

A document may define presentation behavior associated with sections, containers, themes, and other structures.

This may involve author-controlled:

- HTML;
- CSS;
- selectors;
- attributes;
- links;
- URLs;
- images;
- embedded assets;
- potentially other constructs defined by the specification.

This creates a security boundary.

A malicious document could attempt to introduce:

- `<script>`;
- inline event handlers;
- `javascript:` URLs;
- malicious SVG;
- DOM mutation mechanisms;
- iframe injection;
- `<object>` / `<embed>`;
- external executable resources;
- CSS network requests;
- CSS-based UI spoofing;
- browser-specific parsing tricks;
- mutation XSS;
- malformed HTML designed to bypass naïve filters;
- future browser features capable of turning previously harmless markup into executable behavior.

This is already relevant for static local `.md.html` artifacts.

Public hosting only increases the consequences.

---

## 5. Product Principle: Safe by Default

The default behavior of:

```
mdhtml build
```

MUST produce a document that passes the mdhtml security policy.

Unsafe content MUST cause the build to fail.

Example:

```
$ mdhtml build malicious.md
✗ Security audit failed
MDHSEC001: executable event handler detected
  <img src="x" onerror="alert(1)">
               ^^^^^^^
Build aborted.
```

The CLI MUST NOT silently remove user content during a normal build unless the technical specification determines that normalization can be performed without changing author intent.

The preferred behavior is:

> Reject rather than silently mutate.

---

## 6. Unsafe Escape Hatch

Advanced users may intentionally need behavior prohibited by the safe profile.

The CLI SHOULD provide an explicit escape hatch.

Tentative interface:

```
mdhtml build document.md --unsafe
```

Exact naming MUST be evaluated in the Tech Spec.

Requirements:

- unsafe behavior MUST require explicit opt-in;
- it MUST never be the default;
- the CLI MUST display an appropriate warning;
- unsafe artifacts MUST be distinguishable where practical;
- the official hosting service MUST reject artifacts requiring unsafe mode;
- agents MUST NOT use unsafe mode unless explicitly instructed by the human user.

The Tech Spec must determine whether unsafe mode:

- disables all guards;
- selectively disables particular guards;
- or supports both models.

---

## 7. Security Architecture

Security MUST use defense in depth.

No single sanitizer, regex, CSP, parser, or browser behavior should be treated as sufficient.

Conceptually:

```
Markdown
   │
   ▼
mdhtml parser
   │
   ▼
renderer
   │
   ▼
generated HTML
   │
   ├──── HTML security validation
   │
   ├──── CSS security validation
   │
   ├──── URL/resource validation
   │
   ├──── executable-content validation
   │
   └──── runtime integrity validation
   │
   ▼
SAFE ARTIFACT
   │
   ▼
document.md.html
```

When hosted:

```
SAFE ARTIFACT
     │
     ▼
isolated origin
     │
     ▼
strict HTTP security headers / CSP
     │
     ▼
   browser
```

---

## 8. Security Guard Requirements — HTML

The implementation MUST investigate a mature HTML5-aware sanitizer/parser.

For Rust, Ammonia should be evaluated as the primary candidate.

The implementation MUST NOT rely on regex-based HTML sanitization.

The validator must identify dangerous constructs including, but not limited to:

- scripts;
- inline executable event handlers;
- dangerous URI schemes;
- iframe;
- object;
- embed;
- dangerous SVG constructs;
- executable foreign namespaces;
- HTML parsing edge cases;
- mutation-XSS vectors.

However, the final policy MUST be based on threat analysis rather than simply accepting a sanitizer library's defaults.

---

## 9. CSS

CSS customization is an important mdhtml feature and MUST remain available.

The project MUST NOT solve CSS security by simply eliminating arbitrary styling.

A CSS parser must analyze author-controlled CSS structurally.

A Rust implementation based on a real CSS parser SHOULD be preferred.

Candidate technologies include:

- lightningcss;
- css-sanitizer;
- another mature AST-based CSS parser/sanitizer identified during technical research.

The Tech Spec MUST explicitly evaluate the maturity and security properties of candidate libraries.

Potentially dangerous constructs include:

- network-capable `url(...)`;
- `@import`;
- external fonts;
- resource loading;
- browser-specific executable behavior;
- selectors capable of interfering with hosting chrome;
- CSS-based data exfiltration;
- deceptive full-page overlays;
- future network-capable CSS constructs.

The policy should distinguish:

```
visual customization
```

from:

```
execution / external communication / host interference
```

The objective is to preserve as much CSS capability as reasonably possible.

---

## 10. External Resources

mdhtml's existing philosophy is strongly self-contained.

The security model should reinforce this.

The Tech Spec must inventory every mechanism capable of initiating a network request, including:

- scripts;
- images;
- fonts;
- CSS URLs;
- imports;
- iframes;
- media;
- SVG references;
- forms;
- prefetch/preload;
- redirects;
- browser metadata;
- any other applicable HTML mechanism.

The preferred safe artifact should make zero unexpected network requests.

Any intentionally supported external-resource behavior must be explicitly documented.

---

## 11. JavaScript

The official mdhtml runtime is trusted code.

Author-provided JavaScript is not.

Safe mode MUST prevent arbitrary user JavaScript execution.

The build system knows the exact runtime that belongs to mdhtml.

This allows runtime integrity to become part of the security model.

The Tech Spec should evaluate:

- hashing the canonical runtime;
- CSP script hashes;
- Subresource Integrity where applicable;
- deterministic runtime generation;
- runtime integrity verification during mdhtml audit.

---

## 12. Content Security Policy

Generated artifacts and hosted documents should use the strictest CSP compatible with mdhtml functionality.

The Tech Spec must determine:

- what CSP can be embedded through `<meta http-equiv>`;
- what protections require HTTP headers;
- differences between `file://` and HTTP environments;
- how inline styles interact with CSP;
- how the embedded mdhtml runtime is authorized;
- whether script hashes can eliminate `'unsafe-inline'` for scripts.

The hosting service MUST apply CSP using HTTP response headers.

A conceptual target:

```
default-src 'none'
script-src <official-runtime-hash>
style-src ...
img-src ...
font-src ...
connect-src 'none'
frame-src 'none'
object-src 'none'
base-uri 'none'
form-action 'none'
```

This is illustrative, not normative.

The Tech Spec must determine the final policy.

---

## 13. mdhtml audit

Introduce a first-class security inspection capability.

Tentative command:

```
mdhtml audit document.md.html
```

Example:

```
$ mdhtml audit architecture.md.html
✓ valid mdhtml v1.0
✓ canonical source present
✓ source integrity valid
✓ HTML security policy passed
✓ CSS security policy passed
✓ no unauthorized executable content
✓ runtime integrity valid
✓ no unexpected external resources
SAFE
```

The audit command SHOULD also support machine-readable output:

```
mdhtml audit document.md.html --json
```

Example conceptual response:

```json
{
  "safe": true,
  "specVersion": "1.0",
  "sourceIntegrity": true,
  "html": "pass",
  "css": "pass",
  "runtime": "pass",
  "externalResources": []
}
```

Exact schema belongs in the Tech Spec.

---

## 14. Security Diagnostics

Security failures must be useful to both humans and agents.

Bad:

```
Unsafe document.
```

Good:

```
MDHSEC012: unsafe URI scheme
line 87, column 14
    <a href="javascript:alert(1)">
             ^^^^^^^^^^
javascript: URLs cannot be used in safe mdhtml documents.
```

Diagnostics SHOULD include:

- stable error code;
- category;
- severity;
- source location where available;
- offending construct;
- concise explanation;
- remediation guidance.

Stable diagnostic codes are particularly important for agent workflows.

---

## 15. Test Strategy for Security

Security behavior MUST be heavily test-driven.

Create a dedicated security fixture suite.

Suggested structure:

```
fixtures/
  security/
    html/
    css/
    svg/
    urls/
    malformed/
    mutation-xss/
    runtime/
    external-resources/
```

Each fixture should specify:

```
input
expected result
expected diagnostic
```

Examples must include known historical sanitizer bypass patterns where licensing allows.

Tests MUST cover malformed and adversarial HTML, not merely obviously malicious examples.

---

## 16. Fuzzing

The Tech Spec MUST evaluate fuzz testing for:

- parser;
- HTML validator;
- CSS validator;
- extraction;
- build → audit pipeline.

Rust tooling such as cargo-fuzz SHOULD be considered.

Important invariants should be fuzzed.

Examples:

```
safe build → audit must always pass
```

and:

```
extract(build(source)) == source
```

and:

```
user-controlled input must never create unauthorized executable nodes
```

---

## 17. Dependency Security

Security-sensitive dependencies must be treated differently from ordinary dependencies.

The project SHOULD implement:

- Dependabot or equivalent;
- automated vulnerability scanning;
- cargo audit;
- CI failure for relevant known vulnerabilities;
- explicit review of sanitizer/parser updates.

Sanitizer dependencies MUST remain current.

---

## 18. Public Hosting

Create an official hosting service for mdhtml documents.

Primary objective:

> Make publishing an mdhtml document as easy as sharing a gist.

Example:

```
mdhtml publish architecture.md
```

returns:

```
https://<domain>/H7zPm
```

The service should optimize for:

- simplicity;
- low operational cost;
- reliability;
- agent compatibility;
- abuse resistance;
- no vendor lock-in at the format level.

A hosted document remains a normal `.md.html` artifact.

---

## 19. Hosting MVP

The first version SHOULD avoid unnecessary SaaS infrastructure.

Preferred initial architecture:

```
CLI / Agent
    │
    ▼
Publish API
    │
    ├── rate limit
    ├── validate
    ├── build/audit
    ├── hash
    │
    ▼
object storage
    │
    ▼
CDN / Edge
    │
    ▼
public URL
```

Vercel is the initial deployment target.

Potential components:

- Vercel;
- Vercel Functions;
- Vercel Blob;
- Vercel CDN/Edge.

The Tech Spec MUST verify current pricing, limits, security characteristics, and operational implications before implementation.

---

## 20. No Deployment Per Document

A document MUST NOT require an individual Vercel deployment.

Documents should be stored as immutable or mostly immutable objects and served through the shared hosting application.

Avoid:

```
document
   ↓
Vercel deployment
```

Prefer:

```
document
   ↓
object storage
   ↓
CDN
```

---

## 21. Publishing Source vs Generated Artifact

The Tech Spec must make a final decision regarding the API payload.

Preferred approach:

```
mdhtml publish document.md
```

uploads the canonical source plus necessary metadata.

The server then:

```
validates source
   ↓
builds using canonical implementation
   ↓
audits
   ↓
publishes
```

This prevents a compromised or modified client from uploading arbitrary HTML while claiming that it was produced by mdhtml.

However, deterministic builds, runtime versions, custom assets, and compatibility must be considered.

The Tech Spec MUST explicitly compare:

**Option A** — Upload `.md.html` artifact.

**Option B** — Upload Markdown and build server-side.

**Option C** — Upload both source and artifact and verify correspondence.

The selected architecture must prioritize security and reproducibility without unnecessarily increasing infrastructure complexity.

---

## 22. Isolated Origin

User-generated documents MUST NOT share the authentication/security origin of administrative applications.

Example:

```
mdhtml.example
    → application / API
docs.example
    → user-generated documents
```

A dedicated registrable domain for user content SHOULD be considered if feasible.

The objective is to minimize the impact of any future sanitizer bypass.

Hosted user documents MUST NOT have access to:

- authentication cookies;
- application localStorage;
- privileged APIs;
- admin sessions.

---

## 23. Public URL Design

URLs should be:

- short;
- durable;
- copyable;
- case-insensitive if practical;
- difficult to enumerate at scale.

Example:

```
https://<domain>/H7zPm
```

The Tech Spec should evaluate:

- NanoID;
- base58;
- base62;
- content-derived identifiers;
- random identifiers;
- hybrid approaches.

Avoid sequential IDs.

---

## 24. Content Addressability

Internally, documents SHOULD be content-addressed where practical.

Conceptually:

```
SHA256(canonical source)
         ↓
    object storage
```

Public short IDs can reference the content object.

Benefits:

- deduplication;
- integrity verification;
- immutable artifacts;
- simpler caching;
- reproducibility;
- future verification features.

---

## 25. Publishing Semantics

Anonymous documents SHOULD initially be immutable.

This makes the system dramatically simpler.

Instead of:

```
PUT /document/H7zPm
```

prefer:

```
POST /documents
    ↓
creates H7zPm permanently
```

Updates create new documents.

Authenticated editing/versioning may be introduced later.

---

## 26. Anonymous Publishing

The MVP SHOULD support anonymous publishing.

Potential policy:

- strict document size limit;
- rate limiting;
- abuse detection;
- immutable documents;
- no custom slug;
- no private documents.

Exact limits belong in the Tech Spec.

Do not require accounts unless operational/security analysis shows that anonymous publishing is impractical.

Frictionless publishing is a core product objective.

---

## 27. Abuse Prevention

Public anonymous HTML hosting will attract abuse if successful.

Threats include:

- phishing;
- spam;
- malware distribution;
- illegal content;
- SEO abuse;
- storage abuse;
- automated mass publishing;
- denial of service.

The Tech Spec MUST define an abuse model.

Potential defenses:

- IP rate limiting;
- document-size limits;
- publish-rate limits;
- content security policy;
- no arbitrary JavaScript;
- no forms;
- no arbitrary external requests;
- robots/noindex policy for anonymous content;
- abuse reporting;
- administrative takedown capability;
- content hashes;
- deny lists where justified.

The system MUST support document removal by administrators even if public documents are conceptually immutable.

"Immutable" means immutable by the publisher, not impossible for the service operator to remove.

---

## 28. Privacy

The MVP must clearly communicate that public documents are public.

Do not imply secrecy from an unguessable URL.

Future versions may support:

- unlisted;
- private;
- expiration;
- password protection.

These are NOT required for MVP unless the Tech Spec identifies a compelling reason.

---

## 29. SEO

The Tech Spec must make an explicit decision about indexing.

Recommended MVP:

Anonymous documents SHOULD default to:

```
noindex, nofollow
```

Reasons:

- reduce spam incentive;
- reduce phishing value;
- avoid becoming a free SEO-hosting network.

Authenticated or verified users could eventually opt into indexing.

---

## 30. CLI Publishing

Introduce:

```
mdhtml publish <source>
```

Example:

```
$ mdhtml publish architecture.md
Building...
✓ mdhtml v1.0
✓ source integrity
✓ HTML security
✓ CSS security
✓ runtime integrity
Publishing...
✓ published
https://<domain>/H7zPm
```

Publishing MUST run the local security checks before network upload.

The server MUST independently validate the document.

Never trust client-side validation.

---

## 31. CLI Authentication

Authentication is NOT required for anonymous MVP publishing.

Future authentication may use:

```
mdhtml login
```

and API tokens.

The architecture SHOULD avoid preventing this future capability.

---

## 32. Publish API

Design a small agent-friendly HTTP API.

Conceptually:

```
POST /v1/documents
```

The API should support straightforward invocation from:

- mdhtml CLI;
- curl;
- AI agents;
- CI;
- GitHub Actions;
- third-party tools.

Response example:

```json
{
  "id": "H7zPm",
  "url": "https://<domain>/H7zPm",
  "sha256": "...",
  "mdhtmlVersion": "1.0"
}
```

Error responses MUST be structured.

Example:

```json
{
  "error": {
    "code": "MDHSEC012",
    "message": "Unsafe URI scheme",
    "line": 87,
    "column": 14
  }
}
```

Agents should be able to fix a document based solely on the response.

---

## 33. Agent Skill

Create or update the official mdhtml agent skill.

The skill should teach an agent how to:

- author mdhtml-compatible Markdown;
- understand the specification;
- build documents;
- audit documents;
- respond to diagnostics;
- publish documents;
- return the resulting public URL.

Conceptual workflow:

```
User:
"Create an architecture proposal and share it."
Agent:
   ↓
writes architecture.md
   ↓
mdhtml build
   ↓
mdhtml audit
   ↓
fixes issues if necessary
   ↓
mdhtml publish
   ↓
returns public URL
```

The agent MUST NOT automatically use `--unsafe`.

---

## 34. Skill Security Guidance

The skill must explicitly instruct agents:

1. Never bypass a security diagnostic merely to complete publication.
2. Fix the source while preserving author intent.
3. Never invoke unsafe mode unless explicitly requested by the user.
4. Never attempt to encode or obfuscate prohibited constructs.
5. Treat security errors as constraints, not obstacles to circumvent.
6. Prefer embedded/local assets when external resources violate policy.

---

## 35. CI Integration

The security model should work outside the official hosting service.

Example GitHub workflow:

```
mdhtml build docs/*.md
mdhtml audit docs/*.md.html
```

This allows projects using GitHub Pages or other static hosting to receive the same guarantees.

Potential future command:

```
mdhtml check .
```

But this is optional.

---

## 36. GitHub Pages / Static Hosting

One major success criterion is:

> A developer should receive most mdhtml security benefits even if they never use the official hosting service.

Therefore:

```
mdhtml build README.md
```

must already produce a safe-by-default artifact suitable for static hosting.

The official service adds:

- convenience;
- short URLs;
- CDN;
- API;
- agent publishing;
- abuse controls.

It must not be the component that makes mdhtml safe.

---

## 37. Donations

The service should initially be free.

Monetization is NOT an MVP goal.

A lightweight donation mechanism may be added.

Potential options:

- GitHub Sponsors;
- Ko-fi;
- Buy Me a Coffee;
- Pix;
- another low-friction provider.

The website may include a subtle message such as:

> mdhtml hosting is free. If it saves you time, you can help keep it running.

Do not create artificial product limitations merely to drive donations.

---

## 38. Future Commercial Possibilities

The architecture should not prevent future optional features such as:

- custom slugs;
- custom domains;
- analytics;
- private documents;
- organization namespaces;
- document expiration;
- access control;
- larger assets;
- version history;
- collaborative publishing;
- verified publishers.

These are explicitly out of scope for MVP.

---

## 39. Hosting Website

Create a minimal website explaining:

- **What mdhtml is** — One self-contained document for humans and machines.
- **How to create one** — `mdhtml build document.md`
- **How to publish** — `mdhtml publish document.md`
- **How to inspect** — `mdhtml audit document.md.html`
- **Example documents** — Link to representative mdhtml examples.
- **Open source** — Link to repository.
- **Donation** — Optional support link.

Avoid building a dashboard for MVP unless absolutely necessary.

---

## 40. Optional Browser Publishing

A browser-based drag-and-drop publisher MAY be included if implementation cost is very low.

Concept:

```
Drop your .md or .md.html
        ↓
    Security audit
        ↓
    Publish
        ↓
   Copy URL
```

However, CLI and API are higher priorities.

---

## 41. Observability

The hosting service needs enough telemetry to operate safely without building a complex analytics platform.

Track at minimum:

- publish requests;
- successful publishes;
- validation failures;
- document size;
- storage usage;
- bandwidth;
- HTTP errors;
- rate-limit events;
- takedowns;
- sanitizer/security failures.

Do NOT log document contents unnecessarily.

---

## 42. Cost Controls

Because hosting is initially free, infrastructure must fail safely from a cost perspective.

The Tech Spec MUST define:

- maximum document size;
- request limits;
- storage limits;
- bandwidth strategy;
- CDN caching;
- rate limiting;
- abuse limits;
- alerts;
- budget thresholds.

The system should favor:

```
static object + CDN cache
```

over:

```
server execution per document view.
```

A document view should ideally require zero server-side compute after publication.

---

## 43. Caching

Published immutable documents are ideal for aggressive caching.

The Tech Spec should evaluate:

```
Cache-Control:
  public,
  max-age=...,
  immutable
```

Short-ID resolution should also be cacheable where architecture permits.

---

## 44. Availability Philosophy

This is initially a community/open-source utility, not a mission-critical SaaS.

Prefer:

- simple;
- recoverable;
- inexpensive;
- observable;

over:

- complex HA infrastructure;
- multi-region databases;
- premature distributed systems.

The public artifact itself remains portable, reducing platform dependency.

---

## 45. Spec Compatibility

The current frozen v1.0 specification MUST NOT be casually broken.

The Tech Spec must determine whether safe-by-default behavior represents:

- implementation behavior compatible with v1.0;
- an additive specification revision;
- v1.1;
- a security profile;
- another standards mechanism.

Do not modify the frozen contract without explicitly documenting the compatibility rationale.

---

## 46. ADR Requirements

Important architectural decisions MUST receive ADRs.

At minimum consider ADRs for:

1. Security validation architecture.
2. HTML sanitizer selection.
3. CSS sanitizer/parser selection.
4. Safe vs unsafe mode.
5. CSP strategy.
6. Hosting architecture.
7. Source vs artifact upload.
8. Content addressing.
9. Public ID generation.
10. Anonymous publishing.
11. Isolated user-content origin.
12. Storage provider.
13. Abuse/takedown model.

Agents should avoid putting major architectural rationale only inside implementation code or PR descriptions.

---

## 47. Tech Spec Deliverable

Before implementation, the orchestrating agent MUST produce a detailed Tech Spec.

The Tech Spec must contain:

- current architecture analysis;
- relevant SPEC.md constraints;
- threat model;
- trust boundaries;
- HTML attack surface;
- CSS attack surface;
- runtime attack surface;
- resource/network attack surface;
- sanitizer library evaluation;
- proposed security pipeline;
- CLI changes;
- API contract;
- hosting architecture;
- storage model;
- caching model;
- ID generation;
- rate limiting;
- abuse controls;
- CSP;
- isolated-origin strategy;
- test strategy;
- fuzzing strategy;
- migration/compatibility;
- deployment plan;
- rollout strategy;
- open questions.

No major implementation should begin before this document exists.

---

## 48. Required Research Before Tech Spec

The orchestrating agent should delegate research tasks in parallel.

**Agent A — Current mdhtml architecture**

Inspect: SPEC.md; parser; renderer; JS runtime; CLI; fixtures; ADRs; theming; HTML/CSS customization; source embedding/extraction.

Produce a current-state architecture map.

**Agent B — HTML security**

Research: Ammonia; html5ever; known bypass classes; mutation XSS; SVG/MathML issues; sanitizer maintenance history; alternatives.

Recommend an approach.

**Agent C — CSS security**

Research: lightningcss; css-sanitizer; alternatives; CSS exfiltration techniques; resource-loading constructs; selector risks.

Recommend an approach preserving maximum styling freedom.

**Agent D — Browser security**

Research: CSP; meta CSP vs HTTP CSP; script hashes; `file://` behavior; origin isolation; browser differences; sandbox mechanisms.

**Agent E — Hosting**

Research current: Vercel Hobby limits; Vercel Blob pricing; Vercel Functions; Edge/CDN behavior; Vercel Open Source Program; viable alternatives if necessary.

Estimate cost for: 1k docs; 10k docs; 100k docs; 1M document views/month.

**Agent F — Abuse**

Threat-model anonymous document hosting.

Cover: spam; phishing; malware; illegal content; SEO abuse; automated publishing; takedown requirements.

---

## 49. Agent Orchestration Strategy

After research:

```
Research agents
      │
      ▼
Orchestrating architect
      │
      ▼
   Tech Spec
      │
      ▼
   ADR creation
      │
      ▼
Implementation streams
```

Implementation should then be divided where boundaries allow.

Suggested streams:

- **Stream 1** — Core security pipeline.
- **Stream 2** — CLI: audit; publish; diagnostics.
- **Stream 3** — Hosting/API/storage.
- **Stream 4** — Agent skill and documentation.
- **Stream 5** — Security fixtures/fuzzing/adversarial tests.

Agents should not independently invent incompatible contracts.

Shared interfaces must be established in the Tech Spec first.

---

## 50. Security Review Gate

Before public hosting launches, assign an agent specifically to act adversarially.

Its job is NOT to implement features.

Its job is to break the system.

Attempt: XSS; mXSS; SVG attacks; malformed markup; CSS exfiltration; external network requests; CSP bypass; runtime injection; source/runtime confusion; parser differential attacks; Unicode/encoding attacks; oversized documents; decompression/resource exhaustion where applicable; API abuse.

All findings should become regression fixtures.

---

## 51. Differential Testing

mdhtml currently maintains Rust and JavaScript implementations with structural parity.

Security-sensitive parsing differences can become vulnerabilities.

The Tech Spec MUST evaluate differential tests between implementations.

Where both implementations process equivalent structures:

```
Rust(input) ≈ JavaScript(input)
```

Security interpretation must not diverge in ways that allow a document rejected by one implementation to execute differently in another.

---

## 52. Deterministic Builds

The Tech Spec SHOULD investigate whether identical:

```
source
+
mdhtml version
+
configuration
```

can produce byte-identical `.md.html`.

If practical, deterministic builds provide substantial value for: verification; caching; content addressing; reproducibility; agent workflows.

Do not compromise important functionality merely to achieve determinism, but explicitly investigate it.

---

## 53. Verification

Potential future capability:

```
mdhtml verify document.md.html
```

or:

```
mdhtml verify document.md https://<domain>/H7zPm
```

Possible checks: canonical source hash; runtime hash; build version; artifact integrity.

This is not required for MVP but the architecture SHOULD avoid preventing it.

---

## 54. Definition of Done — Safe Build

Safe-by-default is complete when:

- `mdhtml build` performs security validation by default;
- unsafe executable author content is rejected;
- arbitrary visual customization remains substantially intact;
- HTML validation uses structural parsing;
- CSS validation uses structural parsing;
- dangerous URLs/resources are detected;
- official runtime integrity is distinguishable from user content;
- actionable diagnostics exist;
- regression security fixtures exist;
- extraction remains byte-perfect;
- existing valid documents remain compatible wherever reasonably possible;
- unsafe mode requires explicit opt-in.

---

## 55. Definition of Done — Audit

`mdhtml audit` is complete when:

- existing `.md.html` artifacts can be inspected;
- source integrity is checked;
- HTML security is checked;
- CSS security is checked;
- runtime integrity is checked;
- external resources are reported;
- output works for humans;
- JSON output works for agents/CI;
- exit codes are deterministic.

---

## 56. Definition of Done — Hosting

Hosting MVP is complete when:

```
mdhtml publish document.md
```

can return a public URL.

Additionally:

- server independently validates the document;
- unsafe documents cannot be published;
- published documents are served statically/cacheably;
- user content is isolated from privileged application origin;
- strict HTTP security headers are applied;
- anonymous abuse is rate-limited;
- document size is bounded;
- documents can be administratively removed;
- infrastructure cost is observable;
- the system requires minimal ongoing operations.

---

## 57. Definition of Done — Agent Experience

An agent with the official skill should be able to receive:

> Create a technical proposal about X and share it with me.

and autonomously:

```
create Markdown
    ↓
build
    ↓
interpret diagnostics
    ↓
fix safe-mode violations
    ↓
publish
    ↓
return URL
```

without needing undocumented knowledge of the hosting API.

---

## 58. Non-Goals

This project does NOT aim to build: a CMS; Notion; Google Docs; collaborative editing; a website builder; a general-purpose HTML hosting platform; a JavaScript application hosting platform; a proprietary Markdown replacement; a component DSL replacing Markdown; a paid SaaS platform; a complex user dashboard.

Do not expand scope into these areas.

---

## 59. Critical Product Constraints

The implementation must preserve these principles.

**Constraint 1 — Markdown remains canonical.** The Markdown source is the document. HTML is its self-contained presentation.

**Constraint 2 — Exact recovery.** The original source must remain recoverable byte-for-byte.

**Constraint 3 — Presentation freedom.** Do not solve security by substantially removing mdhtml's visual customization capabilities.

**Constraint 4 — No arbitrary executable code by default.** Customization is not permission to execute arbitrary code.

**Constraint 5 — Safe everywhere.** Security should benefit local files and third-party static hosting, not only official hosting.

**Constraint 6 — Agent-native.** Every important workflow must be scriptable and machine-readable.

**Constraint 7 — Hosting remains optional.** mdhtml must never require the official service.

**Constraint 8 — Simplicity.** Do not turn a document format into an unnecessarily complex cloud platform.

---

## 60. Guiding Architecture

The target conceptual system is:

```
AUTHOR / AGENT
      │
      ▼
   doc.md
      │
      ▼
┌─────────────┐
│ mdhtml CLI  │
└──────┬──────┘
       │
       ▼
    parser
       │
       ▼
    renderer
       │
       ▼
security pipeline
  │     │     │
  │     │     └── runtime integrity
  │     └──────── CSS
  └────────────── HTML/resources
       │
       ▼
   SAFE ARTIFACT
       │
       ├──────────────► file://
       │
       ├──────────────► GitHub Pages
       │
       ├──────────────► arbitrary static hosting
       │
       │
       └── mdhtml publish
                │
                ▼
          Publish API
                │
         server validation
                │
                ▼
         content storage
                │
                ▼
              CDN
                │
                ▼
         short public URL
```

The official hosting service is therefore not a new document format.

It is simply:

> the easiest place to publish an already-safe mdhtml document.

---

## 61. Desired End State

The experience should eventually feel this simple:

```
$ mdhtml build proposal.md
✓ proposal.md.html
```

or:

```
$ mdhtml publish proposal.md
✓ safe
✓ published
https://<domain>/7km2x
```

And for an AI agent:

> Write the proposal and share it.

should be sufficient.

Behind that simplicity should exist:

- a frozen and explicit document specification;
- deterministic parsing;
- byte-perfect source preservation;
- safe-by-default generation;
- HTML and CSS security guards;
- runtime integrity;
- CSP;
- adversarial tests;
- isolated hosting;
- immutable static delivery;
- a tiny publishing API;
- an agent-native CLI.

The complexity belongs inside the tooling.

The artifact should remain simple: one file, self-contained, human-readable, machine-readable, customizable, portable, and safe by default.
