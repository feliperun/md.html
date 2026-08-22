# Containers (`:::`)

Containers are Pandoc-style fenced divs. An opener line has at most three
leading spaces, at least three consecutive colons, and one ASCII-lowercase
semantic name (`[a-z][a-z0-9-]*`), with or without `{.name}` braces, followed
by an optional `| argument`. A line containing only colons closes the
innermost open container; fence length need not match. The body is ordinary
Markdown, including nested containers. An opener with no matching close is
ordinary paragraph text.

The known container names are exactly:

`note`, `warning`, `critical`, `success`, `decision`, `quote`, `stats`,
`bars`, `kv`, `steps`, `grid`, `columns`, `details`

Names are semantic tokens, not CSS classes; generated markup uses the `md-`
namespace.

## Argument rules

- `quote` and `details` may take an opener argument: the attribution for
  `quote`, the summary for `details` (default `Details`).
- The other eleven containers take no argument. An argument on them is a
  failed unit.

## Strict body shapes

"Nonempty" means at least one parsed block. A two-column table is one GFM
table whose header and every body row have exactly two cells, with at least
one body row; it must be the only top-level block in the container.

| Container | Required body | Rendered as |
| --- | --- | --- |
| `note`, `warning`, `critical`, `success`, `decision` | any nonempty blocks | callout aside with a visible title-cased badge |
| `quote` | any nonempty blocks | blockquote figure; optional figcaption from the argument |
| `columns` | at least two top-level blocks | one column per block |
| `details` | any nonempty blocks | native details element; summary from the argument or `Details` |
| `stats` | exactly one two-column table with at least one body row | wrapped stat table |
| `bars` | exactly one two-column table; every second body cell a finite number ≥ 0 | table with one meter per second body cell, max the greatest value or 1 when all are zero |
| `kv` | exactly one two-column table with at least one body row, or exactly one nonempty unordered list whose every item starts with a strong key followed by `:` | definition list |
| `steps` | exactly one nonempty ordered list | numbered step list |
| `grid` | one or more level-3 heading groups and no leading top-level content | one card per `###` group, ending before the next `###` |

## Degradation (COMP-02)

A valid container applies its transformation. A failed unit keeps its parsed
children rendered as ordinary Markdown, omits the wrapper, never fails
`build`, and produces exactly one ordered `W-COMP-02` record carrying the
container name and a null target. Unknown names, invalid arguments, and
malformed bodies (including any block alongside a required table) are failed
units; a valid outer container still renders its wrapper when only a nested
unit fails.
