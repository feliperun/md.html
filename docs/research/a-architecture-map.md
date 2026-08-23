# Current-State Architecture Map of mdhtml (Agent A research input)

Research input for the mdhtml Safe-by-Default & Public Hosting Tech Spec
(`docs/prd/mdhtml-safety-hosting-prd.md` §47–§48, Agent A). This document is a
factual map of the system as it exists today; it proposes no security policy.

## Overview

mdhtml is a spec-first document format: Markdown is the canonical source, HTML
is its self-contained presentation, and both live in one `.md.html` artifact.
The frozen v1.0 contract is `SPEC.md` ("single normative authority", §1), with
two conforming implementations: a dependency-free classic-JavaScript browser
runtime (`runtime/src/`) and a std-only Rust CLI (`crates/mdhtml/src/`), both
required to pass the shared fixture suite (`SPEC.md` §1, §20; ADR 0004).

The artifact model (SPEC §2, FMT-01): a document declares
`data-mdhtml="1.0"` on `<html>` and contains exactly one
`script#mdhtml-source[type="text/markdown"]` whose `textContent` is the
canonical source (front matter + body). Everything else in the artifact —
metadata, styles, asset blocks, runtime fragments, CSP — is derived data and
must never become source. `mdhtml build` embeds the source verbatim;
`mdhtml extract` restores it byte-for-byte (SPEC §3, §15 CLI-03). The
byte-exact round-trip `SHA256(original) == SHA256(extracted)` is the
project's core invariant (PRD §3; SPEC CLI-03; enforced by fixtures, not by a
digest stored in the artifact — see "Source embedding and extraction").

High-level flow (`docs/ARCHITECTURE.md`): `doc.md` → `mdhtml build` →
`doc.md.html` (single file, everything embedded, CSP closes the network) →
browser opens via `file://` and the runtime renders the canonical block;
`mdhtml extract` returns the Markdown byte-for-byte.

The PRD context for this map: safe-by-default generation and public hosting
(PRD §1). The PRD's guiding principle is that "safety is a property of the
generated artifact, not a property of the hosting provider" (PRD §1, §60),
while authors keep extensive HTML/CSS customization control (PRD §1, §59
Constraint 3). This map inventories where author-controlled content flows into
generated output so the Tech Spec can draw the threat model.

## Rust CLI pipeline

### Top-level wiring

- `crates/mdhtml/src/main.rs` collects `std::env::args_os()`, calls
  `mdhtml::run_cli` from `crates/mdhtml/src/lib.rs`, prints `Ok(text)` to
  stdout or the error to stderr with exit 1.
- `lib.rs` exposes modules `analysis`, `build`, `check`, `cli`, `commands`,
  `extract`, `frontmatter`, `scanner`, `selection`; `run_cli` calls
  `cli::parse_args` then `commands::dispatch`.
- `crates/mdhtml/src/cli.rs` defines `Command` (`Build`, `Check`, `Extract`,
  `New`, `Themes`), `parse_args` and per-command parsers
  (`parse_build`, `parse_check`, `parse_extract`, `parse_new`,
  `parse_themes`). All argument errors are `E-CLI-05` via `CliError`.
- `crates/mdhtml/src/commands.rs` `dispatch` routes each command:
  - `build` (and `--watch`) → `read_source` → `repository_layout()` (returns
    `runtime/dist`, `themes`, `fonts` relative to `CARGO_MANIFEST_DIR`) →
    `build_document` → `build::build` or `build::build_no_fonts` →
    `write_atomic` (temp file + rename in the destination directory).
  - `watch` polls every `WATCH_POLL_INTERVAL` (200 ms), rebuilds only on byte
    change, always through the atomic write.
  - `check` → `is_artifact` (`.html` extension) → `check::check_artifact`
    for built documents or `check::check_source` for `.md`; nonzero exit via
    `E-CLI-02` when the report `has_errors()`.
  - `extract` → `extract::extract_source` and optionally
    `extract::extract_assets`, staged writes, refuses to overwrite existing
    output (`E-CLI-03`).
  - `new` materializes one of five templates (default `memo`, `E-CLI-04` on
    existing target); `themes` lists built-in themes.

### Build pipeline (`crates/mdhtml/src/build/mod.rs`)

`assemble_document` is the whole pipeline, in order:

