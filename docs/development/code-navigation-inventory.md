---
pinker-doc: 1
id: development.code-navigation
domain: development
kind: reference
status: active
parent: development
audience:
  - human
  - agent
related:
  - development
---

# Inventário de navegação de código (cartografia semântica)

- **Classe:** Engine
- **Papel:** inventário humano da cartografia `@pinker-nav`
- **Status:** ativo (em ondas)

Este documento é o **inventário humano** da cartografia semântica do código
Pinker. Ele explica, arquivo a arquivo, para que cada peça serve e quais regiões
receberam âncoras `@pinker-nav`. O endereçamento para máquinas vive no catálogo
`src/navigation.jsonl` (consultável por `pink nav`); este inventário não o
substitui.

A cartografia avança em **ondas**, do mais simples ao mais complexo. Cada onda é
útil sozinha. As **Ondas 0, 1 e 2** já estão na `main`; esta rodada adiciona a
**Onda 3** (modelos de dados). As demais estão inventariadas e explicitamente
adiadas.

## Contrato do scanner (limitação registrada)

O scanner de `pink nav` indexa hoje **somente `src/**/*.rs`** (ver
`trama.codigo.catalogo` em `src/nav.rs`: os caminhos são derivados com prefixo
`src/`). Runtime, testes e apps **não** são varridos.

**Estratégia escolhida: B** — concluir a cobertura de `src/` progressivamente e
deixar `runtime/`, `tests/` e `apps/` explicitamente inventariados para PRs
seguintes. Não ampliamos o scanner de forma improvisada para não quebrar o
contrato do catálogo (chaves únicas, sem sobreposição, hash determinístico). A
ampliação de raízes é, ela mesma, uma onda posterior com testes próprios.

## Convenção de chaves

`<camada>.<domínio>.<responsabilidade>` — ver `docs/development/README.md` e o
contrato em `src/nav.rs`. Chaves estáveis a movimentos de linha/arquivo; sem
`phase` quando não há proveniência canônica confirmada.

## Legenda

- **Revisão:** `integral` = lido linha a linha nesta rodada; `estrutural` =
  módulo-doc e estrutura lidos, corpo não percorrido integralmente (será
  aprofundado na onda correspondente).
- **Complexidade:** simples · moderada · alta · transversal.

---

## Onda 1 — módulos utilitários (concluída)

| Arquivo | Camada | Propósito | Complexidade | Âncoras adicionadas | Revisão |
|---|---|---|---|---|---|
| `src/token.rs` | token | Vocabulário de tokens (palavras-chave PT, operadores, literais) e representação de posição/span de origem. | simples | `token.lexico.vocabulario`, `token.representacao.spans` | integral |
| `src/error.rs` | error | Taxonomia unificada de erros do pipeline e renderização para o CLI com linha de origem e cursor `^`. | simples | `error.diagnostico.taxonomia`, `error.diagnostico.contexto-fonte` | integral |
| `src/layout.rs` | layout | Layout estático (tamanho/alinhamento) de tipos e offsets de campos de struct, com arredondamento e proteção contra recursão. | moderada | `layout.tipos.memoria` | integral |
| `src/repl.rs` | repl | Laço leitura-avaliação-impressão e avaliação de um trecho como `principal` temporária por todo o pipeline. | simples | `repl.ciclo.leitura-avaliacao`, `repl.avaliacao.pipeline` | integral |
| `src/palette.rs` | palette | Identidade cromática canônica da Pinker (RGB/ANSI, cores, tema) e helpers de estilização com respeito a `NO_COLOR`. | simples | `palette.visual.identidade`, `palette.visual.estilizacao` | integral |
| `src/printer.rs` | printer | Renderização textual indentada da AST (`--ast`); a variante JSON delega ao serializador da AST. | moderada | `printer.ast.renderizacao` | integral |

## Onda 2 — validadores e ferramentas da Trama (concluída)

