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
útil sozinha. Este PR entrega as **Ondas 0, 1 e 2**; as demais estão inventariadas
e explicitamente adiadas.

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

## Onda 3+ — modelos, frontend, semântica, execução (adiadas)

Estes arquivos estão inventariados; suas âncoras serão adicionadas nas ondas
indicadas. Revisão atual: `estrutural` (módulo-doc e estrutura), a ser elevada a
`integral` ao ancorar.

| Arquivo | Camada | Propósito (do módulo-doc/estrutura) | Complexidade | Âncoras atuais | Onda-alvo |
|---|---|---|---|---|---|
| `src/ast.rs` | ast | Modelo da árvore sintática (programa, itens, comandos, expressões, tipos) e serialização JSON. | alta | — | 3 |
| `src/ir.rs` | ir | IR estruturada após a semântica; preserva estrutura e adiciona tipos/slots; lowering a partir da AST. | alta | — | 3/5 |
| `src/cfg_ir.rs` | cfg | CFG com blocos básicos e terminadores explícitos; lowering da IR. Contém `cfg.logica.*`. | alta | `cfg.logica.curto-circuito`, `cfg.logica.slot-logico` | 3/5 |
| `src/instr_select.rs` | select | Seleção de instruções a partir do CFG. | alta | — | 3/5 |
| `src/abstract_machine.rs` | machine | Máquina de pilha abstrata; lowering da seleção. | alta | — | 3/5 |
| `src/lexer.rs` | lexer | Tokenização (comentários, strings/escapes/interpolação, números, keywords, operadores). | alta | — | 4 |
| `src/parser.rs` | parser | Parsing recursivo-descendente completo da Pinker (funções, tipos, leques, genéricos, tratos, closures, `tentar`/`propagar`, imports, fluxo). | transversal | — | 4 |
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

## Cobertura desta rodada

| Métrica | Valor |
|---|---:|
| Arquivos de produção em `src/` (excl. gerados e fixtures) | 30 |
| Arquivos integralmente revisados e ancorados nesta rodada | 18 |
| Arquivos apenas inventariados (estrutural) | 12 |
| Regiões existentes antes | 2 |
| Regiões adicionadas | 25 |
| Regiões no catálogo | 27 |
| Chaves duplicadas | 0 |
| Erros de validação (`nav verificar`) | 0 |

### Cobertura por camada

| Camada | Ancorada? | Regiões | Status |
|---|---|---:|---|
| token | sim | 2 | completo |
| error | sim | 2 | completo |
| layout | sim | 1 | completo |
| repl | sim | 2 | completo |
| palette | sim | 2 | completo |
| printer | sim | 1 | completo |
| ir/cfg/select/machine/backend-text (validadores) | sim | 5 | entry ancorado |
| trama (doc/nav/change/projection/text_norm/jsonl) | sim | 10 | completo |
| ast/ir/cfg/select/machine (modelos+lowerings) | não | 2 (cfg.logica.*) | pendente (ondas 3/5) |
| lexer/parser/semantic | não | 0 | pendente (ondas 4/5) |
| interpreter/backend-text/backend-s/runtime | não | 0 | pendente (onda 6) |
| cli/editor/boot | não | 0 | pendente (onda 7) |
| tests/apps | não | 0 | pendente (ondas 8/9, requer ampliar raízes) |

## Próximo ponto de retomada

**Onda 3 — modelos de dados e representações:** ancorar `src/ast.rs` (programa,
item, comando, expressão, tipos) e as definições estruturais de `src/ir.rs`,
`src/cfg_ir.rs`, `src/instr_select.rs`, `src/abstract_machine.rs`, preservando as
âncoras `cfg.logica.*` já existentes e sem marcar cada variante de enum
isoladamente.
