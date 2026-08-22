# Release smoke test

Manual checklist a human runs before shipping a release. Everything happens
locally against the built binary and a fresh synthetic document — no real
data, no network.

## 1. Build the release CLI

```bash
cargo build --release --locked -p mdhtml
./target/release/mdhtml --version
```

- [ ] The version printed matches the release being shipped.

## 2. Fresh document lifecycle

```bash
WORK="$(mktemp -d)"
cd "$WORK"
mdhtml new smoke --template resume
mdhtml build smoke.md
mdhtml check smoke.md
mdhtml check smoke.md.html
mdhtml extract smoke.md.html -o smoke-roundtrip.md
cmp smoke.md smoke-roundtrip.md
```

- [ ] `mdhtml new` scaffolds the document.
- [ ] `mdhtml build` writes `smoke.md.html`.
- [ ] `mdhtml check` reports no `E-`/`W-` diagnostics and a portable verdict
      on both the source and the built artifact.
- [ ] `extract` followed by `cmp` is an empty round trip (byte-identical).

## 3. Browser rendering via file://

Open `smoke.md.html` in a real browser from disk (double-click the file or
navigate to it via `file://`) with the network disabled.

- [ ] The document renders: headings, paragraphs, lists, the table, and links.
- [ ] TOC navigation: clicking TOC entries scrolls to the matching section,
      and the browser back/forward buttons move through visited sections.
- [ ] Theme cycling: the toolbar theme control switches the visual presets.
- [ ] Clipboard copy: copying the Markdown (smart/full/body modes) yields the
      canonical source when pasted.
- [ ] Download: the downloaded `.md` file equals the canonical source.
- [ ] No network activity: the page works with the network disabled.

## 4. No-JavaScript fallback

Reload `smoke.md.html` with JavaScript disabled.

- [ ] The fallback shows the canonical Markdown source as readable text.
