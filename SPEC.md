# mdhtml 1.0 — Format Specification

> **Status:** frozen for implementation. This document is the single normative
> authority for the mdhtml format and toolchain. `.plans/plan.md` and
> `.plans/plan_preview.md` are planning context only and are superseded here.

## 1. Conventions

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are to be
interpreted as described in RFC 2119.

mdhtml 1.0 has two conforming implementations:

- **browser runtime** — dependency-free classic JavaScript embedded in the
  document;
- **CLI** — `mdhtml`, a Rust binary built with no third-party crates.

Both MUST satisfy every MUST/MUST NOT in this document and MUST pass the shared
fixture suite in `fixtures/`. A construct accepted by one implementation and
rejected by the other is a spec violation, not a feature. Neither
implementation MAY accept a front matter or Markdown extension not described
here.

Symbols defined by this document:

| Symbol | Meaning |
|---|---|
| `mdhtml/1.0` | The document format identified by `data-mdhtml="1.0"` |
| `Diagnostic` | Stable, machine-readable error/warning/info record (§16) |
| `Runtime fragment manifest` | Committed artifact describing runtime fragments (§17) |

## 2. Document model (FMT-01)

A conforming document is a complete HTML document whose root element declares
`data-mdhtml="1.0"` and that contains exactly one
`script#mdhtml-source[type="text/markdown"]`. The `textContent` of that element
is the **canonical source**: the single source of truth. Everything else in the
document — metadata, styles, asset blocks, runtime fragments, CSP — is derived
data and MUST NOT be treated as source.

The canonical source is `front matter + body`: an optional front matter block
(§8) followed by the Markdown body (§9).

### FMT-01 — document identity and canonical source

- A document MUST declare `data-mdhtml="1.0"` on the root element.
- A document MUST contain exactly one `#mdhtml-source[type="text/markdown"]`.
- A document MUST NOT contain a second element with `id="mdhtml-source"` nor
  any other `type="text/markdown"` script.
- Every derived artifact (rendered document, copy output, `extract` output)
  MUST be derivable from the canonical source plus the declared build inputs.
- Violation: diagnostic `E-FMT-01`.

Example skeleton:

```html
<!doctype html>
<html lang="en" data-mdhtml="1.0" data-mdhtml-portable="true">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data: blob:; font-src data:; media-src data: blob:">
  <title>Quarterly report</title>
  <style id="mdhtml-tokens">…</style>
  <style id="mdhtml-theme">…</style>
  <style id="mdhtml-user">…</style>
</head>
<body>
  <div id="mdhtml-app"></div>
  <noscript><style>#mdhtml-source{display:block;white-space:pre-wrap;font-family:ui-monospace;padding:2rem}</style></noscript>
  <script id="mdhtml-source" type="text/markdown">---
title: Quarterly report
---

# Results
</script>
  <script type="application/octet-stream" data-path="images/photo.png" data-type="image/png">BASE64…</script>
  <script id="mdhtml-runtime">…</script>
</body>
</html>
```

## 3. Canonical source handling

- The canonical source is stored verbatim: no newline normalization, no
  trimming, no re-encoding. `extract` and "full" copy MUST return the stored
  bytes exactly (§15, §13).
- Editing prose inside the canonical source of a built document and reloading
  MUST work without a toolchain (runtime rendering).
- Two situations cannot be fixed by prose edits and require a rebuild:
  1. a referenced asset that is not embedded — the runtime renders the image's
     alt text (UI-04) and `check` warns;
  2. a component whose runtime fragment is not included — the component
     degrades to prose (COMP-02) and `check` warns.
- No derived artifact may become source. If the rendered DOM and the canonical
  source disagree, the canonical source wins.

## 4. Escape rule (FMT-02)

A raw text element has no escapes: the sequence `</script` (ASCII
`<` `/` `s` `c` `r` `i` `p` `t`, case-insensitive) terminates the script block
in the HTML parser. It is the one forbidden sequence in the format.

### FMT-02 — script-termination sequence

- The canonical source MUST NOT contain `</script` (case-insensitive).
- `mdhtml build` MUST reject any input containing it with diagnostic `E-FMT-02`
  and MUST NOT write output.
- A hand-edited document MAY contain the encoded form `<\/script`. The runtime
  MUST decode every case-insensitive occurrence of the literal sequence
  `<\/script` to `</script` before Markdown parsing.
- Decoding applies to rendering and to the `smart` and `body` copy modes, but
  NOT to `full` copy or `extract`, which return the stored bytes verbatim.

Rationale: `<\/script` keeps a hand-edited file well-formed; decoding restores
the author's intent without breaking round-trip, because `extract` returns
exactly what `build` stored.

## 5. Portability and CSP (FMT-03)

### FMT-03 — portability

- A document is portable if and only if every subresource it may load comes
  from an inline `data:` or `blob:` payload — no external-origin requests for
  scripts, styles, images, fonts, or media.
- A portable document MUST:
  - carry the canonical CSP below;
  - use only classic inline scripts (never `type="module"`, never external
    `src`);
  - declare `data-mdhtml-portable="true"`.
- Hyperlinks (`<a href>`) and social metadata URLs MAY reference external
  origins without affecting portability: the document does not fetch them.
