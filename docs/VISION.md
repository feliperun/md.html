# md.html — Product Vision

> TODO: replace the placeholders below with the real vision. Keep it short and
> opinionated — this is the "why", not the "how".

## Why this, why now

Markdown é a língua dos agentes: escrevemos, editamos e analisamos tudo nele. Mas
no momento em que o documento precisa atravessar para um humano, a língua falha —
ninguém quer ler `##` e `**` numa tela. As alternativas de hoje perdem um dos dois
lados: o PDF perde o Markdown, o export de HTML perde a fonte, e reparsear um PDF
para devolver ao agente é trabalho que ninguém quer fazer.

## The problem

- O autor de conteúdo (humano ou agente) trabalha em Markdown o dia inteiro.
- Compartilhar com colegas em Markdown cru é feio e pouco legível.
- Exportar para HTML ou PDF corta o caminho de volta: o agente do destinatário
  precisa reparsear, adivinhar estrutura e perder fidelidade.
- O resultado é gente lendo `**negrito**` literal, ou documentos bonitos que
  viraram beco sem saída.

## The insight

Um único arquivo pode ser **as duas coisas ao mesmo tempo**. Se o Markdown é a
fonte da verdade e o HTML só existe para vesti-lo, então: abre no browser e é
página; clica em "Copiar Markdown" e é documento; edita o bloco e dá F5 e é
editor. Nada de gerar PDF, nada de reparsear, nada de manter dois artefatos em
sincronia.

## Principles

- **Self-contained é garantia, não desejo.** Zero requisições de rede, imposto
  por CSP e verificado por `mdhtml check` — portabilidade deixa de ser promessa.
- **O `#mdhtml-source` é o documento.** Tudo que importa é o `textContent` dele;
  "Copiar Markdown" é literalmente esse valor. Nenhuma derivação pode virar fonte.
- **O visual não polui o Markdown.** Front matter, tokens e componentes escolhem
  apresentação de conteúdo que continua sendo Markdown válido — e degrada para
  prosa em qualquer outro renderizador.
- **Elegância é requisito.** A missão do HTML é renderizar bonito; documento feio
  é bug, não gosto.

## Near-term horizon

Spec 1.0 escrita, runtime no navegador, CLI Rust (`build`, `check`, `extract`,
`new`) e os cinco exemplos de referência (currículo, ata, spec, receita,
capítulo). O rito de pronto: `mdhtml build doc.md` gera um `.md.html` que abre
elegante via `file://` e devolve o Markdown idêntico na cópia.

## Non-goals (for now)

- Hosting/URLs — o offline vem primeiro; o serviço em nuvem é o próximo produto.
- PDF — o navegador imprime; geração dedicada é outro produto.
- Matemática (KaTeX/MathJax) e diagramas não-embutíveis.
- Multi-língua do runtime — `lang:` existe, i18n da interface não.

## Related docs

- [Architecture](ARCHITECTURE.md) · [Abstractions](ABSTRACTIONS.md) · [ADRs](adr/README.md)
