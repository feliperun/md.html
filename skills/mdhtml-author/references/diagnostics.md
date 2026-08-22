# Diagnostics

Diagnostics are stable records with a code, a severity, and a message. CLI
messages use the form `mdhtml: <code>: <message>`.

## Severities

- `error` — violates a MUST; `build` and `check` exit nonzero and `build`
  writes no output.
- `warning` — convention-level deviation; reported by `check`; exit code
  unaffected.
- `info` — the portability verdict and the byte-budget report from `check`.

## Enumerated codes

| Code | Severity | Condition |
| --- | --- | --- |
| `E-FMT-01` | error | missing or duplicate canonical source, or wrong type |
| `E-FMT-02` | error | `</script` in input |
| `E-FMT-03` | error | portability attribute contradicts content |
| `E-FMT-05` | error | missing `title` |
| `E-PARSE-01` | error | front matter violation |
| `E-SECT-01` | error | orphan slug in `sections:` |
| `W-SECT-01` | warning | duplicate explicit `{#id}` |
| `W-COMP-02` | warning | container or component out of convention |
| `E-CLI-01` | error | unresolvable asset or out-of-table MIME |
| `E-CLI-03` | error | unsafe path, invalid base64, duplicate asset path, or existing target file |
| `W-UI-04` | warning | missing embedded asset in a hand-edited artifact |
| `I-CLI-02` | info | portability verdict and byte-budget report |

## Authoring notes

- Fix every `E-` diagnostic; a document that reports one has not completed
  the workflow.
- `W-COMP-02` is the degradation signal: content outside a container or
  component shape renders as prose. Fix the shape or remove the unit.
- `I-CLI-02` is the green light: the document is portable and the byte budget
  is reported by category (content, runtime, fonts, images).
