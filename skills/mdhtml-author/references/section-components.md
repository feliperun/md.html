# Section components

Section components bind to an existing heading slug through the `sections:`
front matter map: `slug: {component: name, class?: text}`. `class`, when
present, is one or more whitespace-separated CSS identifiers
(`[A-Za-z_][A-Za-z0-9_-]*`) appended to the section wrapper only. A binding
whose slug has no heading is `E-SECT-01`; an invalid binding, an unknown
component, or a shape mismatch leaves the section unchanged and reports one
`W-COMP-02` record with the component name and the slug as target.

The known components are exactly:

`timeline`, `cards`, `meters`, `gallery`, `kv`, `columns`, `hero`

## Strict body shapes

"Nonempty" means at least one parsed block. A two-column table follows the
container rule: one GFM table whose header and every body row have exactly
two cells, with at least one body row.

| Component | Required section body | Rendered as |
| --- | --- | --- |
| `timeline` | exactly one nonempty ordered or unordered list | timeline wrapper |
| `cards` | one or more immediate child section elements and no other body blocks | card grid wrapper |
| `meters` | exactly one two-column table; every second body cell a finite number 0–100 | table with one `min="0" max="100"` meter per second body cell |
| `gallery` | one or more paragraphs, each containing exactly one image | one figure per paragraph |
| `kv` | the same table or strong-key list shape as the `kv` container | definition list |
| `columns` | at least two top-level body blocks | one column per block |
| `hero` | nonempty body with at most one standalone-image paragraph | content wrapper plus a media wrapper for the image |

A standalone-image paragraph is a paragraph containing exactly one image and
no other inline content; a hero without one keeps an empty media wrapper.
Component wrappers render inside the bound section element, so the section
identity and its class stay stable. Slugs follow the heading rules: computed
from the heading text, overridable with `{#id}`, and collision-suffixed
`-2`, `-3`, … — a duplicate explicit id is `W-SECT-01`.
