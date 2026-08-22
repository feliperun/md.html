---
title: Extract round-trip
summary: A synthetic fixture proving byte-exact source and asset round-trips with Unicode and an escaped terminator.
fonts:
  body: asset-tiny.css
---

# Round-trip

Unicode survives: Olá — 日本語 — 🎉.

The escaped terminator stays verbatim: <\/script is not decoded.

![tiny svg](asset-tiny.svg)

An HTML image too: <img src="asset-tiny.svg" alt="tiny svg">

Trailing `code` line.