- Declaring `fonts.url` makes the document non-portable: the CSP MUST be
  relaxed only for the declared origins, `data-mdhtml-portable="false"` MUST be
  declared, and no other network capability MAY be enabled (external scripts
  remain forbidden).
- `mdhtml check` MUST compute the verdict from content and MUST report
  `E-FMT-03` when the declared attribute contradicts the verdict.

Canonical (portable) CSP:

```text
default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline';
img-src data: blob:; font-src data:; media-src data: blob:
```

Non-portable CSP example (Google Fonts):

```text
default-src 'none'; script-src 'unsafe-inline';
style-src 'unsafe-inline' https://fonts.googleapis.com;
img-src data: blob:; font-src data: https://fonts.gstatic.com;
media-src data: blob:
```

## 6. No-JavaScript fallback (FMT-04)

### FMT-04 — noscript readability

- A built document MUST include a `<noscript>` block that makes the canonical
  source readable as plain text.
- With JavaScript disabled the page MUST NOT be blank and MUST NOT hide the
  canonical source.
- The runtime MUST NOT apply `visibility:hidden` or an equivalent hiding
  technique before rendering.

The `<noscript><style>` in the skeleton renders `#mdhtml-source` as a
preformatted block.

## 7. Metadata (FMT-05)

### FMT-05 — derived metadata

- `build` MUST derive document metadata from the front matter:
  - `<title>` from `title`;
  - `meta[name="description"]` and `meta[property="og:description"]` from
    `summary`;
  - `meta[property="og:title"]` from `title`; `meta[property="og:type"]` is
    `article`;
  - `<html lang>` from `lang` (default `en`);
  - `<link rel="canonical">` and `meta[property="og:url"]` from `url`;
  - `meta[property="og:image"]` only when both `url` and `cover` are present
    and the cover asset resolves.
- `title` MUST be present; its absence is diagnostic `E-FMT-05`.
- `og:image` MUST be an absolute URL. When `cover` is a relative path and `url`
  is set, `build` MUST resolve the cover asset and derive the absolute URL from
  `url`.

## 8. Front matter (PARSE-01)

Front matter is an optional block at the very start of the canonical source:
a line containing exactly `---` at the start, content, then a closing line
containing exactly `---`.

### PARSE-01 — closed YAML subset

Supported:

- scalars: plain, single-quoted (`''` escapes a quote), double-quoted strings,
  finite integers and floats representable by JavaScript and Rust numeric
  types, booleans (`true`/`false`), `null`;
- maps by indentation — spaces only; a leading tab in indentation position is
  an error;
- block sequences (`- item`), flow sequences (`[a, b]`), flow maps
  (`{a: 1}`); maps and sequences nest;
- literal `|` and folded `>` blocks;
- comments (`#` to end of line, outside quoted scalars).

Rejected — each is diagnostic `E-PARSE-01`:

- tabs used for indentation;
- anchors (`&name`) and aliases (`*name`);
- tags (`!!str`, `!foo`);
- duplicate keys (case-sensitive equality of the resolved key);
- numeric syntax whose resolved value is not finite;
- inconsistent indentation (sibling keys at different indentation, or a value
  less indented than its key);
- multiple documents (a second `---` after content has started, or a `...`
  end marker).

Reserved keys:

| Key | Type | Default | Used for |
|---|---|---|---|
| `title` | string | — (required) | `<title>`, OG, smart copy |
| `summary` | string | — | description / OG |
| `lang` | string | `en` | `<html lang>` |
| `theme` | string \| path | `technical` | built-in preset or local `.theme.css` inlined by `build` |
| `tokens` | map | `{}` | per-document token overrides |
| `fonts` | `auto` \| `system` \| map | `auto` | font embedding policy (`{body, mono, url}`) |
| `url` | string | — | canonical + `og:url` |
| `cover` | path | — | `og:image` (with `url`) |
| `toc` | `false` \| map | `{depth: 3, position: side}` | map accepts integer `depth` 1…6 and `position: side|inline`; `false` disables TOC |
| `sections` | map | `{}` | slug → `{component, class}` |
| `figures` | map | `{}` | path → `{align, size, caption, group, shape}` |
| `date`, `authors`, `tags` | scalar \| sequence | — | semantic metadata, smart copy |

- Unknown keys MUST be preserved by smart copy (they may be semantic). The
  presentation keys (`theme`, `tokens`, `fonts`, `sections`, `figures`, `toc`)
  MUST be dropped by smart copy.
- Built-in theme names are exactly `technical` and `editorial`. The runtime
  uses `technical` when a hand-edited artifact names an unknown/non-built
  theme; `build` resolves a local `.theme.css` or rejects an unknown name.
- Both implementations MUST accept exactly this subset; fixture parity is
  enforced by `fixtures/` (§20).

Example:

```yaml
---
title: Quarterly report
summary: A synthetic example document.
date: 2026-08-20
authors: [Ada, Grace]
tags: [reports, synthetic]
theme: technical
toc: { depth: 2, position: side }
sections:
  results: { component: cards }
figures:
  charts/revenue.png: { align: right, size: md }
---
```

## 9. Markdown coverage (PARSE-02)

### PARSE-02 — supported constructs

The runtime MUST parse and render:

- ATX headings `#`–`######` (Setext headings are MAY);
- paragraphs; hard breaks (two or more spaces at end of line; backslash-newline
  is MAY);
