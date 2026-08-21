# Getting Started

## Prerequisites

- [Sentrux CLI](sentrux.md#install) for the structural quality gate.
- Stack a nascer (nada instalável ainda): Node ≥ 22 para buildar o runtime
  (`runtime/build.mjs`) e Rust stable para a CLI (`crates/mdhtml/`).
- Sem secrets: o produto não conhece rede por design.

## Quick start

```bash
# TODO: fica real quando a CLI existir —
# hoje o formato vive na spec (SPEC.md) e no plano (docs/plan_preview.md)
```

## Daily commands

```bash
echo 'TODO: define the check suite (typecheck + test)'            # types + tests
sentrux check .           # architectural rules
sentrux gate .            # no structural regression
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

## First contribution checklist

- [ ] Read [AGENTS.md](../AGENTS.md).
- [ ] Run the check suite locally and confirm it's green.
- [ ] `sentrux gate --save .` before touching existing files.
