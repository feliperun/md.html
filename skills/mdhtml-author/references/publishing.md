# Publishing

`mdhtml publish` uploads a document to the official hosting service and
prints a short public URL. Publishing is safe-by-default: nothing reaches
the network until the local build and audit pass. The normative contract is
docs/prd/mdhtml-safety-hosting-prd.md (§30–§34, §57) and
docs/adr/0012-source-vs-artifact-upload.md.

## The build → audit → fix → publish → URL loop

1. `mdhtml build doc.md` — produce the artifact.
2. `mdhtml audit doc.md.html` — verify the built artifact against the
   security policy.
3. Fix every diagnostic in the source; never bypass a diagnostic to
   publish.
4. `mdhtml publish doc.md` — upload the canonical source and referenced
   assets, then receive the public URL.
5. Return the public URL to the user. No undocumented API knowledge is
   needed: the CLI prints the URL and the agent relays it.

## Publish grammar

```
mdhtml publish <source> [--url <base-url>]
```

- `<source>` is the canonical Markdown file (for example `doc.md`), never
  the built `.md.html` artifact.
- `--url <base-url>` targets the publish endpoint; it is how a local or mock
  endpoint is exercised.
- `MDHTML_PUBLISH_URL` is the environment-variable equivalent. Resolution
  order: `--url` flag wins over `MDHTML_PUBLISH_URL`, which wins over the
  default official endpoint.

## Local validation before upload

`mdhtml publish` runs the local build and audit first, and the server
independently re-validates with the pinned canonical toolchain
(docs/adr/0012-source-vs-artifact-upload.md). Client-side validation is
never trusted, so a diagnostic that passes locally can still fail on the
server. The agent must fix every local diagnostic before uploading, and
must treat a server-side rejection the same way: fix the source, rebuild,
and republish.

## Structured errors

Failures come back as self-sufficient structured errors:

```json
{
  "error": {
    "code": "E-MDHSEC-012",
    "message": "unsafe URI scheme",
    "line": 87,
    "column": 14
  }
}
```

`line` and `column` locate the offending construct in the source; map the
`E-MDHSEC-*` code back to the construct it names (for example
`E-MDHSEC-012` is an unsafe URI scheme in a link or reference destination,
`E-MDHSEC-003` a denied attribute). Fix the construct in the Markdown, not
the artifact, and never encode or obfuscate a prohibited construct to make
a diagnostic pass.

## `--unsafe` can never be published

`--unsafe` disables the content-security guards, marks the artifact unsafe,
and official hosting rejects it (docs/adr/0009-safe-vs-unsafe-mode.md).
`mdhtml publish` never uploads such an artifact, and the agent never invokes
`--unsafe` unless a human explicitly requested it.
