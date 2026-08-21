# mdhtml — formato `.md.html` + runtime + CLI

## Context

Markdown é o formato em que trabalhamos e em que os agentes trabalham melhor. Mas compartilhar Markdown cru com colegas é ruim — ninguém quer ler `##` e `**` numa tela. As saídas atuais são todas ruins: mandar `.md` (feio), gerar PDF (o agente do outro lado precisa reparsear tudo), ou exportar HTML (perde o Markdown, e aí não dá pra jogar de volta num agente).

O formato `.md.html` resolve os dois lados de uma vez: **um arquivo único onde o Markdown é a fonte da verdade e o HTML só existe para renderizá-lo bonito.** Manda pro amigo, ele abre no browser e lê algo elegante; clica em "Copiar Markdown" e cola no agente dele, sem Ctrl+A nem PDF no meio.

Já existe um protótipo funcionando que prova a tese (`~/dev/micromed/coreum/specs/dialogo/analise-felipe-broering.html`): script `type="text/markdown"`, notação `:::` para componentes, copiar-markdown, tokens de DS. O que falta é virar formato: front matter, imagens, portabilidade real, spec escrita e ferramenta.

O offline vem primeiro. Hosting na nuvem é o passo seguinte — e a escolha de Rust existe em parte para que o mesmo core vire WASM quando essa hora chegar.

## Decisões travadas

| Decisão | Escolha |
|---|---|
| Renderização | **Runtime** — Markdown + parser inline; sem passo de build para editar prosa |
| Binding de seção | **Front matter** (`sections:`) + `data-md-section` sempre presente para CSS livre |
| Blocos sem heading | **`:::`** (fenced divs, padrão Pandoc), com nomes semânticos |
| CLI | **Rust**, zero deps, ~300–450 KB, mesmo core compila pra WASM depois |
| Tema | **Base única + presets por token** (`editorial` ↔ `technical`) |
| Preview social | Meta OG textual sempre; `og:image` só quando front matter declara `url:` |
| Fontes | **Instrument Sans** (technical) / **Newsreader** (editorial) / **Geist Mono** — OFL, variable, subset latin, embutidas; `system` e online como opt-in |

## O formato

### Anatomia do arquivo

```html
<!doctype html>
<html lang="pt-BR" data-mdhtml="1.0" data-mdhtml-portable="true">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none';
        img-src data: blob:; style-src 'unsafe-inline'; script-src 'unsafe-inline';
        font-src data:; media-src data: blob:">
  <title>…</title>                        <!-- do front matter -->
  <meta property="og:title" …>            <!-- do front matter -->
  <style id="mdhtml-tokens">…</style>     <!-- design tokens, editável -->
  <style id="mdhtml-theme">…</style>      <!-- base + preset -->
  <style id="mdhtml-user">…</style>       <!-- CSS do autor, opcional -->
</head>
<body>
  <div id="mdhtml-app"></div>
  <noscript><style>
    #mdhtml-source{display:block;white-space:pre-wrap;font-family:ui-monospace;padding:2rem}
  </style></noscript>
  <script id="mdhtml-source" type="text/markdown">…front matter + markdown…</script>
  <script type="application/octet-stream" data-path="images/foto.jpg"
          data-type="image/jpeg">BASE64…</script>
  <script id="mdhtml-runtime">…</script>
</body>
</html>
```

**Invariantes da spec:**

- Existe **exatamente um** `#mdhtml-source`, e seu `textContent` é o documento canônico. "Copiar Markdown" é literalmente esse valor.
- Única regra de escape: a sequência `</script` (case-insensitive) não pode aparecer no conteúdo. A CLI falha alto; o runtime desescapa `<\/script`.
- Script clássico inline obrigatório — `type="module"` é bloqueado por CORS em `file://`.
- Sem `pushState` — lança exceção em `file://`. Navegação de seção usa hash + `hashchange`.
- Sem JS, o `<noscript><style>` revela o Markdown cru legível. Nunca uma página em branco (o erro do Markdeep é `visibility:hidden`).