1. **FMT-02 gate**: `contains_script_terminator` rejects any input containing
   `</script` (case-insensitive) with `E-FMT-02` before anything else.
2. **Analysis**: `analysis::analyze_document` (see below). Any diagnostic with
   `Severity::Error` fails the build with its code (e.g. `E-FMT-05` missing
   title, `E-PARSE-01` front matter, `E-SECT-01` orphan binding).
3. **Toolchain integrity**: `selection::load(runtime_dir)` loads
   `runtime/dist/manifest.json` and verifies schema, per-fragment `size`,
   `sha256`, `requires` ordering and the `runtime.min.js` concatenation
   invariant (`crates/mdhtml/src/selection/manifest.rs`, codes
   `E-MANIFEST-01`/`E-MANIFEST-02`); `selection::fonts::load`
   (`crates/mdhtml/src/selection/fonts.rs`, codes `E-FONTS-01`/`E-FONTS-02`)
   validates `fonts/catalog.json` the same way. Both fail closed.
4. **Body extraction**: `frontmatter::parse_front_matter` re-parses the source
   to get the body bytes.
5. **Runtime embedding**: `embed_runtime` →
   `selection::select_fragments(body, analysis, manifest)`
   (`selection/manifest.rs`): `core` and `copy` always; `toc` when a heading
   level ≤ the normalized `toc.depth` exists and `toc` is not `false`;
   `lightbox` when the document contains an image. The CLI reads each selected
   fragment file from `runtime/dist/` and concatenates them in manifest order
   into `#mdhtml-runtime`.
6. **Styles**: `embed_styles` reads `themes/base.css` plus
   `technical.theme.css` or `editorial.theme.css` (concatenated into
   `#mdhtml-theme`), renders `tokens:` front matter into CSS custom
   properties (`render_tokens` + `escape_css`, into `#mdhtml-tokens`), and
   for `Theme::Local(name)` inlines the author's local `.theme.css` raw into
   `#mdhtml-user` after only an `is_plain_file_name` check (no `.`, `..`,
   separators) — no CSS validation.
7. **Fonts**: `assets::embed_fonts` → `selection::fonts::select_faces`
   (`crates/mdhtml/src/selection/fonts.rs`): `auto` resolves the preset by
   theme (`technical`/`editorial`), body normal always, body italic only when
   the scanner reports `has_emphasis`, mono only when `has_code`; `system`
   embeds nothing; a `fonts:` map embeds nothing. Each selected face becomes
   an `@font-face` whose `src` is a base64 `data:font/woff2` URI of the exact
   committed file bytes (`crates/mdhtml/src/build/assets.rs`).
8. **Assets**: `assets::embed_assets` → `collect_asset_paths` (scanner image
   destinations that are relative paths, then `figures:` keys, then
   `fonts.body`/`fonts.mono`), `mime_for_path` against the closed
   `MIME_BY_EXTENSION` table (8 entries, anything else `E-CLI-01`),
   `base64_encode` of the exact bytes. `is_relative_path` only rejects empty,
   absolute (`/`-prefixed) and scheme-bearing destinations; it does not reject
   `..` segments (see Open questions).
9. **og:image**: `assets::og_image` requires both `url` and `cover`; derives
   an absolute URL by string concatenation (`absolute_url`) after
   `fs::metadata` on the resolved cover. The cover file is not embedded.
10. **CSP**: canonical `build::CSP`
    (`default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline';
    img-src data: blob:; font-src data:; media-src data: blob:`) when
    `fonts` has no `url`; otherwise `assets::relaxed_csp(url)` relaxes
    `style-src`/`font-src` with `origin_of(url)` and marks the document
    `data-mdhtml-portable="false"`.
11. **Assembly**: `assemble` writes the FMT-01 skeleton — escaped metadata
    (`escape_html`) from `title`/`summary`/`lang`/`url`/og:image, the three
    (or four) style blocks, the `<noscript>` fallback (SPEC §6 FMT-04), the
    source verbatim in `#mdhtml-source`, one
    `script[type="application/octet-stream"][data-path][data-type]` block per
    asset, and the concatenated runtime in `#mdhtml-runtime`.

### Analysis (`crates/mdhtml/src/analysis/`)

`analysis/mod.rs` `analyze_document`:

