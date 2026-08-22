# mdhtml

One portable Markdown document format. Write Markdown, ship a single
self-contained `.md.html` that opens in the browser from disk — no server, no
network, no build step for the reader.

The canonical Markdown source lives inside the file. Readers get a rendered
document with TOC navigation, themes, copy, and download; the source survives
byte for byte, so extracting it returns exactly what the author wrote.

## Quick start

```bash
npx mdhtml new intro --template memo      # scaffold from a canonical template
npx mdhtml build intro.md                 # produce intro.md.html (self-contained)
npx mdhtml check intro.md                 # portability verdict, no diagnostics
npx mdhtml extract intro.md.html -o intro-back.md   # recover the source
```

The first `npx mdhtml` run downloads the release binary for your platform,
verifies its SHA-256 checksum, and installs it into a per-user bin directory;
later runs reuse the installed binary without re-downloading. Add that
directory to your `PATH` to use `mdhtml` directly.

Open the built `intro.md.html` in any browser via `file://` — rendering,
copying, and downloading all work offline.

## Install from source

```bash
npm ci
cargo build --release -p mdhtml
./target/release/mdhtml --version
```

## Documentation

- [SPEC.md](SPEC.md) — the normative format and toolchain specification
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — current-state architecture
- [docs/GETTING-STARTED.md](docs/GETTING-STARTED.md) — setup, daily commands, and contribution flow
- [skills/mdhtml-author/SKILL.md](skills/mdhtml-author/SKILL.md) — authoring guidance
- [AGENTS.md](AGENTS.md) — the contributor/agent playbook

## License

[MIT](LICENSE)