| Arquivo | Camada | Propósito | Complexidade | Âncoras adicionadas | Revisão |
|---|---|---|---|---|---|
| `src/ir_validate.rs` | ir | Valida invariantes da IR estruturada (constantes, slots, tipos) antes do lowering para CFG. | alta | `ir.validacao.invariantes` | integral (entry) |
| `src/cfg_ir_validate.rs` | cfg | Valida o CFG IR (blocos, terminadores, alcançabilidade, tipos entre blocos). | alta | `cfg.validacao.invariantes` | integral (entry) |
| `src/instr_select_validate.rs` | select | Valida a camada de seleção de instruções (operandos, temporários, boa formação). | moderada | `select.validacao.invariantes` | integral (entry) |
| `src/abstract_machine_validate.rs` | machine | Valida a máquina de pilha (disciplina de pilha, labels, aridade de calls). | alta | `machine.validacao.invariantes` | integral (entry) |
| `src/backend_text_validate.rs` | backend-text | Valida o pseudo-assembly do backend textual (instruções, rótulos, referências). | moderada | `backend-text.validacao.invariantes` | integral (entry) |
| `src/text_norm.rs` | trama | Normalização determinística de consultas (minúsculas, sem diacríticos, termos). | simples | `trama.consultas.normalizacao` | integral |
| `src/jsonl.rs` | trama | Leitor mínimo de JSON de uma linha para reconstruir os catálogos. | simples | `trama.catalogo.leitor-jsonl` | integral |
| `src/doc.rs` | trama | Marco documental, política forward-only e projeções de `.pinker/doc.toml`; gate anti-retroatividade. | moderada | `trama.documentos.marco` | integral |
| `src/doc_index.rs` | trama | Catálogo documental (geração schema 2 e verificação) e superfície de consulta (loader JSONL, busca, validação de âncora). | alta | `trama.documentos.catalogo`, `trama.documentos.consulta` | integral |
| `src/nav.rs` | trama | Catálogo de código (geração/verificação) e superfície de consulta (loader JSONL, busca, validação de região/hash). | alta | `trama.codigo.catalogo`, `trama.codigo.consulta` | integral |
| `src/change.rs` | trama | Manifesto de mudança (parsing + validação real de schema) e ledger mecânico derivado. | alta | `trama.mudancas.manifesto`, `trama.mudancas.ledger` | integral |
| `src/projection.rs` | trama | Projeções documentais determinísticas em regiões geradas (`history`/`state`/`roadmap`). | moderada | `trama.projecoes.geracao` | integral |

**Nota sobre os validadores:** cada validador é uma unidade de validação
independente (§6.10 do prompt). A âncora cobre o ponto de entrada `validate_program`
e todas as checagens até o fim do arquivo — uma única responsabilidade
consultável (“onde os invariantes de X são verificados”). Não se fragmentou em
uma âncora por helper.

## Onda 3 — modelos de dados e representações (concluída)

Âncoras nas **definições estruturais** (o modelo de dados), não nos lowerings —
estes ficam para a Onda 5. As âncoras históricas `cfg.logica.*` foram
preservadas.

| Arquivo | Camada | Propósito da(s) região(ões) | Complexidade | Âncoras adicionadas | Revisão |
|---|---|---|---|---|---|
| `src/ast.rs` | ast | Modelo da AST separado por responsabilidade: programa/itens, tipos, comandos, expressões e o escritor JSON. | alta | `ast.programa.estrutura`, `ast.tipos.representacao`, `ast.comandos.representacao`, `ast.expressoes.representacao`, `ast.serializacao.json` | integral |
| `src/ir.rs` | ir | Modelo de dados da IR estruturada (programa, funções, blocos, instruções, valores, tipos, operadores). | alta (modelo) | `ir.modelo.representacao` | modelo integral; lowering → Onda 5 |
| `src/cfg_ir.rs` | cfg | Modelo de dados do CFG IR (blocos básicos, instruções, terminadores, operandos). | alta (modelo) | `cfg.modelo.representacao` (+ `cfg.logica.*` preservadas) | modelo integral; lowering → Onda 5 |
| `src/instr_select.rs` | select | Modelo de dados da seleção de instruções (instruções selecionadas, terminadores). | alta (modelo) | `select.modelo.representacao` | modelo integral; lowering → Onda 5 |
| `src/abstract_machine.rs` | machine | Modelo de dados da máquina de pilha (instruções de pilha, terminadores, slots). | alta (modelo) | `machine.modelo.representacao` | modelo integral; lowering → Onda 5 |

## Onda 4+ — frontend, semântica, execução, orquestração (adiadas)

Inventariados; revisão atual `estrutural`.