- links — inline `[text](url "title")` and reference `[text][id]` with
  definitions;
- images `![alt](path "title")`, resolved as assets per §14;
- blockquotes, including nesting;
- GFM pipe tables (alignment via `:---`);
- thematic breaks (`---`, `***`, `___`);
- code spans (backticks) and fenced code blocks with an optional language;
- lists: nested, ordered (including non-1 starts), unordered, and task lists
  (`[ ]` / `[x]`);
- emphasis `*`/`_`, strong `**`/`__`, strike `~~`;
- footnotes: `[^id]` references and `[^id]:` definitions, rendered as a
  footnote section;
- backslash escapes for ASCII punctuation.

Hostile text MUST be escaped on output. Raw HTML from Markdown content is
escaped by default; raw HTML is not enabled by default and is never enabled
unless explicitly configured.

## 10. Inline precedence (PARSE-03)

### PARSE-03 — code spans shield inline markup

- Code spans MUST be tokenized before emphasis, strong, and links.
- Inside a code span, `*`, `**`, `_`, `[`, backticks, and backslash escapes are
  literal.
- Emphasis MUST NOT span a code span boundary; links MUST NOT open inside a
  code span.
- Regression fixture: `` `a**b**c` `` MUST render as a code span whose text is
  literally `a**b**c` — no `<strong>` inside.

## 11. Sections and slugs (SECT-01)

### SECT-01 — deterministic section identity

- Every heading receives an `id` computed by the slug algorithm:
  1. Unicode lowercase;
  2. NFD normalization; remove combining marks;
  3. replace whitespace runs with `-`;
  4. remove every character not in `[A-Za-z0-9-_]`.
- Collisions MUST be resolved deterministically, in document order, by
  appending `-2`, `-3`, …
- An explicit `{#id}` override (`## Title {#slug}`) MUST replace the computed
  slug. The override is normalized (lowercase, whitespace → `-`), participates
  in the same uniqueness pass (a duplicate override gets `-2`, …), and a
  duplicate override MUST produce warning `W-SECT-01`.
- Every section — a heading plus its content until the next heading of the
  same or higher level — MUST be addressable as `data-md-section="<slug>"`.
- `check` MUST fail with `E-SECT-01` when a key in `sections:` has no matching
  slug in the document (orphan binding).
- Section navigation MUST use hash + `hashchange`; `history.pushState` MUST NOT
  be used (it throws under `file://`).

Example:

```markdown
## Quarterly results {#results}

First section.
## Quarterly results

Second section (slug `quarterly-results-2`).
```

## 12. Containers and components (COMP-01, COMP-02)

### COMP-01 — fenced-container grammar and AST

Containers use Pandoc nested fenced-div semantics. An opener is a line with at
most three leading spaces, at least three consecutive colons, and either one
ASCII-lowercase semantic name or exactly `{.name}`. A name matches
`[a-z][a-z0-9-]*`; the braces are not part of the AST name. An opener MAY have
`| text` after the name. The text after `|` is trimmed; an empty result is
`null`. The trimmed result MUST be preserved in the AST `argument` for every
syntactically valid opener, including names other than `quote` and `details`;
later validation decides whether that argument is allowed. For `quote` it is
the attribution, and for `details` it is the summary.

A line containing only at least three colons, ignoring leading/trailing spaces,
closes the innermost open container; its fence length need not equal the
opener's. The body is recursively parsed as Markdown, including nested
containers. The exact container AST record is
`{type:'container',name,argument,children}`, where `argument` is the trimmed
argument string or `null`, and `children` is the ordered Markdown block AST.
Unknown names that satisfy the name grammar still produce this AST record.
An opener with no matching close is ordinary paragraph text, not a partial
container. A line that is neither a valid opener nor a close is ordinary
Markdown.

Every table AST node MUST also include
`sourceCellCounts: {header:number,rows:number[]}`. These counts are captured
after optional leading/trailing pipe removal and escaped-pipe handling, but
before any cell padding or truncation.

Known container names are exactly `note`, `warning`, `critical`, `success`,
`decision`, `quote`, `stats`, `bars`, `kv`, `steps`, `grid`, `columns`, and
`details`. Names are semantic tokens, not CSS classes. Every generated class
uses the `md-` namespace. A container is valid only when its name is known, its
argument rule is satisfied, and its required body shape below is satisfied.
Unknown names parse as containers but are failed units.

The following rules are strict. “Nonempty” means at least one parsed block;
whitespace and an empty table/list do not count. “Two-column table” means one
GFM table whose header has exactly two cells and whose every body row has
exactly two cells. A missing cell is invalid even if the GFM fallback parser
would pad it, and an extra cell is invalid even if that parser would truncate
or otherwise normalize it. The table must be the only table block required by
the shape. Rendered Markdown inside every wrapper remains the original
inline/block content.

For a structured table, those cardinalities MUST be proved by the preserved
`sourceCellCounts`: `header` MUST equal `2`, every entry in `rows` MUST equal
`2`, and the normalized AST header MUST also have length `2`. A required table
MUST contain at least one body row, and the table MUST be the only top-level
block in its container; any paragraph, list, heading, or other block alongside
it invalidates the whole structured container. Implementations MUST NOT infer
cardinality from padded or truncated normalized cells.

