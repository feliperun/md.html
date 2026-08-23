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
  "source": "---\ntitle: T\n---\n\n[a](javascript:alert(1))\n",
  "assets": { "logo.svg": "PHN2Zy8+" },
  "detail": "javascript: destination in a Markdown link"
}
```

- `id` is unique and matches the file name (`<category>-<name>`).
- `category` is one of `html`, `url`, `css`, `svg`, `path`, `runtime`, `csp`,
  `artifact`, `unsafe`.
- `status` is `invalid` (the build must reject with exactly `diagnostic` and
  write no output) or `valid` (the build must succeed and stay portable).
- `diagnostic` is required for `invalid` cases and must be one of the frozen
  `E-MDHSEC-*` codes in the Tech Spec addendum; omitted for `valid` cases.
- `source` is the canonical Markdown input for build-level cases.
- `assets` is optional: base64 (RFC 4648) file bytes keyed by the relative
  path the source references. The harness materializes them in a temp
  directory next to the source before building.
- `detail` is a one-line human explanation of the attack being exercised.

Artifact-level cases (the `artifact` and `unsafe` categories, used by
`mdhtml audit`) replace `source` with `kind: "artifact"` and an `html` field
holding the built artifact bytes as text; the harness writes them to a
`.md.html` file and runs the audit path instead of the build path.

## Invariants

- `invalid` cases assert the exact diagnostic code, not a substring.
- `valid` cases are security-relevant inputs that must keep building: the
  corpus pins the allowlist, so a future tightening that breaks a `valid`
  case is a contract change, not a test failure to silence.
- Every guard rejection landed by a fix must add its regression fixture here
  in the same commit (ADR 0006, consequences).