### Front matter

Subset estrito de YAML, parseado à mão nos dois lados (~200 linhas em Rust, ~80 em JS). Suporta escalares, mapas aninhados por indentação, listas em bloco e em flow, `|`/`>`, comentários. **Não** suporta âncoras, aliases, tags nem multi-documento.

```yaml
---
title: Currículo — Felipe Broering        # obrigatório
summary: Head de Engenharia…              # → description + og:description
lang: pt-BR
theme: technical                          # editorial | technical
tokens:
  accent: "#FF3C3C"
  measure: 72ch
fonts: auto                               # auto | system | { body:, mono:, url: }
url: https://frb.dev/cv                   # habilita canonical + og:image
cover: images/foto.jpg
toc: { depth: 2, position: side }
sections:
  experiencia: { component: timeline, class: wide }
  idiomas:     { component: meters }
figures:
  images/foto.jpg: { align: right, size: sm, shape: circle }
---
```

### Binding de seção

Slug do heading no padrão GitHub (lowercase, NFD, remove diacríticos, `\s+`→`-`), com colisão resolvida por sufixo `-2`. Override explícito via `## Título {#slug}` estilo Pandoc.

Toda seção recebe `data-md-section="slug"` **independente de configuração** — então CSS puro funciona sem tocar no front matter. O `component:` existe para o que CSS não alcança: reestruturar a DOM.

`mdhtml check` **falha** se um slug em `sections:` não existir no documento. Isso mata o modo de falha óbvio (renomeou o heading, o binding morreu em silêncio).

### Containers `:::`

**Regra fundadora, e a principal correção sobre o protótipo: `:::` escolhe apresentação de conteúdo que continua sendo Markdown válido. Nunca inventa sintaxe de dados.**

```markdown
::: warning | Atenção
O serviço roda em **Node 18**, morto desde abril de 2025.
:::

::: bars
| Queixa            | Menções |
|-------------------|---------|
| Tempo de laudo    | 19      |
| Acesso / internet | 4       |
:::
```

No GitHub: um aviso legível e uma tabela de verdade. No mdhtml: callout e barras. O protótipo usa `Label | 19` cru, que vira mingau em qualquer outro renderizador — é a degradação que perdemos se mantivermos aquilo.

Nomes são semânticos, nunca classes CSS (`::: warning`, não `::: cbox warn`). Aceita também a forma Pandoc `::: {.warning}`. Containers aninham.

| Container | Conteúdo esperado | Render |
|---|---|---|
| `note` `warning` `critical` `success` `decision` | prosa ou lista | callout com badge |
| `quote` | prosa | citação com atribuição via `\| fonte` |
| `stats` | tabela 2 col | grid de números grandes |
| `bars` | tabela 2 col | barras proporcionais |
| `kv` | tabela 2 col ou `- **k**: v` | definition list |
| `steps` | lista ordenada | passos numerados |
| `grid` | headings `###` | cards |
| `columns` / `details` | qualquer | multi-coluna / colapsável |

Componentes de **seção** (front matter): `timeline`, `cards`, `meters`, `gallery`, `kv`, `columns`, `hero`.

**Todo componente degrada.** Conteúdo fora da convenção renderiza como prosa normal e `mdhtml check` emite aviso — nunca quebra.

### Imagens

O Markdown usa caminhos normais (`![Foto](images/team.jpg)`), então o `.md` extraído funciona ao lado dos arquivos reais. A CLI resolve para blocos embutidos.

- `< 32 KB` → data URI direto.
- `≥ 32 KB` → `atob` → `Uint8Array` → `Blob` → `createObjectURL`, preguiçoso via `IntersectionObserver`. Evita memória dobrada e decode na main thread.
- `figures:` controla `align`, `size`, `caption` (default: o alt), `group`, `shape`.
- Lightbox: `<dialog>` + `showModal()` — top layer, `::backdrop`, `inert` e devolução de foco vêm de graça. **Sem focus trap manual.** Setas, swipe, Esc, contador, `prefers-reduced-motion`.