- `frontmatter::parse_front_matter` (`frontmatter/mod.rs`, grammar split
  across `frontmatter/block.rs`, `flow.rs`, `scalar.rs`) — strict PARSE-01
  subset; the Rust implementation is the reference grammar implementation
  (ADR 0004).
- `config::normalize` (`analysis/config.rs`) — reserved-key normalization for
  `title`, `summary`, `lang`, `theme`, `tokens`, `url`, `cover`, `sections`,
  `figures`, `fonts`, `toc` into `NormalizedConfig`; unknown values warn and
  degrade, never error (except missing/invalid `title` = `E-FMT-05`).
- `scanner::scan_document` (`scanner/mod.rs` → `scanner/lines.rs`,
  `scanner/inline.rs`) — evidence-only scan: `HeadingEvidence` (level, text,
  explicit `{#id}`, offsets), `ImageEvidence` (kind Markdown|Html,
  destination), `ContainerEvidence` (name, argument, body range),
  `has_emphasis`, `has_code`. This is not a Markdown parser; the CLI never
  runs one (ADR 0004, `docs/ARCHITECTURE.md`).
- `slug::compute_sections` + `slugify` (`analysis/slug.rs`, ASCII-safe
  variant `analysis/slug_ascii.rs`) — SECT-01 slug algorithm, `{#id}`
  overrides, collision suffixing, section body byte ranges, `W-SECT-01`
  duplicate overrides.
- `bindings::validate` (`analysis/bindings.rs`) — `sections:` schema,
  component name, CSS-identifier class list (`valid_class`), orphan
  bindings (`E-SECT-01`); shape-valid bindings become `PendingBinding`.
- `section_components::validate` (`analysis/section_components.rs`) — per
  component shape predicates (`timeline`, `cards`, `meters`, `gallery`,
  `kv`, `columns`, `hero`) over `shape::classify` (`analysis/shape.rs`,
  a shallow, convention-only classifier, not a parser); failed shapes become
  `W-COMP-02` and a `DegradedBinding`.
- `containers::validate` (`analysis/containers.rs`) — COMP-02 name/argument/
  body-shape validation for the 13 known container names over the same
  classifier; failures are `W-COMP-02` warnings (content degrades to prose).

### Check (`crates/mdhtml/src/check/mod.rs`)

- `check_source` reuses `analyze_document`, adds selected-runtime and
  selected-font byte totals from the committed manifest/catalog, and computes
  the portability verdict from declared fonts origins plus image
  destinations (`content_verdict`).