| Name | Required top-level blocks | Exact semantic output |
|---|---|---|
| `note`, `warning`, `critical`, `success`, `decision` | any nonempty blocks | `<aside class="md-callout md-NAME"><span class="md-callout-badge">LABEL</span>children</aside>`; `LABEL` is the title-cased name and is visible |
| `quote` | any nonempty blocks; optional opener argument | `<figure class="md-quote"><blockquote>children</blockquote>[<figcaption>argument</figcaption>]</figure>`; the figcaption is omitted when argument is null |
| `columns` | at least two top-level blocks | `<div class="md-columns"><div class="md-column">block</div>...</div>`; one column contains one original top-level block |
| `details` | any nonempty blocks; optional opener argument | `<details class="md-details"><summary>argument or Details</summary>children</details>` |
| `stats` | exactly one two-column table with at least one body row | `<div class="md-stats">table</div>`; the original table, including header and inline content, is retained |
| `bars` | exactly one two-column table with at least one body row; every second body cell is a finite number `>= 0` | `<div class="md-bars">table-with-meters</div>`; each second body cell is replaced by `<meter min="0" max="M" value="N">N</meter>`, where `M` is the greatest value or `1` when all values are zero |
| `kv` | exactly one two-column table with at least one body row, or exactly one nonempty unordered list whose every item starts with `strong key` followed by `:` | `<dl class="md-kv"><dt>key</dt><dd>value</dd>...</dl>`; all table rows or list item content is preserved as inline content |
| `steps` | exactly one nonempty ordered list | `<ol class="md-steps">items</ol>`; list item blocks and inline content are preserved |
| `grid` | one or more level-3 heading groups and no leading top-level content | `<div class="md-grid"><section class="md-grid-item">h3 and its following blocks</section>...</div>`; each group starts at one `###` and ends before the next `###`; each card section carries the normal `data-md-section` slug and its `h3` carries the same `id` |

The `LABEL` and `summary` text are escaped and rendered as inline Markdown; no
raw HTML is introduced. The ordinary Markdown renderer supplies the `table`,
list, heading, image, and paragraph elements inside these wrappers.

For `steps`, an item containing exactly one paragraph uses the compact form
`<li>inline content</li>`; items with additional blocks retain their nested
block markup. Grid groups are registered in the shared SECT-01 heading registry
in source order, including headings outside the grid, collision suffixes, and
explicit-id warnings. Each card section MUST carry the registered slug in
`data-md-section`, and its `h3` MUST carry the same registered id; the grid
presentation MUST NOT create an identity namespace separate from SECT-01. A
grid group may contain any following Markdown blocks; only leading content
before the first level-3 heading invalidates the container.

### COMP-02 — degradation and diagnostics

A valid container or bound section component applies its transformation. A
failed unit renders its parsed children (for a container) or the original
section normally, omits that unit's component wrapper, retains any nested
warnings, and never fails `build`. Invalid non-`quote`/`details` arguments are
failed container units under this rule. `check` emits exactly one ordered
record for each failed unit:

`{code:'W-COMP-02',name,target}`

Records are ordered by source traversal. `name` is the failed container name or
component name, and `target` is `null` for containers or the bound section slug
for section bindings. An unknown syntactically valid container name and an
unknown component name are failed units. A malformed structured container is
not rendered best-effort; it degrades as a whole. A valid outer container still
renders its wrapper when only a nested unit fails.

### Section bindings and components

Front matter `sections` is a map from an existing section slug to a binding
object. A binding is a mapping with a scalar `component` value and optional
scalar `class` value: `{component: name, class?: text}`. `class` is optional;
when present it is whitespace-separated CSS identifiers, each matching
`[A-Za-z_][A-Za-z0-9_-]*`, and the complete value must contain one or more such
identifiers separated only by whitespace. A valid class string is appended only
to the bound section wrapper, after its `data-md-section` attribute; it is
never copied to an inner component wrapper. A non-mapping binding, a missing or
non-scalar `component`, a non-scalar `class`, or an invalid class string is an
invalid binding and degrades with exactly one `W-COMP-02` record. A binding
whose slug is absent produces `E-SECT-01` from `check` and has no runtime
target.

Binding records and orphan errors MUST follow the source order of the `sections`
mapping, including integer-like slugs; this ordering evidence is parser metadata and
does not change the public plain mapping.

For a non-mapping binding, or a binding whose `component` is missing or not a
scalar string, the warning `name` is the empty string (`''`). An empty scalar
component is also reported with `name:''`; a scalar but unknown component uses
its supplied value as `name`.

The known section components are exactly `timeline`, `cards`, `meters`,
`gallery`, `kv`, `columns`, and `hero`. The following shapes are minimal and
strict; any mismatch, unknown component, or invalid binding leaves the original
section HTML unchanged.

| Component | Required section body | Exact semantic output |
|---|---|---|
| `timeline` | exactly one nonempty ordered or unordered list | `<div class="md-timeline">list</div>` |
| `cards` | one or more immediate child `section` elements and no other body blocks | `<div class="md-cards">sections</div>` |
| `meters` | exactly one two-column table; every second body cell is a finite number from `0` through `100` | `<div class="md-meters">table-with-meters</div>`; meters use `min="0" max="100" value="N"` and fallback text `N` |
| `gallery` | one or more paragraphs, each containing exactly one image | `<div class="md-gallery"><figure class="md-gallery-item"><p>paragraph</p></figure>...</div>` |
| `kv` | the same table/list convention as the `kv` container | `<dl class="md-kv">...</dl>` |
| `columns` | at least two top-level body blocks | `<div class="md-columns"><div class="md-column">block</div>...</div>` |
| `hero` | nonempty body with at most one standalone-image paragraph | `<div class="md-hero"><div class="md-hero-content">non-image blocks</div><div class="md-hero-media">image paragraph when present</div></div>` |