### Copiar como Markdown

Três modos, default `smart`:

| Modo | Entrega |
|---|---|
| `smart` (default) | Front matter **semântico** (`title`, `summary`, `date`, `authors`, `tags`) + prosa. Descarta chaves de apresentação (`theme`, `tokens`, `fonts`, `sections`, `figures`, `toc`) |
| `full` | O `textContent` cru do `#mdhtml-source`, byte a byte |
| `body` | Só a prosa, sem front matter |

O default existe porque o destinatário mais provável é um agente: metadado semântico ajuda, `--md-measure: 72ch` é ruído. Containers `:::` sobrevivem em todos os modos — são semânticos e são Pandoc válido.

`navigator.clipboard.writeText` dentro do handler de click (síncrono — iOS invalida o gesto se houver `await` antes), com fallback `execCommand` em textarea `position:fixed` + `readonly`.

### Portabilidade verificável

*Self-contained* = zero requisições de rede, garantido em três camadas: a CSP bloqueia no browser, `data-mdhtml-portable` declara, e `mdhtml check` verifica e reporta o orçamento de bytes por categoria. Fonte online é permitida, mas relaxa a CSP e marca o documento como não-portável.

```
$ mdhtml check cv.md.html
✓ self-contained — 0 requisições externas
  187 KB total · fontes 74 · imagens 61 · runtime 38 · conteúdo 14
```

### Tema e tokens

Uma folha de estilo. `theme:` só troca valores de token.

```css
--md-font-body     Georgia, serif  ↔  Inter, sans-serif
--md-measure       68ch            ↔  78ch
--md-density       1.0             ↔  0.82
--md-heading-scale 1.25            ↔  1.18
```

Paleta completa em `:root` (light), redefinida em `@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) }` e em `:root[data-theme="dark"]`, com toggle. Nenhuma cor definida só dentro de media query.

**Temas são arquivos, não só nomes.** Um tema é um `:root { --md-* }` mais, opcionalmente, referências de fonte — nada além de CSS:

```yaml
theme: technical              # preset embutido
theme: ./micromed.theme.css   # arquivo local, a CLI inlina
```

É o que dá o "mini design system reusável": a paleta Herz que já existe no protótipo (`--primary`, tints, shades, raios, sombras, focus ring) vira um `micromed.theme.css` compartilhado entre todos os documentos da empresa, versionado como qualquer outro arquivo. `mdhtml themes` lista os embutidos; `tokens:` no front matter sobrescreve pontualmente por documento.

### Edição à mão de um arquivo já construído

Renderização em runtime significa que editar prosa dentro do `#mdhtml-source` e dar F5 funciona — sem toolchain. Duas coisas, porém, não sobrevivem à edição manual: referenciar uma imagem que não foi embutida, e usar um componente cujo módulo não foi incluído no bundle.

A spec resolve por degradação, não por erro: componente desconhecido renderiza como prosa (é a mesma regra de degradação de todo componente), imagem ausente mostra o alt. O documento nunca quebra, e `mdhtml check` sobre o `.md.html` aponta as duas situações. Mudou conteúdo? edite à vontade. Mudou configuração ou adicionou imagem? rode `build`.

## Runtime

JS clássico, sem dependências, buildado com esbuild e commitado como `runtime.min.js` para que `cargo build` rode sozinho.

Módulos incluídos **só quando o documento usa**:

| Módulo | Quando | ~gz |
|---|---|---|
| `core` (parser + render + tokens) | sempre | 14 KB |
| `copy` | sempre | 1 KB |
| `lightbox` | se houver imagem | 3 KB |
| `highlight` (Prism core + linguagens usadas) | se houver code fence com linguagem | 2 KB + 0,5/ling |
| `toc` | se `toc:` ativo | 1 KB |
| `components/*` | só os usados | 0,5 KB cada |

