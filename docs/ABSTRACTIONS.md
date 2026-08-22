# Abstractions

> The vocabulary of this codebase: the core types/modules and the contracts
> between them. Read this before adding a new module — reuse an abstraction
> before inventing one.

## Core layers

| Camada | Responsabilidade única |
|---|---|
| **Documento canônico** | O `textContent` de `#mdhtml-source`: front matter + Markdown. É a única fonte de verdade; todo o resto deriva dele. |
| **Front matter** | Subset estrito de YAML (escalares, mapas, listas, `\|`/`>`; sem âncoras/aliases/tags/tab/chave duplicada). Parseado à mão nos dois lados; a implementação Rust é a referência da gramática. |
| **Binding de seção** | Slug GitHub-style do heading (com `{#id}` como override) → componente/classe, declarado em `sections:`. |
| **Containers `:::`** | Blocos sem heading com nome semântico (`warning`, `bars`, `stats`…). O miolo é sempre Markdown válido. |
| **Assets** | Blocos `application/octet-stream` com `data-path`/`data-type`; < 32 KiB viram data URI, ≥ 32 KiB viram Blob preguiçoso. |
| **Diagnostic** | Registro estável `código/severidade/mensagem` (`E-`/`W-`/`I-<REQ-ID>`) compartilhado por runtime, CLI e fixtures; mensagens de erro curtas, sem stack trace. |
| **Runtime fragment manifest** | JSON commitado (`mdhtml/manifest/1.0`) com tamanho/hash/dependências de cada fragmento IIFE; a CLI seleciona e concatena só os necessários. |
| **Runtime** | Parser + renderer + módulos opt-in (lightbox, highlight, toc, componentes) montados conforme o documento usa. |
| **CLI** | `build`/`check`/`extract`/`new`/`themes` — validação, embute e extrai; nunca parseia Markdown. |

## External systems

Nenhum, por design — CSP `default-src 'none'` e zero dependências de rede fazem
parte do contrato do formato. A única exceção declarada é `fonts.url`, que
relaxa a CSP e marca o documento como não-portável. `esbuild` e `playwright`
são dependências apenas de desenvolvimento/CI; nem o CLI nem o documento as
carregam.

## Contracts & invariants

- Existe **exatamente um** `#mdhtml-source`; "Copiar Markdown" devolve o valor dele.
- A sequência `</script` é proibida no conteúdo — `build` falha alto, o runtime desescapa `<\/script`.
- Script clássico inline; `type="module"` e `fetch` são proibidos (CORS em `file://`).
- `pushState` proibido (lança em `file://`); navegação de seção é hash + `hashchange`.
- **Todo componente degrada**: conteúdo fora da convenção vira prosa, nunca erro, nunca adivinhação.
- **Round-trip**: `build` → `extract` → diff contra o `.md` original é vazio (bytes exatos).
- **Sem JS há fallback**: `<noscript>` revela o Markdown cru; nunca página em branco.
- **Fragmentos são clássicos e reprodutíveis**: sem import/export/fetch; build byte a byte idêntico.
- **Saída atômica**: build e extract nunca deixam arquivo parcial; path de asset inseguro falha antes de escrever; `extract` nunca sobrescreve arquivo existente em silêncio.
- **Orçamento de bytes**: `check` reporta conteúdo/runtime/fontes/imagens; binário do CLI ≤ 450 KiB.

## Quality & governance

- Structural limits live in `.sentrux/rules.toml`; regression baseline in `.sentrux/baseline.json`.
- Architecture decisions are recorded as [ADRs](adr/README.md).

## Adding a new module — checklist

- [ ] Does an existing abstraction already cover this? Reuse it.
- [ ] Inputs/outputs validated at the boundary.
- [ ] Unit tests close to the change.
- [ ] `sentrux gate .` shows no degradation.
- [ ] ADR if it introduces a cross-cutting pattern or external dependency.