A standalone-image paragraph contains exactly one image and no other inline
content. A hero with no image has an empty `md-hero-media` wrapper; all other
body blocks remain in `md-hero-content`. Component wrappers are inside the
existing `section[data-md-section="slug"]` element, so the section's identity
and its valid bound class remain stable. The wrapper and all nested output
preserve original inline and block content.

These choices deliberately follow Pandoc's nested fenced-div semantics, GFM
tables/lists/headings for readable graceful fallback, and native `details` and
`meter` semantics. No component introduces a data-only authoring syntax.

Example:

```markdown
::: warning
The service runs on **Node 18**.
:::

::: quote | Ada Lovelace
The analytical engine has no pretensions whatever to originate anything.
:::
```

## 13. Browser chrome (UI-01…UI-06)

### UI-01 — toolbar

- The document chrome MUST offer: copy Markdown in modes `smart` (default),
  `full`, and `body`; view Markdown; download `.md`; theme toggle.
- Chrome is mounted only after successful document rendering. Its stable hooks
  are `#mdhtml-toolbar`, `#mdhtml-copy-mode`, buttons carrying
  `data-md-action="copy|view|download|theme"`, and
  `dialog#mdhtml-source-view` containing a `pre` whose `textContent` is the
  viewed projection. Mounting twice MUST NOT duplicate chrome.
- The toolbar is a labelled navigation landmark. Controls use native `select`
  and `button` elements, the mode options occur as `smart`, `full`, `body`, and
  the source-view dialog has a native close button.
- The toolbar MUST NOT appear in print output.
- "View" displays the text the current copy mode produces; "Download" writes
  the full canonical source byte-for-byte as `document.md`, identical to
  `extract` output. Download uses an in-memory Blob URL, clicks a temporary
  anchor, then revokes and removes it without a network request.
- Copy modes: `smart` = semantic front matter (title, summary, date, authors,
  tags, unknown keys) + body; `full` = the raw stored canonical source;
  `body` = prose only, without front matter. Containers survive in every mode.
- `full` never parses or decodes its input. `body` and `smart` first apply the
  defensive canonical-source decoding from FMT-02 and then use the PARSE-01
  parser; their body is the parser's byte-preserved body.
- `smart` retains every top-level front matter key except exactly `theme`,
  `tokens`, `fonts`, `sections`, `figures`, and `toc`. Retained keys follow YAML
  source order, including integer-like keys. When no front matter exists,
  `smart` returns the decoded source unchanged; when no key remains, it returns
  the decoded body without empty delimiters.
- A retained smart-copy mapping is emitted as `---\n`, one top-level entry per
  line, `---\n`, then the body. Keys and string scalars use JSON-compatible
  double quotes; null, booleans, and finite numbers use their literals;
  sequences use `[value, ...]`; mappings use `{key: value, ...}` recursively in
  YAML source order. This canonical form is valid PARSE-01 input and makes copy
  output deterministic without retaining presentation comments or formatting.

### UI-02 — clipboard

- Copy MUST call `navigator.clipboard.writeText` synchronously inside the click
  handler, with no `await` before the call.
- On failure, copy MUST fall back to a readonly `textarea` (fixed position)
  plus `document.execCommand('copy')`; the textarea is removed in every outcome.

### UI-03 — navigation

- TOC and section navigation MUST use hash + `hashchange`. `pushState` and
  `fetch` MUST NOT appear in runtime artifacts.
- A document with headings mounts one labelled `nav#mdhtml-toc` after the
  toolbar. It contains one source-ordered anchor per rendered heading, with
  `href="#ID"`, the plain heading text, and `data-level="N"`; a document with
  no headings or `toc: false` omits the TOC. A map includes headings whose
  level is at most `depth` (default 3) and exposes its normalized `position`
  (default `side`) as `data-md-toc-position`. Invalid map values degrade to
  their defaults; `check` reports the front matter contract separately.
- Mount and every `hashchange` synchronize exactly one matching TOC link with
  `aria-current="location"`, or none when the hash has no heading target.
  Native anchor behavior changes the hash; the runtime neither cancels the
  click nor writes browser history. Repeated mounting MUST NOT duplicate the
  TOC or its listener.

### UI-04 — images

- Assets below 32 KiB (32 768 bytes) MUST be embedded as `data:` URIs.
- Assets at or above 32 KiB MUST be decoded (`atob` → `Uint8Array` → `Blob` →
  `createObjectURL`) lazily via `IntersectionObserver`.
- A missing embedded asset (hand-edited artifact) MUST render its alt text and
  MUST NOT break the document; `check` warns (`W-UI-04`).
- The Markdown renderer emits image references as `<img data-md-asset-path="PATH"
  alt="ALT">` (plus an escaped title when present), without `src`. This prevents
  a relative `file://` or network read before the embedded block is validated.