O parser precisa cobrir o que o protótipo não cobre: **fenced code**, listas aninhadas, task lists, `~~strike~~`, `_ênfase_`, hard breaks, footnotes, e a ordem correta de inline (code spans imunes a bold/link — hoje `` `a**b**c` `` vira `<code>a<strong>b</strong>c</code>`).

Chrome do documento: barra com **Copiar Markdown**, **Ver como Markdown**, **Baixar .md**, toggle de tema. Some no `@media print`.

## CLI Rust

```
mdhtml build <in.md> [-o out] [--watch] [--no-fonts]
mdhtml new <nome> [--template resume|memo|spec|recipe|chapter]
mdhtml extract <in.md.html> [-o out.md] [--assets ./dir]
mdhtml check <arquivo>
mdhtml themes
```

Zero dependências: arg parsing, front matter e base64 escritos à mão — o parser de front matter vira a implementação de referência da gramática da spec. Template, runtime, CSS e fontes entram por `include_str!`/`include_bytes!`.

```toml
[profile.release]
opt-level = "z"; lto = "fat"; codegen-units = 1; panic = "abort"; strip = "symbols"
```

`build` faz: lê `.md` → parseia front matter → resolve `![](path)` e `<img src>` → lê bytes → base64 → valida (`</script`, slugs órfãos, convenções de componente) → seleciona módulos e fontes → injeta no template → escreve. Nenhum parser de Markdown do lado Rust.

## Skill `mdhtml-author`

O alvo principal do formato é um agente escrevendo o documento. A skill dispara quando o pedido é "faça um relatório / currículo / ata / spec pra eu compartilhar" e contém:

- **Fluxo**: `mdhtml new --template <tipo>` → escreve o `.md` → `mdhtml build` → `mdhtml check` antes de entregar.
- **Referência compacta** de front matter, containers `:::` e componentes de seção, cada um com um exemplo de 3 linhas.
- **Escolha de tema por tipo de documento** — `editorial` para prosa longa, `technical` para spec/currículo/relatório.
- **As regras que evitam os erros previsíveis**: nunca inventar sintaxe de dados dentro de `:::` (use tabela); nomes semânticos, não classes; imagens referenciadas por caminho normal com os arquivos ao lado do `.md`; não repetir no corpo o que já está no front matter.
- **Critério de pronto**: `mdhtml check` verde, incluindo o veredito de portabilidade.

O `--template` de `mdhtml new` e os `examples/` são o mesmo material — o agente parte de um documento que já respeita a spec em vez de montar do zero.

## Estrutura do repo

```
md.html/
  SPEC.md                      # a especificação, versionada mdhtml/1.0
  crates/mdhtml/
    src/{main,frontmatter,assets,template,check,extract}.rs
    assets/{template.html,runtime.min.js,theme.css,fonts/*.woff2}
  runtime/
    src/{core,render,containers,components,lightbox,copy,toc}.js
    styles/{tokens,base,components,print}.css
    build.mjs
    test/fixtures/*.md
  skills/mdhtml-author/SKILL.md
  examples/{resume,memo,spec,recipe,chapter}.md
  docs/plan_preview.md
  install.sh
  .github/workflows/release.yml
```

## Ordem de construção

1. **SPEC.md** + `template.html` + tokens/base CSS — o formato existe no papel.
2. **Runtime**: parser → containers `:::` → componentes → lightbox → copy/toc.
3. **Exemplos de referência**, montados à mão contra o runtime. *Antes* da CLI, de propósito: valida o formato enquanto mudá-lo ainda é barato.
4. **CLI Rust** — com o formato estável, vira trabalho mecânico.
5. **Skill** `mdhtml-author`.
6. **install.sh + GitHub Actions** (matriz macOS arm64/x64, Linux gnu/musl, Windows; checksums).

## Verificação

- `runtime/test/`: fixtures `.md` → HTML esperado, rodando em Node. Inclui os casos do CommonMark que o parser declara suportar.
- `mdhtml check` sobre todos os `examples/` no CI — falha em slug órfão, `</script`, componente fora de convenção ou requisição externa inesperada.
- Round-trip: `build` → `extract` → diff contra o `.md` original deve ser vazio.
- Manual, por exemplo: abrir via `file://` em Chrome e Safari; copiar markdown (inclusive iOS); lightbox com teclado e swipe; `Cmd+P` e conferir a paginação; desligar JS e confirmar que o Markdown aparece legível.
- Tamanho do binário medido no CI, com teto que falha o build se estourar.

