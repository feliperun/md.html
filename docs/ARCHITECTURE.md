# Architecture

> Current-state summary. ADRs in [adr/](adr/README.md) hold the history and the
> *why*; this file reflects only **active** decisions. Update it in the same
> commit as any structural change.

## High-level flow

```
doc.md ── mdhtml build ──► doc.md.html ──► browser (file://)
  prosa + front matter       arquivo único         renderiza na DOM
  imagens por caminho        tudo embutido         "Copiar Markdown"
                             CSP fecha a rede      devolve a fonte
```

O autor escreve um `.md` comum — front matter + Markdown, imagens referenciadas
por caminho normal ao lado do arquivo. `mdhtml build` valida, embute tudo
(prosa, assets em base64, runtime, tema, fontes) e produz um `.md.html`
self-contained. No browser, o runtime lê o bloco canônico e renderiza.

## Components

| Componente | Responsabilidade |
|---|---|
| **`.md.html`** (o formato) | `<script id="mdhtml-source" type="text/markdown">` canônico + blocos de asset `application/octet-stream` + `<style>` de tokens/tema + runtime inline. Meta CSP `default-src 'none'`. |
| **Runtime JS** (`runtime/`) | Parser de Markdown, renderer, containers `:::`, componentes de seção, lightbox, copiar/ver/baixar. Script clássico inline; módulos incluídos só quando o documento usa. |
| **CLI Rust** (`crates/mdhtml/`) | `build`, `check`, `extract`, `new`. Zero deps; front matter à mão (implementação de referência da gramática); template e runtime embutidos via `include_str!`/`include_bytes!`. |
| **Skill de autoria** (`skills/mdhtml-author/`) | Ensina um agente a escrever documentos que respeitam a spec; mesma matéria-prima dos `examples/`. |

## Runtime & hosting

Sem servidor: o documento roda inteiro no browser via `file://`. Isso impõe
contratos duros — script clássico inline (sem `type="module"`), sem `fetch`,
sem `pushState` (lança em `file://`), navegação por hash. O mesmo core Rust
compila para `wasm32` quando o produto hospedado existir.

## Observability & quality

- Type checks + testes no push (ver [Getting Started](GETTING-STARTED.md)).
- Structural health gated by [Sentrux](sentrux.md).
- Errors/telemetry: não há servidor nem rede — telemetria é não-goal por design;
  o contrato de qualidade é `mdhtml check` verde no CI sobre todos os `examples/`.

## Security model

- **A CSP é o perímetro**: `default-src 'none'` + whitelist explícita de
  `data:`/`blob:` bloqueia toda e qualquer requisição de rede do documento.
- HTML cru do Markdown é escapado por padrão; só é aceito sob flag explícita.
- Sem authn, sem secrets, sem dados fora do arquivo: um `.md.html` não conhece
  o mundo externo.

## Related docs

- [Vision](VISION.md) · [Abstractions](ABSTRACTIONS.md) · [ADRs](adr/README.md) · [Sentrux](sentrux.md)
