---
name: mdhtml-author
description: Author mdhtml 1.0 documents: scaffold with `mdhtml new` from the canonical templates, write the accepted front matter subset and Markdown conventions, shape content with `:::` containers and section components, extract or hand-edit built documents, close with `mdhtml build` then `mdhtml check`, audit with `mdhtml audit`, and publish the result with `mdhtml publish`. Use when creating, editing, reviewing, or publishing a Markdown document that must build into a portable self-contained `.md.html` file.
---

# mdhtml authoring

mdhtml 1.0 turns a Markdown file (optional front matter plus a Markdown body)
into a single portable HTML document that renders in the browser and works
from disk. The committed templates and examples are the reference material for
every convention in this skill: read them, and never re-derive or copy the
format.

## Workflow

1. **Scaffold from a canonical template.** Run `mdhtml new <name> --template
   <kind>` with `<kind>` one of `resume`, `memo`, `spec`, `recipe`, `chapter`.
   The committed templates are the same material and the starting point when
   the CLI is unavailable:

   - templates/resume.md
   - templates/memo.md
   - templates/spec.md
   - templates/recipe.md
   - templates/chapter.md

   Their built counterparts show the conventions applied end to end:

   - examples/resume.md
   - examples/memo.md
   - examples/spec.md
   - examples/recipe.md
   - examples/chapter.md

2. **Write the front matter.** Only the reserved keys in
   references/front-matter.md are presentation input; everything else is
   semantic metadata. Keep it minimal: `title` is required, `summary` and
   `lang` are common, and `theme`, `toc`, `sections`, and `figures` appear
   only when the document needs them.

3. **Write the body in the supported Markdown subset.** Headings, paragraphs,
   links, images, blockquotes, GFM tables, code spans and fences, nested and
   ordered lists, task lists, emphasis, strong, strike, and footnotes. Raw
   HTML is not enabled; keep content in Markdown. Every heading receives a
   deterministic slug, and an explicit `{#id}` override replaces it.

4. **Shape content with containers and section components.** Containers are
   Pandoc-style `:::` fences; section components bind to a heading slug
   through the `sections:` front matter map. Use only the shapes in
   references/containers.md and references/section-components.md. Content
   that misses a shape is not guessed: it degrades to prose and `mdhtml
   check` reports `W-COMP-02`.

5. **Extract and hand-edit a built document.** The `#mdhtml-source` block
   inside a built `.md.html` is the only source of truth: rendering, copy,
   and extraction all derive from it. Two working modes:

   - **Author in Markdown.** `mdhtml extract doc.md.html -o doc.md` restores
     the canonical source byte for byte — `build` → `extract` is an empty
     round-trip — and `--assets <dir>` recovers the embedded assets with
     their original paths. Extraction never overwrites an existing file.
   - **Edit the built file in place.** Pure prose changes inside the
     `#mdhtml-source` block take effect on reload, with no toolchain. A
     hand-edited file may store the forbidden sequence as `<\/script`; the
     runtime decodes it for rendering while `extract` returns the stored
     bytes.

   Anything beyond prose needs a rebuild: a newly referenced asset is not
   embedded (its alt text renders; `check` warns `W-UI-04`), a new container
   or section component lacks its runtime fragment (it degrades to prose;
   `check` warns `W-COMP-02`), and presentation or configuration changes do
   not regenerate the embedded style.

6. **Close with build then check.** Always run, in order:

   ```bash
   mdhtml build doc.md
   mdhtml check doc.md
   ```

   Fix every `E-` diagnostic; a completed document reports no errors and no
   warnings. After a hand edit, close with `mdhtml check doc.md.html`, which
   reports the portability verdict and the byte budget; when the edit needs
   re-embedding, apply it to the extracted source and rebuild.

7. **Audit, then publish.** When the document is meant to be shared, run:

   ```bash
   mdhtml audit doc.md.html
   mdhtml publish doc.md
   ```

   `mdhtml publish` accepts `--url <base-url>`, or the `MDHTML_PUBLISH_URL`
   environment variable, to target a local or mock endpoint. Fix every
   diagnostic before publishing and return the resulting public URL to the
   user. See references/publishing.md for the full loop and error mapping.

## Rules

- Reference the committed templates and examples by path; never inline their
  content into a document or into this skill's files.
- Every convention must match the normative format specification (SPEC.md,
  §12). Content outside a convention becomes prose — never an invented
  rendering.
- No personal or production-derived data: the templates and examples are
  synthetic, and so must be anything derived from them.

## Security rules

- Never bypass a security diagnostic to complete publication.
- Fix the source while preserving author intent.
- Never invoke `--unsafe` unless the human explicitly requested it.
- Never encode or obfuscate prohibited constructs.
- Treat security errors as constraints, not obstacles to circumvent.
- Prefer embedded/local assets when external resources violate policy.

Publishing is safe-by-default: a document that fails `mdhtml audit` is never
published, and `--unsafe` artifacts cannot be published at all.
