---
pinker-doc: 1
id: development.symbol-index
domain: development
kind: reference
status: active
parent: development
audience:
  - human
  - agent
canonical_for:
  - development.symbol-index
related:
  - development.deterministic-infrastructure-window
  - development.trama
---

# Índice derivado de símbolos e `pink nav localizar`

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

Este documento publica o contrato da quinta capacidade da janela #417,
rastreada pela Issue #434. O índice é reconstruído em memória; não existe
`symbols.toml`, manifesto manual de símbolos ou artefato persistente equivalente.

<!-- @pinker-doc:start
id: development.symbol-index.contract
tags: [desenvolvimento, simbolos, navegacao, trama, determinismo]
aliases:
  - pink nav localizar
  - localizar simbolo
  - indice de simbolos
summary: Autoridades, modelo, schema, vínculos explícitos, ausências e limites de pink nav localizar.
-->
## Autoridades

O catálogo derivado `src/navigation.jsonl`, reconstruível dos marcadores
`@pinker-nav`, fornece identidade, categoria, papel e regiões. O catálogo
`docs/navigation.jsonl` resolve somente IDs documentais explicitamente
vinculados. Ambos são carregados diretamente pelas APIs Rust `CodeCatalog` e
`DocCatalog`; `pink nav localizar` não executa `pink nav`, `pink doc`, `pink
estado`, grep ou outro subprocesso.

Os campos opcionais da região são:

```text
@pinker-nav:symbol IDENTIDADE|NOME|CATEGORIA|PAPEL
@pinker-nav:related-symbol IDENTIDADE
@pinker-nav:test-for IDENTIDADE
@pinker-nav:symbol-doc IDENTIDADE|ID_DOCUMENTAL
```

`IDENTIDADE` é qualificada por segmentos separados por `::`; `NOME` é a forma
exata não qualificada aceita na consulta. `PAPEL` é `declaration` ou
`implementation`. Repetição inválida, identidade conflitante e destino
inexistente são erros estruturais. `test-for` só é válido numa região cuja
camada seja `evidencia`. O destino de `symbol-doc` precisa ser ID de documento,
ID de seção ou conceito com autoridade única em `canonical_for`.

`related`, aliases, summaries, nomes de arquivos, proximidade textual e
ocorrências na prosa não criam vínculo de símbolo. Em particular, aliases
documentais continuam servindo à busca documental, não à atribuição de
ownership. A semântica publicada de `canonical_for` e `related` não foi
ampliada.

## Símbolos suportados

| Categoria | Origem e identidade | Declaração/implementação | Estabilidade e limite |
|---|---|---|---|
| `rust-function` | binding explícito numa região Rust; identidade qualificada e nome exato | cada papel é declarado separadamente | metadata v1; não promete descoberta automática de todo item Rust |
| `rust-type` | binding explícito numa região Rust | a declaração do tipo e regiões de implementação podem ser distintas | metadata v1; não infere impls por parsing textual |
| `pinker-function` | binding explícito numa região `.pink` da raiz oficial `apps/` | papéis publicados pelo marcador | metadata v1; não altera nem expõe AST ou semântica da linguagem |
| `UNKNOWN` | binding que registra explicitamente que a autoridade não classifica o item | papéis continuam explícitos | não promove informação ausente a categoria conhecida |

A consulta casa somente a identidade integral ou o nome integral, com igualdade
exata. Um mesmo nome pode produzir vários candidatos; a CLI os ordena pela
identidade e nunca escolhe um homônimo arbitrariamente. O índice não promete
cobertura de todo identificador existente no repositório.

## Relações e ausência

Cada item material carrega catálogo, path, região e campo que autorizou o
vínculo. Paths públicos são sempre repo-relativos.

- `declaration`: regiões com binding e papel `declaration`;
- `implementation`: regiões com binding e papel `implementation`;
- `regions`: bindings, `related-symbol` e regiões de evidência vinculadas;
- `documentation`: `symbol-doc` resolvido por ID ou `canonical_for` no catálogo
  documental;
- `tests`: somente `test-for` explícito em camada `evidencia`.

Ausência de vínculo explícito é `UNKNOWN`, nunca uma lista vazia apresentada
como conhecimento completo. Catálogo documental ausente torna documentação
`UNAVAILABLE`; catálogo documental inválido continua falha estrutural. Uma
relação `KNOWN` sempre possui ao menos um item.

## CLI e schema

```text
pink nav localizar SÍMBOLO [--repo DIRETÓRIO]
pink nav localizar SÍMBOLO [--repo DIRETÓRIO] --json
```

`--limite` não pertence a `localizar`: homônimos legítimos não são truncados.
O schema JSON próprio é `1`, no campo raiz `schema`. O documento contém
`query` e `candidates`; cada candidato contém `identity`, `name`, `kind`,
`stability`, `declaration`, `implementation`, `regions`, `documentation` e
`tests`. Toda relação contém `status`, `reason` e `items`. A saída humana e o
JSON recebem exclusivamente o mesmo `LocateReport`.

Códigos de saída:

- `0`: consulta encontrada;
- `2`: uso inválido;
- `3`: catálogo obrigatório ausente/inválido ou vínculo estrutural inválido;
- `4`: nenhum símbolo estruturado com identidade ou nome exato;
- `5`: continua reservado a fonte/âncora ou drift nas demais operações `nav`.

`pink nav buscar` permanece busca textual ranqueada de regiões por chave,
domínio, camada, summary e path. `pink nav localizar` resolve exclusivamente
símbolos e relações estruturados.

## Somente leitura e determinismo

O comando abre os dois catálogos e monta o modelo em memória. Não sincroniza
catálogos, não prepara ou aceita projeções, não altera snapshots ou estado do
agente, não cria arquivo, não executa Git/GitHub, não usa rede e não modifica
mtime. A ordenação usa estruturas ordenadas e desempates explícitos; a mesma
consulta em roots absolutos distintos produz JSON byte-idêntico, sem root
absoluto, hostname, usuário, PID, timestamp, mtime ou ANSI.

## Limites

Não há parsing geral de Rust, grafo de chamadas, análise interprocedural ou
inferência por substring. Símbolos sem binding não pertencem ao índice. Uma
categoria ou relação só deixa `UNKNOWN` quando uma autoridade explícita passa a
publicá-la. A entrega não altera AST, parser, IR, CFG, seleção, máquina,
interpretador, backend, runtime ou semântica da Pinker.
<!-- @pinker-doc:end development.symbol-index.contract -->
