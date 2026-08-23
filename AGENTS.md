# AGENTS.md — md.html

> [Architecture](docs/ARCHITECTURE.md) · [Abstractions](docs/ABSTRACTIONS.md) · [Vision](docs/VISION.md) · [Getting Started](docs/GETTING-STARTED.md) · [ADRs](docs/adr/README.md) · [Sentrux](docs/sentrux.md)

Write the minimum code that runs. No fluff, no gold-plating.

- Do not preserve backward compatibility. Remove obsolete paths instead of adding
  compatibility layers, fallbacks, or migrations.
- Choose the simplest implementation that fully meets the current requirements.
  Avoid speculative abstractions, configuration, and indirection.
- Grow the system in layers. Start from the smallest version that works end to end,
  and add each new capability on top of a product that already works. Never trade a
  working product for unfinished complexity.
- Keep components modular and concerns clearly separated.
- Prefer established, well-maintained libraries when they reduce overall complexity
  or improve reliability. Do not reimplement common functionality without a clear reason.
- Lean on the dependencies already in the project before writing your own
  implementation or adding packages. Do not assume a library lacks a capability
  without checking its documentation and types.
- Make architectural decisions for the long term. Do not accept a stopgap that only
  works for now and is meant to be replaced later.
- Study how established products solve the problem before designing a solution. Adopt
  their proven patterns and conventions rather than inventing an approach from scratch.

## Repository rules

- **`AGENTS.md` is the single source of guidance.** `CLAUDE.md`, `GEMINI.md`,
  `CURSOR.md`, `AGENT.md` and `.github/copilot-instructions.md` are symlinks to it.
  Never edit a symlink; never let one drift into a real file.
- **Never commit secrets.** Tokens, credentials, and service-account JSON stay in a
  secret manager or a gitignored `.env`. The `pre-commit` hook scans the staged diff;
  do not work around it.
- **No personal or production-derived data in source**, migrations, fixtures, tests,
  or docs. Committed fixtures are synthetic. User-specific values belong in runtime
  configuration.
- **Never expose internals to users.** No stack traces, internal URLs, or env var
  names in user-facing copy.
- **Conventional Commits required.** `feat:`, `fix:`, `docs:`, `refactor:`, `test:`,
  `chore:`. One logical change per commit; one bounded scope per PR. Release tooling
  parses them — a break in a published contract ships as `feat!:` or carries a
  `BREAKING CHANGE:` footer, never as a plain `feat:`.
- **Never `--no-verify`.** If a hook blocks, fix the underlying issue.
- **Code, comments, and identifiers in English.** Surgical changes — no opportunistic
  refactors in feature PRs, no suppression comments to silence a linter.
- **Shell scripts** run under `set -euo pipefail` and are idempotent — re-running
  completes what is missing instead of duplicating or destroying.

## Workflow

- Check `docs/adr/` before any structural choice. Branch from `main`.
- **TDD for behavior changes**: red → green → refactor → commit. Bug fixes start with
  a failing regression test. Exception: pure docs, formatting, or copy changes.
- **E2E for key features**: any user-visible change to a primary workflow adds or
  updates a deterministic E2E scenario, isolated from real data and credentials.
  Unit tests do not replace it.
- **ADRs** live in `docs/adr/`, one decision per file, created in the same commit as
  the code (`/create-adr`). Never edit an active ADR — supersede it. Required for a
  new dependency that changes surface area, a storage or schema convention, a core
  abstraction, a hosting or secrets strategy, or a cross-cutting pattern. Not for
  behavior-preserving fixes, refactors, version bumps, or copy tweaks. After a
  structural change, update `docs/ARCHITECTURE.md` in the same commit — it reflects
  **active** decisions only.
- **Destructive actions** — merging, force-pushing, changing repository permissions,
  dropping schema, deleting data — require explicit human sign-off in the moment.
  An agent that hits this gate hands off to the user rather than routing around it.

## Gates

