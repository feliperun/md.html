---
title: Analysis fixture
lang: en
theme: technical
toc: { depth: 2, position: inline }
sections:
  results: { component: cards }
  quarterly-results-2: { component: timeline }
  orphan: { component: hero }
  broken: { component: cards, class: "not-valid!" }
  unknown: { component: mystery }
---
# Crème brûlée / 2 {#results}

First section.

## *Quarterly* Results

Nested content with a [link](https://example.test/target) and `code *span*`.

## Quarterly Results

Collision test.

## Quarterly Results {#quarterly-results}

Duplicate explicit id test.

# Éditorial **Notes**

- One
- Two

# Référence [note][n1]

Reference text.

# Alt ![Cover](images/cover.png) text

Image alt text.

# Broken

Content.

# Unknown

Content.

[n1]: https://example.test/notes
# Code `*span*` shields emphasis

Inline projection with a code span.

# Référence [nested][n2] inside a quote

> [n2]: /url
