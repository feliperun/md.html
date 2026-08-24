# Security fixtures

Deterministic corpus for the safe-by-default pipeline (Tech Spec "Test
strategy", ADR 0006). One JSON object per file, walked directly by the Rust
test suite (`crates/mdhtml/tests/security_*.rs`): adding a fixture requires no
harness change.

## Contract

One case per file, named `fixtures/security/<category>-<name>.json`:

```json
{
  "id": "url-javascript-scheme",
  "category": "url",
  "requirement": "PRD-14",
  "status": "invalid",
  "diagnostic": "E-MDHSEC-012",
  "location": "5:1",
  "source": "---\ntitle: T\n---\n\n[a](javascript:alert(1))\n",
  "assets": { "logo.svg": "PHN2Zy8+" },
  "detail": "javascript: destination in a Markdown link"
}
```

- `id` is unique and matches the file name (`<category>-<name>`).
- `category` is one of `html`, `url`, `css`, `svg`, `path`, `runtime`, `csp`,
  `artifact`, `unsafe`, or the Phase 6 adversarial categories `mutation-xss`,
  `malformed`, `external` (PRD §15).
- `status` is `invalid` (the build must reject with exactly `diagnostic` and
  write no output), `valid` (the build must succeed and stay portable), or
  `unsafe` (build-level pair: without the flag the build must reject with
  exactly `diagnostic` and write no output; with `--unsafe` it must succeed,
  carry `data-mdhtml-safe="false"` on the root element, and still pass
  `extract`).
- `diagnostic` is required for `invalid` and `unsafe` cases and must be one
  of the frozen `E-MDHSEC-*` codes in the Tech Spec addendum — except the
  `E-FMT-02` stored-source terminator, which `mutation-xss` breakout cases
  pin as the guard that makes the breakout impossible; omitted for `valid`
  cases.
- `location` is optional and only meaningful for `invalid` and `unsafe`
  cases: a string
  `"LINE:COLUMN"` (1-based) asserted exactly when present. For document-level
  categories (`url`, `html`, `path`, `svg`) it cites the canonical source —
  the whole `.md` file including front matter. For `css` cases it is relative
  to the local `.theme.css` file, the frame the diagnostic message already
  names as author CSS. Omitted (no position) where the guard has no cheap
  position: front-matter `url`/`fonts.url`/`cover` values and CSS constructs
  the visitor classifies without a parser location.
- `source` is the canonical Markdown input for build-level cases.
- `assets` is optional: base64 (RFC 4648) file bytes keyed by the relative
  path the source references. The harness materializes them in a temp
  directory next to the source before building.
- `detail` is a one-line human explanation of the attack being exercised.

Artifact-level cases (the `artifact` category, used by `mdhtml audit`)
replace `source` with `kind: "artifact"` and an `html` field holding the
built artifact bytes as text; the harness writes them to a `.md.html` file
and runs the audit path instead of the build path. The `unsafe` category is
build-level: its cases carry `status: "unsafe"` and pin the pair — the safe
build rejects while the `--unsafe` build succeeds, attests, and still
extracts (see the status contract above).

## Invariants

- `invalid` cases assert the exact diagnostic code, not a substring.
- `valid` cases are security-relevant inputs that must keep building: the
  corpus pins the allowlist, so a future tightening that breaks a `valid`
  case is a contract change, not a test failure to silence.
- Every guard rejection landed by a fix must add its regression fixture here
  in the same commit (ADR 0006, consequences).

### Phase 6 adversarial categories

`mutation-xss`, `malformed`, `runtime` and `external` cases are walked by
`crates/mdhtml/tests/security_adversarial.rs` with the build-level contract
above plus two additions: `valid` cases must also audit SAFE and round-trip
`extract(build(source)) == source`, and `kind: "artifact"` cases audit
exactly as the `artifact` category does. `mutation-xss` cases with status
`valid` are additionally rendered by `runtime/test/security-mutation-xss.test.mjs`,
which asserts at the renderer — the layer where mutation XSS actually lands —
that the payload never survives as a real element.

### Artifact cases (`mdhtml audit`)

Artifact-level cases keep `kind: "artifact"` and are walked by
`crates/mdhtml/tests/security_audit.rs` through the audit path instead of the
build path. A case carries EITHER:

- `html` — static artifact text, written verbatim to a `.md.html` file and
  audited. Static fixtures are only usable for structure violations that fail
  before runtime hashing matters (a static artifact cannot carry runtime bytes
  whose hash matches its own CSP); or
- `source` — canonical Markdown, built into a REAL artifact with
  `mdhtml::build` (or `mdhtml::build_unsafe` when `"unsafe": true`) and then
  audited, with optional `assets` materialized next to the source.

For artifact cases `status` is `valid` (the audit must report SAFE) or
`invalid` (the audit must report UNSAFE and the rendered report must contain
exactly `diagnostic`, one of the frozen audit codes `E-MDHSEC-015/-016/-017/
-018` or a stored-source guard code). `location` is optional and, when
present, asserts the cited `LINE:COLUMN` appears in the report. The stored
`unsafe` marker only selects `build_unsafe`; the `unsafe` STATUS contract
above is build-level only.
