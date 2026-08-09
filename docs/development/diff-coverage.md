---
pinker-doc: 1
id: development.diff-coverage
domain: development
kind: reference
status: active
parent: development
audience:
  - human
  - agent
canonical_for:
  - development.diff-coverage
related:
  - development.deterministic-infrastructure-window
  - development.symbol-index
  - development.projection-snapshots-contract
---

# Cobertura de diff somente leitura

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

Este documento publica o contrato da sexta capacidade da janela #417,
rastreada pela Issue #438. A cobertura não mantém grafo, manifesto de ownership
ou cache próprio: ela deriva cada relação das autoridades locais vigentes.

<!-- @pinker-doc:start
id: development.diff-coverage.contract
tags: [desenvolvimento, diff, cobertura, navegacao, determinismo]
aliases:
  - pink nav cobertura-diff
  - cobertura de diff
summary: Entrada, autoridades, relações, schema, ausências e limites de pink nav cobertura-diff.
-->
## Entrada e arquivos

O comando recebe um unified diff UTF-8 por stdin:

```text
git diff --unified=0 | pink nav cobertura-diff
git diff --unified=0 | pink nav cobertura-diff --json
```

Git é apenas um exemplo de produtor externo. A Pinker não o executa, não
seleciona refs e não consulta rede. O diff é a autoridade dos arquivos,
statuses e coordenadas novas; os paths publicados são repo-relativos.

O parser aceita arquivos adicionados, modificados, removidos, renomeados e
marcadores binários, valida paths e confere o consumo declarado dos hunks.
Linhas `+` formam intervalos novos canônicos. Uma deleção pura não possui
coordenada exata no catálogo corrente e, por isso, não é aproximada pela linha
anterior ou seguinte: suas relações dependentes permanecem `UNKNOWN` com
`W-DIFF-DELETION-ONLY`. Diff vazio é sucesso com zero arquivos.

## Autoridades e relações

Cada item material inclui `authority` com origem, path, registro e campo.

### Regiões tocadas

`src/navigation.jsonl` fornece os spans publicados. Uma linha nova toca uma
região somente quando intersecta o intervalo entre seus marcadores
`@pinker-nav:start/end`. Conteúdo sem região, binário e deleção pura não
recebem uma região inferida.

### Documentos relacionados

`docs/navigation.jsonl` fornece duas relações:

- o próprio documento ou seção cujo path e span foram alterados;
- `symbol-doc` resolvido pelo índice derivado para identidades das regiões
  tocadas.

Aliases, proximidade textual, nomes parecidos e a prosa de summaries não criam
relações documentais.

### Projeções afetadas

Dois domínios canônicos permanecem distintos:

- snapshots históricos de navegação: `.pinker/projections` e a composição
  oficial relacionam regiões presentes, inputs diretos e receitas consumidas;
- projeções documentais: `.pinker/doc.toml` relaciona targets diretos e os
  campos `updates.*: true` do manifesto corrente alterado.

Uma projeção não é declarada afetada por extensão, nome de arquivo ou
semelhança. Artefato inválido ou autoridade ausente produz aviso e
`UNAVAILABLE` quando nenhuma relação válida puder ser publicada.

### Testes associados

Uma região tocada na camada `evidencia` é teste direto. Para código de
produção, somente `@pinker-nav:test-for` resolvido pelas identidades estruturais
das regiões tocadas cria associação. Nome `tests/`, convenção de sufixo e
ocorrência textual não bastam.

## Ausência explícita

Cada categoria contém `status`, `reason` e `items`:

- `KNOWN`: ao menos uma relação foi estabelecida por autoridade explícita;
- `UNKNOWN`: as autoridades disponíveis não determinam a relação;
- `UNAVAILABLE`: a autoridade necessária não pôde ser carregada integralmente.

Avisos estáveis `W-DIFF-*` explicam binário, deleção pura, categoria sem vínculo
ou autoridade parcial. A ausência nunca é convertida silenciosamente em lista
conhecida vazia.

## CLI, schema e exits

```text
pink nav cobertura-diff [--repo DIRETÓRIO] [--json]
```

O comando não aceita arquivo posicional nem `--limite`. O schema JSON próprio é
`1`. A raiz contém `schema`, `source` e `files`; cada arquivo contém `path`,
`old_path`, `status`, `changed_lines`, as quatro relações e `warnings`. Saída
humana e JSON são derivadas do mesmo `CoverageReport`.

- `0`: diff válido, inclusive vazio ou com relações `UNKNOWN`;
- `2`: uso inválido;
- `3`: configuração ou catálogo obrigatório ausente/inválido;
- `6`: stdin, UTF-8, limite, path, hunk ou vínculo estrutural inválido.

## Somente leitura, determinismo e limites

O derivador não abre fontes, escreve, sincroniza catálogos, executa Git,
subprocessos ou rede. A CLI lê somente stdin e autoridades locais. Ordenação
por estruturas canônicas torna texto e JSON byte-idênticos em roots absolutos
distintos e exclui root absoluto, relógio, PID, usuário, mtime e ANSI.

Ficam fora: aplicar correções, alterar fontes, selecionar testes por heurística,
análise interprocedural, grafo geral, edição transacional, novo executor e
mudança de AST, IR, CFG, seleção, máquina abstrata, interpretador, backend,
runtime, ABI ou semântica da Pinker.
<!-- @pinker-doc:end development.diff-coverage.contract -->
