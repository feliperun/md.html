# Getting Started

## Prerequisites

- Node.js ≥ 22 for the runtime build harness and tests.
- Rust stable toolchain for the CLI (`crates/mdhtml/`).
- [Sentrux CLI](sentrux.md#install) for the structural quality gate.
- No secrets: the product never talks to the network by design.

## Install the CLI

The quickest path is `npx mdhtml`, which downloads the release binary for your
platform, verifies its SHA-256 checksum, installs it into a per-user bin
directory, and runs it. Re-running skips the download when the installed
binary is already correct.

```bash
npx mdhtml --version
```

Add the per-user bin directory to your `PATH` to use `mdhtml` directly. To
build from source instead:

```bash
npm ci
cargo build --release -p mdhtml
./target/release/mdhtml --version
```

## Quick start

```bash
mdhtml new intro --template memo       # scaffold from a canonical template
mdhtml build intro.md                  # produce intro.md.html (self-contained)
mdhtml check intro.md                  # portability verdict, no E/W diagnostics
mdhtml extract intro.md.html -o intro-back.md   # recover the source byte for byte
```

Open `intro.md.html` in a browser via `file://` — no server, no network.
Before shipping a release, run the manual [smoke test](SMOKE-TEST.md).

## Daily commands

```bash
npm test                        # runtime unit tests
node runtime/build.mjs check    # committed runtime artifacts match the build
cargo test --locked             # CLI tests (grammar, validation, round-trip)
./scripts/check-examples.sh     # five canonical examples build/check/round-trip
sentrux check .                 # architectural rules
sentrux gate .                  # no structural regression
```

## Worktree workflow

```bash
# Create a worktree for a task (keeps main clean):
git worktree add ../md.html-<task> -b <dev>/<issue>-<slug>
```

## Documentation map

- [Vision](VISION.md) — why this exists
- [Architecture](ARCHITECTURE.md) — current-state structure
- [Abstractions](ABSTRACTIONS.md) — the vocabulary
- [ADRs](adr/README.md) — decision history
- [Sentrux](sentrux.md) — the quality gate
- [AGENTS.md](../AGENTS.md) — the contributor/agent playbook
- [Spec](../SPEC.md) — the normative format specification
- [Smoke test](SMOKE-TEST.md) — manual release checklist

## First contribution checklist

- [ ] Read [AGENTS.md](../AGENTS.md).
- [ ] Run the check suite locally and confirm it's green.
- [ ] `sentrux gate --save .` before touching existing files.
