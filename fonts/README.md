# Built-in fonts

Committed font binaries, license texts, and `catalog.json` are immutable build
inputs for portable documents. `fonts/check.mjs` verifies them offline. Update
them only from the pinned upstream distributions recorded in the catalog; do
not locally subset or rewrite the WOFF2 files.

The `technical` preset uses Instrument Sans for body text and Geist Mono for
code. `editorial` uses Newsreader for body text and the same Geist Mono face.
Normal body text is always selected; italic body text is selected for emphasis,
and mono is selected for code. `system` selects no files.

All committed faces are upstream latin WOFF2 variable distributions. Sources,
versions, package integrity, hashes, byte sizes, licenses, and notices are
recorded in `catalog.json` and `NOTICE.md`.

Run the offline checks with:

```sh
node fonts/check.mjs
npm run check:fonts
```