- Runtime lookup matches `data-md-asset-path` exactly to one
  `script[type="application/octet-stream"][data-path]`. A duplicate path,
  missing block, unsupported image MIME, or malformed standard padded base64 is
  unavailable and leaves the image without `src`, marked
  `data-md-asset-missing=""`; hydration returns one ordered
  `{code:'W-UI-04',path}` record for it.
- Base64 ASCII whitespace is removed before validation. The 32 KiB boundary is
  the decoded byte length. A payload of at most 32 767 bytes receives
  `src="data:MIME;base64,PAYLOAD"` immediately and
  `data-md-asset-ready=""`.
- A payload of at least 32 768 bytes keeps no `src` until its observer entry is
  intersecting. The observer then performs the specified decode pipeline once,
  sets the Blob URL and `data-md-asset-ready=""`, and unobserves that image.
  Missing required browser primitives degrade to the same alt-only unavailable
  state. Repeated hydration MUST NOT duplicate work.

### UI-05 — lightbox

- Lightbox MUST use `<dialog>` + `showModal()`. Focus return, `Esc`, and
  backdrop are native behaviors and MUST NOT be reimplemented.
- Lightbox MUST support arrow keys, swipe, a counter, and
  `prefers-reduced-motion`.
- When at least one non-missing asset image exists, runtime mounts exactly one
  `dialog#mdhtml-lightbox` with an image, an `output` carrying
  `data-md-lightbox-counter`, and native buttons carrying
  `data-md-lightbox-action="previous|next|close"`. Repeated mounting MUST NOT
  duplicate the dialog or image listeners.
- Clicking an image opens the dialog only when that image has
  `data-md-asset-ready`; the dialog copies its current `src`, `alt`, and optional
  `title` without HTML interpolation. The gallery is the current DOM-ordered
  set of ready, non-missing asset images, so a lazily hydrated image joins when
  it becomes ready. The counter text is `INDEX / TOTAL` using one-based index.
- Previous/next buttons and `ArrowLeft`/`ArrowRight` wrap at both ends. A
  horizontal pointer gesture of at least 40 CSS pixels invokes the same wrapped
  previous/next transition; shorter or predominantly vertical gestures do
  nothing. No listener handles `Escape` or backdrop clicks, and no focus trap
  or manual focus-return code is present.
- `matchMedia('(prefers-reduced-motion: reduce)')` sets
  `data-md-reduced-motion=""` on the dialog. Runtime CSS disables animation,
  transition, and smooth scrolling for that hook and in the matching media
  query.

### UI-06 — themes and print

- Light, dark, and system themes MUST provide complete token sets; contrast
  and visible focus MUST hold in every theme.
- The complete runtime color set is `--md-bg`, `--md-text`, `--md-muted`,
  `--md-surface`, `--md-border`, `--md-accent`, and `--md-focus`. Explicit
  light/dark selectors define every token; system defaults to light and
  overrides every token inside `prefers-color-scheme: dark`.
- The root `data-mdhtml-theme` value is `system`, `light`, or `dark`. A newly
  mounted document starts at `system`; the toolbar cycles
  `system → light → dark → system` without persistence or network access.
- Runtime CSS lives in one idempotently mounted `style#mdhtml-runtime-style`.
  Keyboard focus uses a visible `:focus-visible` outline. Print hides the
  toolbar, TOC, and source dialog; removes app width/overflow clipping; and
  prints with a white background and black text.
- The shared visual token set additionally includes `--md-font-body`,
  `--md-font-mono`, `--md-measure`, `--md-density`,
  `--md-heading-scale`, `--md-radius`, and `--md-shadow`. `technical` sets
  Instrument Sans, 78ch, 0.88, and 1.18; `editorial` sets Newsreader, 68ch,
  1, and 1.25. Both use Geist Mono and define radius/shadow. Presets contain
  token declarations only; all element, component, chrome, lightbox, dark-mode,
  reduced-motion, and print rules live in the shared base stylesheet.
- The selected built-in preset is exposed as root
  `data-mdhtml-preset="technical|editorial"`. Theme source files are the
  authority; the JavaScript CSS module is generated deterministically and
  `runtime/build.mjs check` fails on drift.

## 14. Assets

Asset references in the source are normal relative paths: Markdown images
(`![alt](path)`), `<img src="path">`, `figures:` keys, and
`fonts.body`/`fonts.mono`. `build` resolves them relative to the source file
and embeds each as an asset block:

```html
<script type="application/octet-stream" data-path="images/photo.png" data-type="image/png">BASE64…</script>
```

- Payloads are standard base64 (RFC 4648, with padding); embedded newlines and
  whitespace MUST be ignored when decoding.
- MIME mapping is a closed table; an extension outside the table is diagnostic
  `E-CLI-01` at build time.

| Extension | MIME |
|---|---|
| `.png` | `image/png` |
| `.jpg` `.jpeg` | `image/jpeg` |
| `.gif` | `image/gif` |
| `.webp` | `image/webp` |
| `.svg` | `image/svg+xml` |
| `.woff2` | `font/woff2` |
| `.css` | `text/css` |

- Asset payloads MUST be extracted byte-exactly (no re-encoding), so
  `build` → `extract --assets` reproduces the original tree.
- A missing asset file at build time is diagnostic `E-CLI-01` and fails without
  writing output.

## 15. CLI contract (CLI-01…CLI-05)