- `check_artifact` scans the artifact structurally (`scan_elements`,
  mirroring extract's scanner): FMT-01 identity, CSP exactness against
  `build::CSP` or the derived `relaxed_csp` (`E-FMT-03`),
  `data-mdhtml-portable` consistency, `W-UI-04` missing embedded assets, and
  byte budgets read from the document itself (content/runtime/fonts/images).
  External origins are collected only from `<script src>`, `<img src>`,
  `<link rel=stylesheet href>` and `url(...)` inside `<style>` text
  (`collect_html_origins`, `collect_attr_origin`, `collect_css_url_origins`).

### Where author-controlled content flows into build output

- Markdown source → verbatim `#mdhtml-source` text (gated only by
  `</script`), `build/mod.rs::assemble`.
- `title`, `summary`, `lang`, `url`, `cover` → HTML-escaped metadata
  attributes/OG tags, `build/mod.rs::assemble` + `escape_html`.
- `tokens:` → CSS custom properties, `build/mod.rs::render_tokens` +
  `escape_css` (escapes `\ ; { }` newline NUL).
- Local `theme:` file → raw author CSS in `#mdhtml-user`, `embed_styles`.
- `fonts.url` → inserted into the CSP `style-src`/`font-src` directives,
  `build/assets.rs::relaxed_csp` + `origin_of` (no URL validation beyond
  `origin_of`'s scheme/host extraction).
- Asset paths (image destinations, `figures:` keys, `fonts.body`/`mono`) →
  `fs::read(source_dir.join(path))` then base64, `build/assets.rs`.
- `cover` + `url` → og:image absolute URL by string concatenation,
  `build/assets.rs::absolute_url`.

## JS runtime pipeline

### Assembly tooling (`runtime/build.mjs`, `runtime/build-styles.mjs`)

- `runtime/build.mjs` defines `FRAGMENT_DEFINITIONS` in fixed order
  `core`, `copy`, `toc`, `lightbox` (each with `requires`), bundles each
  entry file with esbuild as a minified classic IIFE (`format: "iife"`,
  `charset: "ascii"`), `normalizeBytes` (LF-only, ASCII-only), then writes
  `runtime/dist/core.min.js`, `copy.min.js`, `toc.min.js`,
  `lightbox.min.js`, `runtime.min.js` (exact concatenation) and
  `manifest.json` (`mdhtml/manifest/1.0`, per-fragment `size`/`sha256`/
  `requires`). `runtime/build.mjs check` detects drift vs the committed
  artifacts.
- `runtime/build-styles.mjs` embeds `themes/base.css`,
  `themes/technical.theme.css`, `themes/editorial.theme.css` as
  `BASE_CSS`/`TECHNICAL_CSS`/`EDITORIAL_CSS` into
  `runtime/src/styles.generated.js`; drift-checked the same way. These are
  the same theme sources the Rust CLI reads at build time
  (`crates/mdhtml/src/build/mod.rs::embed_styles`), giving one theming
  source for both sides.
- Entry files: `runtime/src/entry-core.js` (mounts core only),
  `entry-copy.js`, `entry-toc.js`, `entry-lightbox.js` (each mounts only if
  `globalThis[Symbol.for("mdhtml.runtime.1")]` has `result.ok === true`);
  `runtime/src/bootstrap.js` is the ESM composition used by tests/dev only
  (ADR 0005, `docs/ARCHITECTURE.md`).

### Core mount (`runtime/src/core.js`, `canonical.js`, `frontmatter.js`)

- `mountCore(doc)` (entry `entry-core.js`) reads
  `script[type="text/markdown"]#mdhtml-source` `textContent`, calls
  `mountDocument`, then on success `hydrateImages(doc)` (assets) and
  `mountStyles(doc, frontMatter.theme)` (styles), and freezes the shared
  evidence `{ result, storedSource, images, projectMarkdown }` under
  `globalThis[Symbol.for("mdhtml.runtime.1")]`.
- `mountDocument` (core.js): `formatIsValid` requires `data-mdhtml="1.0"`,
  exactly one `script[type="text/markdown"]` with `id="mdhtml-source"` and
  one `#mdhtml-app` (`E-FMT-01` otherwise); `parseFrontMatter(
  decodeCanonicalSource(source))`; `renderMarkdown(body, { sections })`
  when front matter has `sections`; then `app.innerHTML = rendered.html`
  and sets `data-mdhtml-runtime="1.0"` / `data-mdhtml-ready`.
- `canonical.js` `decodeCanonicalSource` replaces `<\/script` →
  `</script` (case-insensitive) before parsing (SPEC §4 FMT-02);
  `projectMarkdown(storedSource, mode)` implements the copy projections:
  `full` returns stored bytes verbatim; `body` and `smart` decode then
  re-serialize; `smart` drops exactly `PRESENTATION_KEYS`
  (`theme`, `tokens`, `fonts`, `sections`, `figures`, `toc`) and emits a
  canonical YAML form (SPEC §13 UI-01).
- `frontmatter.js` implements the same PARSE-01 subset as the Rust parser
  (`FrontMatterError` with `E-PARSE-01`, ordered mapping entries, byte-
  preserved body slice).

### Parsing and rendering (`runtime/src/markdown.js`, `render.js`)

- `markdown.js`: `parseMarkdownBlocks` (block AST: headings, paragraphs,
  quotes, lists incl. tasks, tables with `sourceCellCounts`, fenced code,
  thematic breaks, fenced containers via `CONTAINER_OPEN_RE`/
  `CONTAINER_CLOSE_RE`, footnote definitions) and `parseInline` (code spans
  tokenized first per PARSE-03, emphasis/strong/strike, links, images,
  escapes, footnotes). Malformed input degrades to text, never drops
  (SPEC §9 PARSE-02).
- `render.js`: `renderMarkdown`/`renderDocument` serialize the AST to HTML.
  All text and attributes go through `escapeText`/`escapeAttribute`
  (escapes `& < >` and `" '`); hostile text is escaped on output; raw HTML
  from Markdown is never enabled (SPEC §9). Links render as
  `<a href="...">` with no scheme allowlist; images render as
  `<img data-md-asset-path="..." alt="...">` with **no** `src` from author
  input — `src` is assigned only by `assets.js` from validated embedded
  blocks (SPEC §13 UI-04).
- Containers (`renderContainer`, render.js): callouts
  (`note|warning|critical|success|decision` → `<aside class="md-callout
  md-NAME">`), `quote` (figure/figcaption from `| text` argument), `columns`,
  `details` (summary from argument), plus `stats`, `bars`, `kv`, `steps`,
  `grid`; any invalid unit degrades to plain prose with `W-COMP-02`
  (`degradeContainer`).
- Section components (`prepareSectionBindings`, `renderSectionNode`,
  `renderSectionComponent`, render.js): `timeline`, `cards`, `meters`,
  `gallery`, `kv`, `columns`, `hero`, keyed by slug from front matter
  `sections:`; the author's `class:` value is validated by `CLASS_RE` and
  emitted as `class="..."` only when shape-valid; invalid bindings degrade
  and report `W-COMP-02`/`E-SECT-01`.

### Optional surfaces (`runtime/src/chrome.js`, `navigation.js`,
`lightbox.js`, `assets.js`, `styles.js`)

- `chrome.js` `mountChrome` builds the toolbar (Copy / View Markdown /
  Download / Theme), the copy-mode `<select>` (`smart|full|body`), the source
  `<dialog>`, clipboard handling (`copyText`: synchronous
  `navigator.clipboard.writeText` inside the click handler, fallback to a
  fixed readonly `textarea` + `execCommand('copy')`), and Blob-URL download.
- `navigation.js` `mountToc` builds `nav#mdhtml-toc` from rendered headings
  up to `toc.depth` (default 3), `position: side|inline` on the root, hash +
  `hashchange` scrolling, `aria-current="location"` sync; no `pushState`,
  no `fetch` (SPEC §13 UI-03; they throw under `file://`).
- `lightbox.js` `mountLightbox` mounts one `dialog#mdhtml-lightbox` with
  `showModal()`, native focus/Esc/backdrop, arrow keys + swipe
  (≥40 px horizontal), counter, `prefers-reduced-motion` hook (SPEC §13
  UI-05).
- `assets.js` `hydrateImages`: matches `data-md-asset-path` exactly to one
  `script[type="application/octet-stream"][data-path]`; payloads < 32 KiB
  become `data:` URIs immediately; ≥ 32 KiB are decoded
  (`atob` → `Uint8Array` → `Blob` → `createObjectURL`) lazily via
  `IntersectionObserver`; closed `IMAGE_MIMES` set, strict
  `validBase64`, duplicate paths/missing blocks/bad MIME/bad base64 →
  alt-only with `W-UI-04` (SPEC §13 UI-04).
- `styles.js` `mountStyles` installs one idempotent
  `style#mdhtml-runtime-style` from `styles.generated.js` and sets
  `data-mdhtml-preset` (SPEC §13 UI-06).

## HTML/CSS customization surface

Every mechanism an author can use to influence the generated HTML/CSS, with
the code that consumes it:

1. **Markdown body** (the canonical source) — rendered by
   `runtime/src/render.js` from the AST of `runtime/src/markdown.js`; all
   text escaped, raw HTML never enabled (SPEC §9). The Rust CLI never parses
   Markdown; it only scans evidence (`crates/mdhtml/src/scanner/`).
2. **Fenced containers** `:::name | argument` — semantic names
   (`note`, `warning`, `critical`, `success`, `decision`, `quote`, `stats`,
   `bars`, `kv`, `steps`, `grid`, `columns`, `details`; COMP-01) validated by
   `crates/mdhtml/src/analysis/containers.rs` and rendered by
   `render.js::renderContainer`; unknown names degrade to prose.
3. **Section components** `sections: {slug: {component, class}}` — component
   set `timeline|cards|meters|gallery|kv|columns|hero`; slug identity from
   headings/`{#id}` (`analysis/slug.rs`, `render.js::registerHeading`);
   class validated as CSS identifiers (`analysis/bindings.rs::valid_class`,
   `render.js::CLASS_RE`) and emitted on the `<section>` only on valid shape
   (`analysis/section_components.rs`, `render.js::renderSectionNode`).
4. **`theme:`** — built-in preset `technical`/`editorial` (CSS from
   `themes/technical.theme.css`/`editorial.theme.css`, embedded by
   `build/mod.rs::embed_styles` and `runtime/src/styles.js` via
   `build-styles.mjs`) or a local `<name>.theme.css` inlined **raw** into
   `<style id="mdhtml-user">` with no CSS validation (only a plain-file-name
   check, `embed_styles` + `is_plain_file_name`).
5. **`tokens:`** — author CSS custom properties in `<style id="mdhtml-tokens">`
   via `render_tokens`/`escape_css` (`build/mod.rs`); the runtime's complete
   token set is closed (`--md-bg`, `--md-text`, `--md-muted`, `--md-surface`,
   `--md-border`, `--md-accent`, `--md-focus`, plus typography tokens,
   SPEC §13 UI-06).
6. **`fonts:`** — `auto` (preset faces from `fonts/catalog.json`, OFL/Apache
   only, `select_faces`), `system` (no bytes), or a map with `body`/`mono`
   local paths (embedded as `@font-face` data URIs) and `url` (relaxes the
   CSP, marks `data-mdhtml-portable="false"`, SPEC §5/§18).
7. **`toc:`** — `false` or `{depth, position}`; drives runtime TOC
   (`navigation.js`) and Rust fragment selection
   (`selection/manifest.rs::select_fragments`).
8. **`figures:`** — keys are embedded as assets by the Rust build
   (`build/assets.rs::collect_asset_paths`) and kept in the smart-copy
   drop list (`canonical.js::PRESENTATION_KEYS`); no runtime renderer
   consumer of `figures` exists in `runtime/src` (verified: the only
   `figures` occurrence in runtime code is `canonical.js:6`).
9. **`url:`/`cover:`** — canonical link, og:url, og:image metadata
   (`build/mod.rs::assemble`, `build/assets.rs::og_image`/`absolute_url`).
10. **Image destinations** in Markdown/HTML — author relative paths become
    `data-md-asset-path` and are hydrated only from validated embedded
    blocks (`render.js::renderInline`, `assets.js::hydrateImages`); author
    `src` values are never emitted.
11. **Heading `{#id}` overrides** — replace computed slugs and participate in
    the uniqueness pass (`analysis/slug.rs`, `render.js::headingParts`).
12. **Container arguments** `| text` — become visible `figcaption` (quote)
    or `summary` (details) HTML (`render.js::renderContainerArgument`,
    `renderContainer`).
13. **The runtime chrome itself** is fixed, not author-controlled: toolbar,
    TOC, lightbox markup come from `chrome.js`/`navigation.js`/`lightbox.js`;
    only the theme token set and preset CSS are customizable.

Summary of validation today: content text is escaped at render; section
`class` values are identifier-validated; local theme CSS, tokens CSS values,
`fonts.url`, `url`, `cover`, and link/image destinations have no
policy-level sanitization beyond escaping/string assembly.

## Source embedding and extraction

- **Embedding**: `mdhtml build` writes the canonical source bytes verbatim as
  the `textContent` of `script#mdhtml-source` (`build/mod.rs::assemble`).
  The only transformation gate is FMT-02: input containing `</script`
  (case-insensitive) fails with `E-FMT-02` and no output is written. The
  source is *not* base64-encoded and no digest of it is stored in the
  artifact; the artifact's only hashes are the runtime fragment manifest
  (`selection/manifest.rs`, `runtime/build.mjs`) and the font catalog
  (`selection/fonts.rs`).
- **Round-trip contract**: `mdhtml extract` MUST restore the source
  byte-for-byte (`SPEC.md` §15 CLI-03; PRD §3). The invariant
  `SHA256(original) == SHA256(extracted)` is verified by comparing extracted
  bytes against the original (fixture round-trip, SPEC §20); it is not a
  runtime check inside the artifact.
- **Extraction**: `extract::extract_source` (`crates/mdhtml/src/extract/
  mod.rs`) scans the artifact structurally (`scan_elements`), requires
  exactly one `script#mdhtml-source[type="text/markdown"]` (`E-FMT-01`), and
  returns its raw text bytes — `<\/script` is never decoded, newlines and
  Unicode never normalized. `extract_assets` validates each
  `application/octet-stream` block: safe relative `data-path`
  (`is_safe_relative_path` rejects empty, absolute, URL/scheme and `..`
  segments), non-empty `data-type`, distinct `data-path`s, standard padded
  base64 (embedded whitespace ignored); any violation is `E-CLI-03` and
  `commands.rs::extract` completes all validation before writing (staged
  assets, then `write_atomic`), never overwriting existing files.
- **`<\/script` decoding is runtime-only**: `runtime/src/canonical.js`
  `decodeCanonicalSource` decodes `<\/script` → `</script` for rendering and
  for `smart`/`body` copy; `full` copy and `extract` return stored bytes
  verbatim (SPEC §4 FMT-02).
- **Hand-edited artifacts**: the runtime re-derives everything from
  `#mdhtml-source` on load (`core.js::mountDocument`); missing embedded
  assets render alt-only with `W-UI-04` and `check` reports them
  (`check/mod.rs::check_artifact`); components without their fragment degrade
  to prose (SPEC §3, COMP-02).
- **Check of the stored source**: `check_artifact` runs the accepted analysis
  over the stored source (`analyze_stored_source`), re-verifies the CSP
  matches the content verdict (`E-FMT-03`), checks `data-mdhtml-portable`
  consistency, reports `W-UI-04` for referenced-but-unembedded assets, and
  reports byte budgets by category (SPEC §18).

## Existing structural decisions relevant to security

What the current ADRs and SPEC already constrain (facts, not recommendations):

- **ADR 0004 (mdhtml 1.0 format and toolchain)** — single canonical source
  per document; hash-based section navigation; a closed CSP for portable
  documents; a closed front-matter YAML subset; semantic containers and
  section components that always degrade to prose; byte-exact round-trip;
  dependency-free classic-JS runtime with no `type="module"`, `fetch`, or
  `pushState`; std-only Rust CLI that never parses Markdown; runtime and
  Rust dependencies require a superseding ADR; esbuild/Playwright are
  dev-only.
- **ADR 0005 (runtime fragment boundaries)** — exactly four manifest
  fragments (`core`, `copy`, `toc`, `lightbox`) in fixed order; one shared
  private protocol symbol `globalThis[Symbol.for("mdhtml.runtime.1")]`;
  optional fragments are no-ops without successful core evidence; selection
  consumes closed analysis evidence; `runtime.min.js` is the exact
  concatenation.
- **ADR 0001 / 0003** — decisions recorded immutably and superseded, never
  edited; Sentrux structural gates (`check` + ratcheted `gate`) are a
  mandatory quality gate with a moving-up-only baseline.
- **SPEC.md** —
  - FMT-01/§2: exactly one `#mdhtml-source`; derived data must never become
    source.
  - FMT-02/§4: `</script` forbidden; `<\/script` decode is runtime-only.
  - FMT-03/§5: closed portable CSP `default-src 'none'` with an explicit
    `data:`/`blob:` allowlist; the only declared exception is `fonts.url`,
    which relaxes only `style-src`/`font-src` and marks the document
    non-portable.
  - FMT-04/§6: `<noscript>` fallback exposes the raw source; no blank page.
  - PARSE-01/§8, PARSE-02/§9: closed front matter subset; hostile text MUST
    be escaped; raw HTML is not enabled by default and never enabled unless
    explicitly configured.
  - SECT-01/§11, COMP-01/§12: deterministic slug identity; strict container
    shape rules; out-of-convention content degrades to prose, never guessed.
  - UI-01…UI-06/§13: smart copy drops presentation keys; clipboard requires a
    synchronous user gesture; navigation is hash-only; image `src` is never
    author-controlled; lightbox uses native `<dialog>`; closed token set for
    theming.
  - §14: closed MIME table (8 extensions); asset payloads extracted
    byte-exactly.
  - CLI-01…CLI-05/§15: atomic writes (no partial output); validation before
    write; `check` portability verdict = zero external requests; `extract`
    refuses unsafe paths and silent overwrites.
  - §17: runtime fragments selected only from closed analysis evidence;
    builds reproducible byte-for-byte.
  - §18: font budget rules (variable `wght@min..max` only, no `opsz`;
    italic/mono only when used; chrome uses system stack; OFL/Apache only,
    committed with license notices, never re-subsetted; catalog records size,
    SHA-256, upstream integrity, license per face); binary size limit.
  - §20: fixtures are the executable contract; deterministic harnesses, no
    network.
- **`docs/ARCHITECTURE.md` (Security model)** — "the CSP is the perimeter";
  raw HTML from Markdown is escaped by default and only accepted under an
  explicit flag; asset extraction rejects absolute paths, `..`, collisions;
  build and extract write atomically; no authn, no secrets, no data outside
  the file.
- **Toolchain integrity today** — the only cryptographic integrity checks are
  the runtime fragment manifest and font catalog SHA-256s verified by the
  CLI (`selection/manifest.rs`, `selection/fonts.rs`, `runtime/build.mjs
  check`). There is no source digest inside the artifact and no
  runtime-integrity check performed by `check` against a canonical runtime
  hash.

## Open questions for the security Tech Spec

Factual gaps the Tech Spec must resolve (no recommendations made here):

1. **Local theme CSS is unvalidated.** `theme: <file>.theme.css` is inlined
   raw into `<style id="mdhtml-user">` (`build/mod.rs::embed_styles`), gated
   only by `is_plain_file_name`. No CSS parser exists in the CLI; no scan of
   user CSS for `@import`, `url(...)`, selectors, or exfiltration vectors
   beyond `check`'s `url(...)` origin counting (`check/mod.rs:352`).
2. **`fonts.url` flows into the CSP string** (`build/assets.rs::relaxed_csp` +
   `origin_of`) with no scheme/host/character validation; `check` mirrors it
   (`check/mod.rs::fonts_origins`). Both sides share the same string
   construction, so they agree — but the input is author-controlled.
3. **Link/image destinations have no scheme allowlist.** Markdown links
   render as `<a href="…">` with attribute escaping only
   (`render.js::renderInline`); `javascript:`-style destinations are not
   filtered by build or runtime. Image `src` is safe by construction
   (author `src` never emitted; `assets.js` assigns from validated blocks),
   but link `href` is not.
4. **Asset path handling is asymmetric.** `build` embeds relative asset paths
   without rejecting `..` segments (`build/assets.rs::is_relative_path`),
   while `extract` rejects them (`extract/mod.rs::is_safe_relative_path`).
5. **No source digest in the artifact.** The `SHA256(original) ==
   SHA256(extracted)` invariant (PRD §3) is verified by fixtures, not by any
   stored digest or runtime/`check` verification; `check_artifact` re-derives
   verdicts from the stored source but never compares it to a canonical
   source hash.
6. **CSP uses `'unsafe-inline'`** for both `script-src` and `style-src`
   (`build/mod.rs::CSP`); the PRD's illustrative target (PRD §12) anticipates
   script hashes and header-based CSP — neither exists in the artifact today,
   and the CSP is a `<meta>` only.
7. **`check`'s origin inventory is narrow.** `collect_html_origins`
   (`check/mod.rs:320`) covers `script[src]`, `img[src]`,
   `link[rel=stylesheet]`, and `url(...)` in style blocks — not forms,
   iframes, objects, embeds, prefetch/preload, media, SVG references, or
   other network-capable HTML mechanisms the PRD §10 inventories.
8. **SVG is an embeddable MIME** (`image/svg+xml` in `MIME_BY_EXTENSION`),
   and the runtime hydrates it like any image (`assets.js::IMAGE_MIMES`) —
   the interaction of embedded SVG with script/event content is
   unaddressed.
9. **`url`/`cover` metadata is unvalidated** — `absolute_url`
   (`build/assets.rs`) concatenates strings; og:image/canonical values are
   only HTML-escaped.
10. **Runtime rendering uses `innerHTML`** (`core.js::mountDocument`:
    `app.innerHTML = rendered.html`) with `rendered.html` built entirely by
    the escaping renderer — any future renderer path that interpolates
    author input without escaping would be a DOM-injection surface; the
    current renderer's escaping surface is `render.js::escapeText`/
    `escapeAttribute`.
11. **`figures:` has no runtime renderer consumer** — only asset embedding
    (Rust) and smart-copy dropping (`canonical.js::PRESENTATION_KEYS`); its
    intended rendering behavior is unspecified in the runtime.
12. **No runtime integrity check in `mdhtml check`** — the runtime is
    embedded from trusted committed fragments at build time and verified
    only at build/`check`-source time against the manifest; `check_artifact`
    reads the embedded runtime's byte length but does not hash it against a
    canonical runtime digest (PRD §11 evaluates this).
