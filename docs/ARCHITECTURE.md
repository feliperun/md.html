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
                             ^                          |
                             +---- mdhtml extract ------+
```

O autor escreve um `.md` comum — front matter + Markdown, imagens referenciadas
por caminho normal ao lado do arquivo. `mdhtml build` valida, embute tudo
(prosa, assets em base64, runtime, tema, fontes) e produz um `.md.html`
self-contained. No browser, o runtime lê o bloco canônico e renderiza; `mdhtml
extract` devolve o Markdown byte a byte (round-trip vazio).

## Components

| Componente | Responsabilidade |
|---|---|
| **`.md.html`** (o formato, `mdhtml/1.0`) | `<script id="mdhtml-source" type="text/markdown">` canônico + blocos de asset `application/octet-stream` + `<style>` de tokens/tema + runtime inline. Meta CSP `default-src 'none'`; `data-mdhtml="1.0"` e `data-mdhtml-portable` no root. |
| **Runtime JS** (`runtime/`) | Parser de Markdown, renderer, containers `:::`, componentes de seção, hidratação de imagens, estilos e superfícies de copiar, TOC e lightbox. `bootstrap.js` é a composição ESM de teste/desenvolvimento; os artefatos do browser são quatro IIFEs clássicos. |
| **Runtime fragment manifest** | JSON commitado (`mdhtml/manifest/1.0`) com tamanho/hash/dependências de `core`, `copy`, `toc` e `lightbox`, nessa ordem fixa; a CLI seleciona por evidência fechada e concatena os buffers; `runtime.min.js` é exatamente a concatenação completa. |
| **Diagnostic** | Registro estável código/severidade/mensagem compartilhado por runtime, CLI e fixtures (`E-`/`W-`/`I-<REQ-ID>`); erros curtos e acionáveis, sem stack trace. |
| **CLI Rust** (`crates/mdhtml/`) | `build`, `check`, `extract`, `new`, `themes` + `--watch`/`--no-fonts`. Std only; front matter à mão (implementação de referência da gramática); template/runtime embutidos via `include_str!`/`include_bytes!`; escrita atômica. |
| **Skill de autoria** (`skills/mdhtml-author/`) | Ensina um agente a escrever documentos que respeitam a spec; mesma matéria-prima dos `examples/`. |

## Runtime & hosting

Sem servidor: o documento roda inteiro no browser via `file://`. Isso impõe
contratos duros — script clássico inline (sem `type="module"`), sem `fetch`,
sem `pushState` (lança em `file://`), navegação por hash. O runtime é JS
clássico sem dependências, gerado com `esbuild` (dev-only) e commitado. `core`
escreve uma única evidência em `globalThis[Symbol.for("mdhtml.runtime.1")]`;
os fragmentos opcionais só montam depois de um core bem-sucedido. O core Rust
compila para `wasm32` quando o produto hospedado existir. `playwright` também
é dev-only: E2E determinístico sobre `file://` em CI.

## Observability & quality

- Type checks + testes no push (ver [Getting Started](GETTING-STARTED.md)).
- Structural health gated by [Sentrux](sentrux.md).
- Sem servidor nem rede — telemetria é não-goal por design; o contrato de
  qualidade é `mdhtml check` verde no CI sobre todos os `examples/`, com
  orçamento de bytes por categoria (conteúdo/runtime/fontes/imagens) e limite
  de binário (450 KiB) verificado no release.

## Security model

- **A CSP é o perímetro**: `default-src 'none'` + whitelist explícita de
  `data:`/`blob:` bloqueia toda e qualquer requisição de rede do documento.
  Documentos com `fonts.url` relaxam a CSP só para as origens declaradas e são
  marcados `data-mdhtml-portable="false"`.
- HTML cru do Markdown é escapado por padrão; só é aceito sob flag explícita.
- Extração de assets rejeita path absoluto, `..` e colisões; build e extract
  escrevem atomicamente (nunca saída parcial).
- Sem authn, sem secrets, sem dados fora do arquivo: um `.md.html` não conhece
  o mundo externo.

## Related docs

- [Vision](VISION.md) · [Abstractions](ABSTRACTIONS.md) · [ADRs](adr/README.md) · [Sentrux](sentrux.md) · [Spec](../SPEC.md)