Interface:

```text
mdhtml build <in.md> [-o out] [--watch] [--no-fonts]
mdhtml check <file>            # .md or .md.html
mdhtml extract <in.md.html> [-o out.md] [--assets dir]
mdhtml new <name> [--template resume|memo|spec|recipe|chapter]
mdhtml themes
```

### CLI-01 — build

- `build` MUST validate, resolve assets, embed template/runtime/theme/fonts,
  select the required runtime fragments (§17), and write the document
  atomically.
- Atomic output: write to a temporary file in the destination directory, then
  rename over the destination. Any validation error MUST leave the destination
  untouched — never a partial or truncated `.md.html`.
- `</script` input (FMT-02), unresolvable assets, out-of-table MIME, invalid
  front matter, and a missing `title` all fail before any output is written.

### CLI-02 — check

- `check` accepts `.md` sources and built `.md.html` artifacts.
- Normative violations are errors (nonzero exit); convention issues are
  warnings (exit 0). The report MUST include the portability verdict and the
  byte budget by category (content / runtime / fonts / images).
- A portable document MUST report zero external requests.

### CLI-03 — extract

- `extract` MUST restore the canonical source byte-for-byte (Unicode and
  newlines preserved; `<\/script` is NOT decoded). The round-trip
  `build` → `extract` MUST produce a byte-identical file.
- `--assets` writes embedded assets under the given directory, preserving
  `data-path`. Paths MUST be relative, MUST NOT contain `..` segments, MUST NOT
  be absolute or URL-based. Invalid base64, a duplicate `data-path`, or an
  unsafe path fails with `E-CLI-03` before anything is written (no partial
  extraction).
- Extraction MUST NOT silently overwrite an existing file: a collision with an
  existing target — the `-o` output or any asset file under `--assets` — fails
  with `E-CLI-03` before anything is written.

### CLI-04 — new, themes, no-fonts, watch

- `new` materializes one of the five canonical templates; `themes` lists the
  built-in presets.
- `--no-fonts` is equivalent to `fonts: system`.
- `--watch` polls the input and rebuilds once per change, is idempotent (a
  rerun does not duplicate or destroy files), and stops cleanly on a signal.

### CLI-05 — diagnostics to users

- User-facing errors MUST be short and actionable: one line, no stack traces,
  no internal URLs, no environment-variable names.
- Message format: `mdhtml: <code>: <message>`.

## 16. Diagnostics

A **Diagnostic** is a stable record with a code, severity, and message. Codes
are part of the public contract: fixtures reference them, and both
implementations MUST produce the same code for the same condition.

- `error` — violates a MUST; `build`/`check` exit nonzero; `build` writes no
  output.
- `warning` — convention-level deviation; reported by `check` (and, where
  noted, by the runtime); exit code unaffected.
- `info` — portability verdict and byte-budget report from `check`.

Code scheme: `<E|W|I>-<REQ-ID>`, where `REQ-ID` is the requirement identifier.
Enumerated codes:

| Code | Condition |
|---|---|
| `E-FMT-01` | missing/duplicate `#mdhtml-source` or wrong type |
| `E-FMT-02` | `</script` in input |
| `E-FMT-03` | portability attribute contradicts content |
| `E-FMT-05` | missing `title` |
| `E-PARSE-01` | front matter violation (§8) |
| `E-SECT-01` | orphan slug in `sections:` |
| `W-SECT-01` | duplicate explicit `{#id}` |
| `W-COMP-02` | container/component out of convention |
| `E-CLI-01` | unresolvable asset or out-of-table MIME |
| `E-CLI-03` | unsafe path, invalid base64, duplicate asset path, existing target file |
| `W-UI-04` | missing embedded asset (hand-edited artifact) |
| `I-CLI-02` | portability + byte-budget report |

Implementations MAY add diagnostics for conditions outside this table (for
example, I/O errors) but MUST NOT change the code or severity of the
enumerated set.

## 17. Runtime fragment manifest

The runtime ships as classic IIFE fragments plus a manifest. The CLI
concatenates only the fragments a document requires; `runtime.min.js` is the
full reference bundle (all fragments in dependency order), committed for
inspection but NOT the mandatory embedding unit.

The executable fragment ids are exactly `core`, `copy`, `toc`, and `lightbox`.
`core` owns canonical parsing/rendering, containers and section components,
asset hydration, styles, and the shared boot state. Component rendering stays
in `core`: it shares parser/heading/warning context and splitting each renderer
would add a global protocol larger and less reliable than the saved branch
code. Optional fragments read the shared state through one private global
symbol and no other global namespace.

Fragment rules:

- Each fragment is a classic IIFE. Fragments MUST NOT use `import`/`export`,
  `fetch`, `pushState`, or network APIs.
- The manifest is committed JSON (`mdhtml/manifest/1.0`):

```json
{
  "format": "mdhtml/manifest/1.0",
  "fragments": [
    { "id": "core", "file": "core.min.js", "size": 14336, "sha256": "…", "requires": [] },
    { "id": "copy", "file": "copy.min.js", "size": 1024, "sha256": "…", "requires": ["core"] },
    { "id": "toc", "file": "toc.min.js", "size": 1024, "sha256": "…", "requires": ["core"] },
    { "id": "lightbox", "file": "lightbox.min.js", "size": 3072, "sha256": "…", "requires": ["core"] }
  ]
}
```