```bash
# Stack a nascer — quando runtime/ e crates/ existirem, o suíte fica:
#   runtime:  node runtime/build.mjs check  (fixtures .md → HTML esperado)
#   cli:      cargo test                     (gramática, validação, round-trip)
# Enquanto não existe, o placeholder abaixo mantém o gate verde de propósito.
echo 'TODO: define the check suite (typecheck + test)'           # types + tests
sentrux check .           # absolute limits (.sentrux/rules.toml)
sentrux gate .            # no structural regression vs .sentrux/baseline.json
```

CI mirrors this (`.github/workflows/quality.yml`). Before touching existing files run
`sentrux gate --save .` to capture the baseline; before committing run `sentrux gate .`
— degradation on a touched file means refactor, not commit. New files pass
`sentrux check .` clean. **Never silence a rule to pass** — the gate is a ratchet, and
every file you touch leaves with an equal-or-better score.

Done means: gates pass locally, CI is green, no secrets or personal data in the diff,
`README.md` updated if a public contract changed, ADR written if a structural decision
was made.

## md.html gotchas

Record every failure that cost real debugging time, with the invariant that prevents
it and a link to the ADR or code that must not be undone. Highest-value part of this
file — keep appending.

- **`file://` não perdoa nada disso.** `type="module"` e `fetch` são bloqueados por
  CORS (origin `null`) — só script clássico inline. `history.pushState` **lança
  exceção**, não falha silenciosa — navegação de seção é hash + `hashchange`.
- **Clipboard exige gesto de usuário síncrono.** `navigator.clipboard.writeText`
  deve ser chamado dentro do handler de `click`, sem `await` antes — o iOS invalida
  o gesto e rejeita com `NotAllowedError`. Fallback: textarea `position:fixed` +
  `readonly` + `execCommand('copy')`.
- **`</script` (case-insensitive) quebra o documento inteiro.** Raw text element
  não tem escape — a sequência é proibida no conteúdo; `build` falha alto e o
  runtime desescapa `<\/script`.
- **Pedir a query padrão de fonte do Google Fonts devolve só o peso 400.** O
  arquivo variable de verdade (`:wght@min..max`) custa o dobro, e eixos ópticos
  multiplicam por até 5× (Fraunces 36→118 KB). `font-optical-sizing: auto` é
  no-op sem o eixo no arquivo — nunca embutir `opsz`.
- **Fontshare/ITF não pode ser embutida.** A licença proíbe redistribuir os
  arquivos, e data URI é redistribuição. Só OFL/Apache; o aviso de copyright
  viaja junto no HTML gerado.
- **Mudou configuração, rode `build`; mudou só prosa, edite o bloco.** Edição
  manual do `#mdhtml-source` funciona, mas componente novo ou imagem nova
  exigem reembute — `mdhtml check` aponta as duas situações.
- **Convenção de componente é contrato com o autor, não código defensivo.**
  Conteúdo fora da convenção degrada para prosa; nunca "adivinhar" a intenção
  do autor no renderer.

---

Adapted from [Marcos Hernanz](https://x.com/MarcosHernanz/status/2083954734487212511).
Structural gate by [Sentrux](https://github.com/sentrux/sentrux).
`CLAUDE.md`, `GEMINI.md`, `CURSOR.md`, `AGENT.md` and `.github/copilot-instructions.md`
are symlinks to this file — edit `AGENTS.md` only.

<!-- plan-runner-active:start (managed by plan-runner — read, never edit) -->
Before starting new work here, check `.runs/`: if a campaign is active or a run is not terminal, continue it instead of starting over — read its `HANDOFF.md`/`STATUS.md`, attach to the campaign, and `resume` or `supervise` the run. Active runs are supervised by a deterministic detached process: do not poll `status` in a loop — on resume, check status once and act only on terminal states.

- plan-runner campaign `mdhtml-safety-hosting`: active — read `.runs/campaigns/mdhtml-safety-hosting/HANDOFF.md`
- plan-runner campaign `mdhtml-v1`: active — read `.runs/campaigns/mdhtml-v1/HANDOFF.md`
<!-- plan-runner-active:end -->