| Arquivo | Camada | Propósito (do módulo-doc/estrutura) | Complexidade | Âncoras atuais | Onda-alvo |
|---|---|---|---|---|---|
| `src/lexer.rs` | lexer | Tokenização (comentários, strings/escapes/interpolação, números, keywords, operadores). | alta | — | 4 |
| `src/parser.rs` | parser | Parsing recursivo-descendente completo da Pinker (funções, tipos, leques, genéricos, tratos, closures, `tentar`/`propagar`, imports, fluxo). | transversal | — | 4 |
| `src/ir.rs` (lowering) | ir | Lowering AST→IR (`lower_program`, `LoweringContext`, `FunctionLowerer`). | transversal | modelo ancorado | 5 |
| `src/cfg_ir.rs` (lowering) | cfg | Lowering IR→CFG; contém `cfg.logica.*`. | transversal | `cfg.logica.*` | 5 |
| `src/instr_select.rs` (lowering) | select | Lowering CFG→seleção. | alta | modelo ancorado | 5 |
| `src/abstract_machine.rs` (lowering) | machine | Lowering seleção→máquina. | alta | modelo ancorado | 5 |
| `src/semantic.rs` | semantic | Checagem semântica em duas passagens (escopos, nomes, tipos, `encaixe`, tratos/impl, monomorfização). | transversal | — | 5 |
| `src/interpreter.rs` | interpreter | Executa a máquina validada; valores de runtime, frames, intrínsecas, coleções (listas/mapas/versos). | transversal | — | 6 |
| `src/backend_text.rs` | backend-text | Lowering para pseudo-assembly textual a partir da seleção. | alta | — | 6 |
| `src/backend_s.rs` | backend-s | Emissão de `.s` e toolchain nativa (ABI SysV, alinhamento, chamadas ao runtime). | alta | — | 6 |
| `runtime/pinker_rt/src/lib.rs` | runtime | Runtime nativo linkado por `pink build --nativo`; alocação, coleções nativas, ABI. **Fora do scanner atual.** | transversal | — | 6 (após ampliar raízes) |
| `src/main.rs` | cli | Orquestração da CLI: parsing de flags, roteamento, pipeline de análise/build, importação de módulos, link nativo, comandos `doc`/`nav`. | transversal | — | 7 |
| `src/editor_tui.rs` | editor | TUI mínima oficial (Fase 136): estado, comandos, ações Pinker reais. | moderada | — | 7 |
| `src/boot.rs` | boot | Fronteiras freestanding: entry `_start`, linker script e stub de kernel. | simples | — | 7 |

## Arquivos sem candidatos a âncora

Registrados para não desaparecerem da análise; não recebem âncoras.

| Arquivo | Motivo |
|---|---|
| `src/lib.rs` | Apenas declarações de módulos (`pub mod ...`). |
| `src/bin/pinker_fase16x_*.rs` | Binários-fixture minúsculos (3–35 linhas) usados por testes de I/O; sem responsabilidade nomeável. |
| `src/navigation.jsonl` | Catálogo **gerado**; nunca é fonte de âncoras. |

## Testes e apps (adiados — fora do scanner atual)

Inventariados para as Ondas 8 e 9. Enquanto o scanner indexar só `src/`, estes
não são varridos; suas âncoras dependem da ampliação de raízes (onda própria).

- `tests/*.rs` — evidência por camada (lexer, parser, semântica, IR/CFG/seleção/
  máquina, interpretador, backends, runtime nativo, Trama, CLI, paridade nativa).
  Marcar apenas grupos de evidência conceituais (ex.: `tests.backend-s.abi-argumentos`,
  `tests.trama.manifesto-imutavel`) — nunca uma âncora por `#[test]`.
- `apps/guardiao_pinker/principal.pink` — Guardião Pinker (auditoria de contratos
  do repositório); marco de app real em Pinker. Candidato: `apps.guardiao.auditoria`.

## Cobertura acumulada (após Onda 3)

| Métrica | Valor |
|---|---:|
| Arquivos de produção em `src/` (excl. gerados e fixtures) | 30 |
| Arquivos com modelo/responsabilidade ancorada | 23 |
| Arquivos apenas inventariados (estrutural) | 7 |
| Regiões antes da Onda 3 | 27 |
| Regiões adicionadas na Onda 3 | 9 |
| Regiões no catálogo | 36 |
| Chaves duplicadas | 0 |
| Erros de validação (`nav verificar`) | 0 |

### Cobertura por camada (contagem real no catálogo)

| Camada | Regiões | Composição |
|---|---:|---|
| token | 2 | vocabulário, spans |
| error | 2 | taxonomia, contexto-fonte |
| layout | 1 | memória |
| repl | 2 | ciclo, pipeline |
| palette | 2 | identidade, estilização |
| printer | 1 | renderização |
| ast | 5 | programa, tipos, comandos, expressões, serialização |
| ir | 2 | modelo (Onda 3) + validador |
| cfg | 4 | modelo (Onda 3) + validador + `cfg.logica.*` (históricas) |
| select | 2 | modelo (Onda 3) + validador |
| machine | 2 | modelo (Onda 3) + validador |
| backend-text | 1 | validador |
| trama | 10 | normalização, jsonl, marco, catálogos e consultas doc/código, manifesto, ledger, projeções |
| **total** | **36** | |

Pendentes (sem âncora): lowerings de ir/cfg/select/machine (Onda 5),
lexer/parser (Onda 4), semantic (Onda 5), interpreter/backend-s/runtime (Onda 6),
cli/editor/boot (Onda 7), tests/apps (Ondas 8/9, após ampliar raízes).

## Próximo ponto de retomada

**Onda 4 — frontend léxico e parsing local:** ancorar `src/lexer.rs`
(comentários, strings/escapes/interpolação, números, keywords, operadores) e as
rotinas de fronteira clara de `src/parser.rs`, começando pelas de contorno
inequívoco e adiando regiões com features profundamente intercaladas. Os
lowerings de `ir`/`cfg_ir`/`instr_select`/`abstract_machine` ficam para a Onda 5,
já com o modelo de dados ancorado nesta rodada.