## Fontes

`technical` → **Instrument Sans + Geist Mono**. `editorial` → **Newsreader + Geist Mono**. Todas SIL OFL 1.1, variable, subset `latin` — que cobre pt-BR inteiro, já que nossos acentos vivem em U+00C0–00FF.

| Preset | Arquivos | woff2 | no HTML (base64 +37%) |
|---|---|---|---|
| `technical` | Instrument Sans wght + itálico, Geist Mono | 83 KB | 114 KB |
| `editorial` | Newsreader wght + itálico, Geist Mono | 143 KB | 196 KB |

**Três regras derrubam esses números na maioria dos documentos:**

- **Sempre `:wght@min..max`, nunca `opsz`.** O eixo óptico multiplica o arquivo (Newsreader 57→129 KB, Fraunces 36→118, Recursive 55→298) e `font-optical-sizing: auto` é no-op quando o eixo não está lá.
- **Itálico só se houver `*ênfase*`; mono só se houver código.** Uma ata sem código e sem ênfase paga 29 KB, não 143.
- **O chrome usa pilha do sistema.** Barra, TOC e badges não merecem fonte embutida — só o corpo do documento.

**Descartes, com motivo:**

| Descartado | Por quê |
|---|---|
| Fontshare/ITF (Satoshi, General Sans, Clash Display) | A licença **proíbe redistribuir os arquivos** — e data URI é redistribuição |
| Onest, Bricolage Grotesque, Space Grotesk | Sem itálico. Markdown tem ênfase em todo parágrafo e oblíqua sintética fica feia |
| Inter | Saturada a ponto de sinalizar "template" |
| Source Code Pro | Sem zero cortado e sem ligaduras, apesar de ser a mono mais barata |

```yaml
fonts: auto      # default — embute as do tema, mono só se houver código
fonts: system    # zero bytes, pilha do sistema
fonts: { body: fonts/Herz-Var.woff2 }             # embute a sua
fonts: { url: "https://fonts.googleapis.com/…" }  # externa: CSP relaxada,
                                                  # documento marcado não-portável
```

A OFL exige que o aviso de copyright acompanhe os bytes: o HTML gerado carrega um comentário com nome, copyright e o texto da licença de cada família embutida (~200 bytes cada). **Não re-subsetar** — os arquivos do Google já vêm no subset que queremos e trazem a licença na tabela `name`; mexer nisso configura "Modified Version" pela OFL.

---

## Prior art consultado

| Referência | Lição |
|---|---|
| **Markdeep** | Provou o conceito `.md.html`, mas dialeto próprio, dependência de CDN e `visibility:hidden` até o JS rodar. Parado desde ~2019. |
| **Pandoc `--embed-resources`** | Inlining funciona, mas **descarta o Markdown**. Base64 de 1000+ imagens já causou >12 GB de RAM na conversão. |
| **Docsify** | Serve `.md` cru e renderiza no client — mesma filosofia, mas para sites, sem arquivo único. |
| **llms.txt / "copy for AI"** | O ecossistema de docs resolve o *twin* Markdown com endpoints separados e build steps. Um `.md.html` **é** o twin, sem duplicação. |
| **Protótipo `analise-felipe-broering.html`** | Validou script `text/markdown`, `:::`, copiar-markdown e tokens de DS. Gaps a fechar: fonte via CDN, sem code fence, sem imagens, sem front matter, bold vazando para dentro de code spans. |

Restrições de `file://` que a spec absorve: sem `type="module"` (CORS), sem `fetch`, sem `pushState` (lança exceção), clipboard funciona mas exige gesto síncrono, CSP via `<meta>` ignora `sandbox` e `frame-ancestors`.
