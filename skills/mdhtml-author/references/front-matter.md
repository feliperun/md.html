# Front matter

Front matter is optional and sits at the very start of the source: a line with
exactly `---`, the content, then a closing line with exactly `---`.

## Accepted YAML subset

The subset is closed: scalars (plain, single-quoted, double-quoted, finite
integers and floats, booleans, null), maps by indentation (spaces only),
block and flow sequences, flow maps, literal `|` and folded `>` blocks, and
comments are accepted. Maps and sequences nest. Rejected as `E-PARSE-01`:
tabs used for indentation, anchors and aliases, tags, duplicate keys,
non-finite numeric syntax, inconsistent indentation, and multiple documents.

## Reserved keys

| Key | Type | Default | Purpose |
| --- | --- | --- | --- |
| `title` | string | — (required) | document title; absence is `E-FMT-05` |
| `summary` | string | — | description and Open Graph |
| `lang` | string | `en` | document language |
| `theme` | string or path | `technical` | built-in preset (`technical`, `editorial`) or a local `.theme.css` |
| `tokens` | map | `{}` | per-document token overrides |
| `fonts` | `auto`, `system`, or map | `auto` | font embedding policy; map shape `{body, mono, url}` |
| `url` | string | — | canonical URL and `og:url` |
| `cover` | path | — | `og:image`; requires `url` |
| `toc` | `false` or map | `{depth: 3, position: side}` | map accepts integer `depth` 1–6 and `position` `side` or `inline`; `false` disables the table of contents |
| `sections` | map | `{}` | heading slug to `{component, class}` binding |
| `figures` | map | `{}` | asset path to `{align, size, caption, group, shape}` |
| `date`, `authors`, `tags` | scalar or sequence | — | semantic metadata |

## Copy behavior

Unknown keys are preserved by smart copy — they are semantic. The
presentation keys (`theme`, `tokens`, `fonts`, `sections`, `figures`, `toc`)
are dropped from smart copy. Write only what the document needs; the
templates show the accepted minimal forms (templates/resume.md,
templates/memo.md, templates/spec.md, templates/recipe.md,
templates/chapter.md).