- `sha256` covers the exact committed bytes of the fragment file; `size` is its
  byte length; `requires` lists fragment ids that MUST appear earlier in the
  concatenation.
- Concatenation MUST be topological in `requires`, with ties broken by manifest
  order.
- Selection rules: `core` and `copy` always; `toc` when at least one heading is
  within the normalized depth and `toc` is not `false`; `lightbox` when the
  document contains an image. No `highlight` or per-component fragment exists
  in 1.0.
- `runtime.min.js` is the byte-for-byte concatenation of every fragment in
  manifest order, with no additional wrapper. Each individual fragment and
  the concatenation MUST execute as classic script; optional fragments are
  safe no-ops when core failed or their document evidence is absent.
- Builds MUST be reproducible: building from the same sources produces
  byte-identical fragments. `runtime/build.mjs check` MUST detect drift between
  freshly built and committed artifacts.

## 18. Byte budgets

- `check` MUST report bytes by category: content (canonical source), runtime
  (selected fragments), fonts (embedded font bytes), images (embedded asset
  bytes).
- Image embedding threshold: 32 KiB (32 768 bytes). Below: `data:` URI; at or
  above: lazy Blob URL (UI-04).
- Font embedding MUST follow the budget rules:
  - variable fonts use `wght@min..max` only — never `opsz`;
  - italic faces are embedded only when the document uses emphasis;
  - the mono face is embedded only when the document contains code;
  - chrome (toolbar, TOC, badges) uses the system stack, never embedded fonts;
  - `fonts: system` embeds no font bytes;
  - every embedded family MUST be OFL/Apache-licensed, MUST be committed with
    its license notice, and MUST NOT be re-subsetted.
- The committed built-in catalog is `fonts/catalog.json`
  (`mdhtml/fonts/1.0`). `technical` selects Instrument Sans body plus Geist
  Mono; `editorial` selects Newsreader body plus Geist Mono. `system` selects
  no file. Body normal is always selected, body italic only when emphasis is
  present, and mono only when code is present.
- Built-in files are existing upstream `latin` WOFF2 distributions, never a
  locally generated subset: Instrument Sans normal/italic carry only `wght`
  400…700; Newsreader normal/italic carry only `wght` 200…800; Geist Mono
  normal carries only `wght` 100…900. The catalog records exact file, style,
  weight range, byte size, SHA-256, upstream URL/version/integrity, license,
  and notice path for every face.
- `fonts/check.mjs` verifies the catalog schema, WOFF2 magic, exact size/hash,
  notices, absence of an `opsz` declaration, and the selection fixtures
  offline. No normal build or test downloads font bytes.
- The release CLI binary MUST NOT exceed 600 KiB (614400 bytes); CI enforces
  the limit and fails the build otherwise. This ceiling is the measured floor
  for a statically-linked Rust release binary with the standard
  size-optimized profile, not an arbitrary figure.

## 19. Product and distribution (PROD-01…DIST-02)

### PROD-01 — reference examples

- `resume`, `memo`, `spec`, `recipe`, and `chapter` are the five canonical
  templates/examples. They MUST be synthetic (no personal or
  production-derived data), MUST build and pass `check` cleanly, and MUST have
  an empty round-trip.

### PROD-02 — authoring skill

- `skills/mdhtml-author` MUST teach the authoring flow and conventions using
  the same canonical templates and examples — no divergent copies — and MUST
  close with `build` + `check`.

### DIST-01 — install

- `install.sh` MUST run under `set -euo pipefail`, be idempotent, and verify
  the downloaded binary checksum.

### DIST-02 — CI and release

- CI MUST run: runtime checks, Rust tests, example build/check/round-trip,
  Sentrux gates, deterministic E2E, and the binary-size limit.
- Release MUST produce checksums for the declared targets: macOS arm64/x64,
  Linux gnu/musl, Windows.

## 20. Conformance fixtures

- `fixtures/` is the shared, executable contract. Valid cases carry source plus
  expected output; invalid cases carry a stable diagnostic code.
- The suite MUST cover every requirement identifier, the `` `a**b**c` ``
  regression (PARSE-03), front matter validity/invalidity (PARSE-01), slug
  collisions and overrides (SECT-01), container nesting and degradation
  (COMP-01/02), asset thresholds (UI-04), and diagnostic codes (§16).
- Harnesses MUST be deterministic and MUST NOT access the network.

## Traceability

| Requirement | Section |
|---|---|
| FMT-01 | §2 |
| FMT-02 | §4 |
| FMT-03 | §5 |
| FMT-04 | §6 |
| FMT-05 | §7 |
| PARSE-01 | §8 |
| PARSE-02 | §9 |
| PARSE-03 | §10 |
| SECT-01 | §11 |
| COMP-01 | §12 |
| COMP-02 | §12 |
| UI-01 | §13 |
| UI-02 | §13 |
| UI-03 | §13 |
| UI-04 | §13 |
| UI-05 | §13 |
| UI-06 | §13 |
| CLI-01 | §15 |
| CLI-02 | §15 |
| CLI-03 | §15 |
| CLI-04 | §15 |
| CLI-05 | §15 |
| PROD-01 | §19 |
| PROD-02 | §19 |
| DIST-01 | §19 |
| DIST-02 | §19 |
