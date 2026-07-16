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
útil sozinha. As **Ondas 0–5D** já estão na `main`; esta rodada adiciona a
**Onda 5E** (lowering CFG → seleção → máquina em `src/instr_select.rs` e
`src/abstract_machine.rs`), fechando a cadeia de lowerings. A execução, os
backends e o runtime (Onda 6) e as demais camadas seguem inventariados e
explicitamente adiados.

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

## Onda 4 — frontend léxico e parsing local (concluída)

Frontend integralmente **revisado**. O lexer está totalmente cartografado; o
parser está integralmente revisado com **cartografia parcial** — a maquinaria de
monomorfização de genéricos residente no parser foi **adiada** (não é léxico/
sintático; ver adiados).

### Lexer (`src/lexer.rs`) — revisão integral

| Âncora | Responsabilidade |
|---|---|
| `lexer.espacos-comentarios.consumo` | Espaços, comentários de linha `//` e de bloco `/* */` aninhados; bloco não terminado encerra no EOF sem token. |
| `lexer.fluxo.tokenizacao` | Laço principal: operadores/delimitadores (incl. multi-caractere), inteiros, strings simples e `"""`, escapes, identificadores × palavras-chave, `$"..."` interpolado, `?`, EOF e erros léxicos. |

**Decisão de granularidade (lexer):** identificadores, números, strings,
interpolação e operadores são **braços do `match` dentro do único método
`tokenize`**, não funções separadas. Fragmentá-los exigiria refatoração
(proibida nesta onda), então são cobertos por `lexer.fluxo.tokenizacao` — uma
região conceitual única e precisa (§5.5).

### Parser (`src/parser.rs`) — revisão integral, cartografia parcial

| Âncora | Responsabilidade |
|---|---|
| `parser.fluxo.nucleo` | Cursor de tokens (peek/advance/check/consume) e erro `Expected`. |
| `parser.programa.estrutura` | Entrada `parse`: `pacote`, imports, freestanding e despacho de itens de topo. |
| `parser.tipos.gramatica` | Gramática de tipos → `ast::Type` (só sintaxe). |
| `parser.declaracoes.tipos` | `apelido`, `ninho` (struct), `impl`, `trato`, `leque` (enum). |
| `parser.encaixe.expressao` | Desugaring de `encaixe` (pattern matching) em `talvez`/`senao`. |
| `parser.resultado.tentar-propagar` | Desugaring de `tentar` e `propagar`/`propagar?`. |
| `parser.closures.expressao` | Funções anônimas e vínculos de valor-função. |
| `parser.funcoes.declaracao` | `carinho ...` incl. parâmetros de tipo genéricos. |
| `parser.constantes.declaracao` | `eterno nome: tipo = expr;`. |
| `parser.comandos.bloco` | Blocos e todos os comandos (`nova`/`muda`/`mimo`/fluxo/`falar`/asm). |
| `parser.lacos.for-each` | Desugaring de `para cada X em COL`. |
| `parser.expressoes.precedencia` | Escada de precedência + unários. |
| `parser.expressoes.primarias` | Expressões primárias (literais, listas/mapas, struct/leque). |
| `parser.expressoes.postfix` | Cadeia postfix: chamada, campo, índice, genérica explícita, cast. |
| `parser.texto.interpolacao` | Desugaring de `$"..."` → `formatar_verso`. |

**Adiado (parser):** a maquinaria de **monomorfização de genéricos** residente no
parser (`generic_type_key`, `substitute_*`, `instantiate_generic_functions`,
`instantiate_generic_enums`, `instantiate_function_param_functions` — ~
`src/parser.rs` entre `parser.funcoes.declaracao` e `parser.constantes.declaracao`)
**não é responsabilidade léxica/sintática**: é monomorfização, explicitamente
fora do escopo da Onda 4 (§2). Foi cartografada na **Onda 5B** (ver seção
própria). Helpers isolados (`register_collection_type`, name-mangling de `impl`)
ficam sem âncora por serem plumbing (§7).

## Onda 5A — checagem semântica (concluída)

`src/semantic.rs` **integralmente revisado** (linha a linha) e cartografado nas
responsabilidades semânticas estáveis. O `SemanticChecker` roda em duas
passagens (declaração global → verificação de corpos); as âncoras seguem essa
espinha, do frontal (importações, sistema de tipos, escopos) ao despacho de
chamadas.

| Âncora | Responsabilidade |
|---|---|
| `semantic.importacoes.familias` | Famílias de intrínsecas importáveis e validação de `trazer` (família inteira sim; seletiva/desconhecida não). |
| `semantic.tipos.sistema` | Compatibilidade estrutural, resolução de tipos nomeados/aliases (com recursão), validação de struct, regras de inteiro/cast e faixa de literais. |
| `semantic.escopos.variaveis` | Pilha de escopos léxicos: `declare_var` (sem sombreamento no mesmo escopo) e `resolve_var` (com fallback para constantes). |
| `semantic.programa.duas-passagens` | Entrada `check_program`: coleta global (funcs/consts/aliases/structs/leques/tratos, conflitos e cargas de variante) e disparo da verificação. |
| `semantic.tratos.contratos` | Contratos de trato/`impl`: cobertura exata, compatibilidade de assinatura (aridade, parâmetros, retorno). |
| `semantic.funcoes.verificacao` | `principal` (política fixa), corpo de constante e de função com alcançabilidade de retorno. |
| `semantic.comandos.verificacao` | Verificação de comandos do bloco (`mimo`/atribuições/fluxo/`falar`/`sussurro`/expressão-comando). |
| `semantic.fluxo.retornos` | Ramo `talvez`/`senão` aninhado, checagem de `mimo` de retorno e análise superficial de alcançabilidade. |
| `semantic.expressoes.verificacao` | Despacho de tipos de expressão (`check_expr`): literais, acessos, cast, `peso`/`alinhamento`, binárias (incl. aritmética de ponteiro) e unárias. |
| `semantic.chamadas.despacho` | Resolução de método de `impl`, chamada nomeada e o grande despachante `check_call_expr` (variantes, `encaixe`, intrínsecas de lista/mapa/texto/CSV/JSON/tempo/processo). |

**Decisão de granularidade (semantic):** `check_call_expr` é um único
despachante de ~4100 linhas com braços sequenciais fortemente interligados
(construção de variante, desugaring de `encaixe`, intrínsecas). Fragmentá-lo
exigiria refatoração (proibida nesta onda), então fica coberto por uma região
conceitual ampla, `semantic.chamadas.despacho`, junto aos resolvedores de método
que ele consome (§6.8). Helpers de plumbing do `SemanticChecker` (construtor,
`type_key`, `parse_impl_function_name`, `push_scope`/`pop_scope`,
`resolve_struct_field_type`) ficam sem âncora por serem infraestrutura (§7).

**Adiado (Ondas 5B–5E):** a **monomorfização de genéricos residente no parser**
(`src/parser.rs`) é a **Onda 5B** (agora concluída, abaixo); os **lowerings** por
camada seguem uma onda cada (5C–5E). Ver adiados abaixo.

## Onda 5B — monomorfização e especialização no parser (concluída)

`src/parser.rs` já havia sido **integralmente revisado** na Onda 4 (cartografia
léxico/sintática); esta rodada **releu o arquivo integralmente** e aprofundou
**somente** a maquinaria de monomorfização/especialização — o bloco de
transformação que estava fisicamente entre `parser.funcoes.declaracao` e
`parser.constantes.declaracao` e ainda não tinha âncoras. Essa maquinaria
converte templates e solicitações registradas durante o parsing em declarações
AST concretas anexadas ao `Program`.

| Âncora | Responsabilidade |
|---|---|
| `parser.genericos.identidade-especializacao` | Chave textual determinística de tipo (`generic_type_key`) e nomes monomórficos de função/leque (`__gen_*`). Só gera identidade; não valida tipos. |
| `parser.genericos.leques-template` | Materializa um `EnumDecl` concreto a partir de um template de leque + argumentos de tipo (aridade, substituição de cargas, nome monomórfico). |
| `parser.genericos.substituicao-ast` | Substituição recursiva parâmetro-de-tipo → tipo concreto por `Type`/`Expr`/`AssignTarget`/`Block`/`ElseBlock`/`IfStmt`/`Stmt`, preservando spans. Uma operação única distribuída pelos `substitute_*`. |
| `parser.callbacks.substituicao-estatica` | Reescrita de chamadas cujo callee é um parâmetro-função por chamadas diretas à função concreta ligada, percorrendo toda a AST do corpo. |
| `parser.callbacks.instanciacao-estatica` | Especialização de callback estático: localiza a função concreta, valida posição/assinatura, exige callback para todo parâmetro-função, gera `__fnparam_*`, remove os parâmetros-função e deduplica. |
| `parser.genericos.funcoes-instanciacao` | Materializa `FunctionDecl` concretos das funções genéricas solicitadas (aridade, nome monomórfico, deduplicação, substituição de parâmetros/retorno/corpo). |
| `parser.genericos.leques-instanciacao` | Percorre as solicitações de leque genérico, deduplica e delega a criação da declaração especializada. |

**Distinção genéricos × callbacks (§3):** os domínios são deliberadamente
separados. `genericos` cobre substituição de **parâmetros de tipo** (produz tipos
concretos); `callbacks` cobre especialização de **parâmetros-função estáticos**
(reescreve chamadas indiretas em diretas e remove o parâmetro-função). Rotular a
segunda como “substituição de genéricos” seria incorreto.

**Pontos de integração já cobertos por âncoras da Onda 4 (não re-ancorados, §5):**

- `parser.programa.estrutura` — registra templates (função genérica, função com
  parâmetro-função, leque genérico) durante o laço de itens de topo e, ao final,
  **invoca** `instantiate_generic_enums`/`instantiate_generic_functions`/
  `instantiate_function_param_functions` e anexa as declarações resultantes (e as
  funções pendentes) ao `Program`. É aqui que a materialização entra no programa.
- `parser.tipos.gramatica` — lê aplicações genéricas de tipo e registra
  solicitações de leque genérico.
- `parser.expressoes.postfix` — lê chamadas genéricas explícitas e chamadas com
  callback estático, registrando as solicitações correspondentes.
- `parser.funcoes.declaracao` — declara os parâmetros de tipo genéricos.
- `parser.closures.expressao` — registra funções sintéticas pendentes.

Essas regiões foram **preservadas intactas**; a 5B não moveu fronteiras nem criou
âncoras aninhadas/sobrepostas.

**Helpers deliberadamente não ancorados (§7/§8):** o estado do `Parser`
(`generic_templates`, `generic_instantiations`, `enum_generic_templates`,
`enum_generic_instantiations`, `function_param_templates`,
`function_param_instantiations`, `pending_functions` e os registros
`GenericInstantiation`/`EnumGenericInstantiation`/`FunctionParamInstantiation`/
`FunctionParamBinding`) vive na struct junto a estado sintático não relacionado —
ancorá-lo exigiria englobar campos alheios, então fica como plumbing. Os helpers
`has_function_param`, `function_type_for_decl` e `function_param_specialization_name`
(entre `leques-template` e `substituicao-ast`) e `function_decl_by_name` (dobrado
em `callbacks.instanciacao-estatica`) são infraestrutura local, sem âncora
própria.

## Onda 5C — lowering AST → IR (concluída)

`src/ir.rs` **integralmente revisado** (linha a linha) e cartografado na
transformação da AST semanticamente válida para a IR estruturada. O modelo de
dados já estava coberto por `ir.modelo.representacao` (preservado, não movido); a
5C acrescenta a maquinaria de lowering, conversão de tipos e renderização.

| Âncora | Responsabilidade |
|---|---|
| `ir.lowering.programa-orquestracao` | Entrada `lower_program`: cria o contexto, despacha constantes/funções e monta `ProgramIR`. |
| `ir.lowering.contexto-declaracoes` | 1ª metade de `from_program`: coleta aliases, structs/campos/offsets, variantes de leque, assinaturas e tipos de constantes do programa. |
| `ir.lowering.assinaturas-intrinsecos` | 2ª metade de `from_program`: catálogo centralizado de assinaturas das intrínsecas embutidas/internas + montagem do contexto. |
| `ir.lowering.funcoes-blocos` | `FunctionLowerer`: parâmetros, bloco de entrada, locais, `FunctionIR`/`BlockIR`; inclui os resolvedores de método de `impl`. |
| `ir.lowering.comandos-controle` | Lowering de `Stmt` → `InstructionIR` (declaração, stores, retorno, `falar`, asm, `talvez`/`sempre que` estruturados com destinos de laço). |
| `ir.lowering.expressoes-valores` | Grande despachante `lower_value` → `TypedValueIR` (literais, chamadas/métodos, intrínsecas de lista/mapa, leques, campos/offsets, cast, `peso`/`alinhamento`). |
| `ir.lowering.bindings-escopos` | Normalização de nomes em slots `%nome#N`, pilha de escopos, coleta de `LocalIR` e geração de rótulos. |
| `ir.lowering.constantes` | `lower_const`: abaixa o inicializador e o tipo de uma constante global em `ConstIR`. |
| `ir.renderizacao.textual` | `render_function`/`render_block`/`render_instruction`/`render_value` — forma textual auditável da IR. |
| `ir.tipos.conversao-ast` | `TypeIR::from_ast_*`: conversão mecânica `Type` → `TypeIR` (aliases, redução de leques, arrays/ponteiros/structs, recusa de função/genérico). |

**Separação de responsabilidades (§3):** o **modelo** (`ir.modelo.representacao`)
define as estruturas; o **lowering** (`ir.lowering.*`) transforma AST → IR; a
**validação** (`ir.validacao.invariantes`, em `src/ir_validate.rs`, intocada)
confere invariantes; a **renderização** (`ir.renderizacao.textual`) produz texto;
o **CFG** (`src/cfg_ir.rs`, Onda 5D) é que divide o fluxo em blocos básicos. Nesta
camada `if`/`while` ainda são estruturas aninhadas, `break`/`continue` carregam
destinos simbólicos e não há SSA, terminadores nem blocos básicos.

**Decisão sobre `LoweringContext::from_program` (§7):** a função (~1025 linhas)
foi dividida em **duas regiões adjacentes** (não aninhadas) no seio da própria
função — `contexto-declaracoes` (fatos derivados do programa) e
`assinaturas-intrinsecos` (catálogo embutido) — porque a segunda metade é um
catálogo repetitivo e volumoso com contrato distinto. Não se criou uma região por
família de intrínseca nem por comentário de fase (§6.3): o catálogo é uma única
responsabilidade conceitual.

**Granularidade de `lower_value` (§6.6):** permanece **uma região ampla**
(`expressoes-valores`), pois é um despachante único e fortemente interligado;
não foi refatorado para gerar regiões menores.

**Helpers deliberadamente não ancorados (§11):** a entrada pública
`render_program` (fica junto a `lower_program`, fisicamente separada do restante
da renderização; é um wrapper fino que delega às funções ancoradas em
`ir.renderizacao.textual`); `resolve_type`, `resolve_struct_name_from_type` e
`pointer_to_bombom_array_size` (helpers de resolução consumidos pelo lowering);
os predicados/nomeação de `TypeIR`, e os `impl` de `ScalarTypeIR`/`UnaryOpIR`/
`BinaryOpIR` (métodos de modelo). Nenhum tem responsabilidade consultável própria.

**Limitações registradas (não corrigidas):** leques abaixam para `bombom`
(discriminante/handle) sem tipo nominal na IR; structs dependem de nome auxiliar e
offsets; ponteiros carregam apenas volatilidade. São representações efetivas da
fase, não bugs; nenhuma mensagem de erro foi alterada.

## Onda 5D — lowering IR → CFG (concluída)

`src/cfg_ir.rs` **integralmente revisado** (linha a linha) e cartografado na
transformação da IR estruturada em blocos básicos com terminadores explícitos. O
modelo de dados já estava coberto por `cfg.modelo.representacao` (preservado, não
movido) e as duas responsabilidades especializadas de lógica por
`cfg.logica.curto-circuito` e `cfg.logica.slot-logico` (históricas, preservadas
sem duplicação); a 5D acrescenta a maquinaria de lowering, construção de blocos e
renderização em torno delas.

| Âncora | Responsabilidade |
|---|---|
| `cfg.lowering.programa-orquestracao` | Entrada `lower_program`: constantes, funções e `ProgramCfgIR`. |
| `cfg.lowering.funcoes-blocos` | `lower_function`: bloco `entry`, um terminador por bloco, `dead_N`, retorno implícito só para `nulo`, `FunctionCfgIR`. |
| `cfg.lowering.instrucoes-controle` | `lower_instruction`: stores, retorno e achatamento de `if`/`while` em `Branch`/`Jump`/join com pilhas de `break`/`continue`. |
| `cfg.lowering.valores-temporarios` | `lower_value_operand`/`lower_expr_stmt`: linearização de valores em operandos e `TempIR`. |
| `cfg.lowering.memoria-indireta` | Acesso/escrita de campos e índices por endereço → `DerefLoad`/`DerefStore`. |
| `cfg.lowering.construcao-blocos` | `fresh_block`/`next_label`/`next_temp` e `BlockBuilder::new`/`is_terminated` (bloco aberto × terminado). |
| `cfg.lowering.constantes` | `lower_constant_value`: valor de constante global → operando CFG. |
| `cfg.renderizacao.programa` | `render_program`: forma textual da CFG ao nível de programa/função/bloco. |
| `cfg.renderizacao.componentes` | `render_instruction`/`render_terminator`/`render_operand`/`render_temp` + operadores + `line`. |

**Regiões `cfg.logica.*` (§5/§8/§9, preservadas):** `cfg.logica.curto-circuito`
já cobre a avaliação do operando esquerdo, os blocos `logic_rhs`/`logic_short`/
`logic_join`, a direção distinta de `e`/`ou`, a materialização do resultado num
slot e os jumps ao join; `cfg.logica.slot-logico` cobre a criação de `%logic#N`
como `LocalIR` mutável para o merge do curto-circuito (não é `phi`, não é SSA). As
novas regiões de lowering **param antes** de `cfg.logica.curto-circuito` e
**retomam depois** de `cfg.logica.slot-logico`; nenhuma fronteira histórica foi
movida, nem se criou região aninhada/sobreposta.

**Separação de responsabilidades (§3):** o **modelo** (`cfg.modelo.representacao`)
define as estruturas; o **lowering** (`cfg.lowering.*`) achata o controle
estruturado em blocos básicos; a **lógica** (`cfg.logica.*`) trata o curto-circuito
e seu slot; a **validação** (`cfg.validacao.invariantes`, em
`src/cfg_ir_validate.rs`, intocada) confere invariantes; a **renderização**
(`cfg.renderizacao.*`) produz texto; a **seleção** (`src/instr_select.rs`, Onda 5E)
é a próxima camada. Os temporários `TempIR` (`%tN`) são resultados intermediários
do CFG — não são slots locais nem registradores físicos; não há SSA.

**Decisões de granularidade:** `lower_instruction` (dispatcher de controle) e
`lower_value_operand` (dispatcher de valores) ficam cada um como **uma região
ampla**, não fragmentada por variante. Campos e índices formam uma região própria
contígua (`memoria-indireta`), conceitualmente distinta do dispatcher de valores.
A renderização usa **duas regiões**: `render_program` (fisicamente junto à
orquestração) e os componentes finais — a alternativa de deixar `render_program`
sem âncora foi descartada por ser função pública substancial.

**Helpers deliberadamente não ancorados (§6.7/§12):** os structs de estado
`FunctionLowerer`/`BlockBuilder` (preâmbulo de plumbing) e os helpers de argumento
`lower_falar_operand`/`lower_falar_args`/`lower_call_operand` (entre
`memoria-indireta` e `cfg.logica.curto-circuito`; a distinção de chamada com/sem
retorno já é descrita por `valores-temporarios`). Nenhum tem responsabilidade
consultável própria.

**Limitações registradas (não corrigidas):** inline asm (`sussurro`) ainda não é
abaixado para CFG (erro local); chamada `nulo` usada como valor é rejeitada;
constante global composta é recusada; campos/índices têm limites de tipo escalar e
base `[bombom; N]`; função não-`nulo` sem terminador e `break`/`continue` fora de
laço produzem erro. `break`/`continue` usam `Branch` com condição constante e
bloco de continuação sintético — registrado como observação, sem alteração.

## Onda 5E — CFG → seleção → máquina (concluída)

`src/instr_select.rs` e `src/abstract_machine.rs` **integralmente revisados**
(linha a linha), fechando a cadeia de lowerings. Os modelos já estavam cobertos por
`select.modelo.representacao`/`machine.modelo.representacao` (preservados) e os
validadores por `select.validacao.invariantes`/`machine.validacao.invariantes` (em
arquivos próprios, intocados); a 5E acrescenta o lowering e a renderização de cada
camada.

**Seleção (`select`, 4 regiões):**

| Âncora | Responsabilidade |
|---|---|
| `select.lowering.programa-blocos` | `lower_program`: `ProgramCfgIR` → `SelectedProgram` (globais, funções, `slot_types`, blocos, terminadores). |
| `select.lowering.instrucoes` | `select_instruction`: `InstructionCfgIR` → `SelectedInstr` (enum-a-enum), com `lower_falar_args`. |
| `select.renderizacao.programa` | `render_program` da seleção. |
| `select.renderizacao.componentes` | `render_instr`/`render_term`/`render_operand`/`render_temp`. |

**Máquina (`machine`, 7 regiões):**

| Âncora | Responsabilidade |
|---|---|
| `machine.lowering.programa-blocos` | `lower_program`: `SelectedProgram` → `MachineProgram`. |
| `machine.lowering.instrucoes-pilha` | `lower_instr`: `SelectedInstr` → operações de pilha (carregar → operar → `StoreSlot %tN`), incl. `falar`. |
| `machine.lowering.terminadores` | `lower_term`: carga de condição/retorno antes de `BrTrue`/`Ret`. |
| `machine.lowering.operandos-slots` | `emit_load`/`temp_name`: `OperandIR` → carga na pilha e nome canônico `%tN`. |
| `machine.renderizacao.programa` | `render_program` da máquina. |
| `machine.renderizacao.apresentacao` | `clean_slot_display`/`is_render_temp`/`block_role_annotation` (apresentação humana). |
| `machine.renderizacao.componentes` | `render_instr`/`render_term`/comentários de fluxo/`render_operand`. |

**A seleção é abstrata e independente de ISA (§4/§6.4):** `select_instruction` é
essencialmente uma transformação enum-a-enum que preserva `OperandIR`, `TempIR`,
tipos e labels e converte terminadores diretamente; **não** escolhe instruções de
CPU, **não** aloca registradores, **não** define ABI. A máquina é uma **VM abstrata
de pilha**: operandos são empilhados, operações consomem a pilha, resultados vão
para slots `%tN` — que **não** são registradores físicos; não há SSA nem ABI de
hardware nesta camada. CFG, seleção, máquina, validação, interpretador e backend
permanecem distintos.

**Decisões de granularidade:** `select_instruction` e `lower_instr` ficam cada um
como **região ampla** (não fragmentada por variante). `lower_falar_args` (select) e
`lower_falar_arg` (machine) foram **incluídos** nas respectivas regiões de
instruções (contíguos, sem região própria). A renderização usa **duas regiões** em
`select` (programa + componentes) e **três** em `machine` (programa + apresentação
+ componentes), pois a apresentação humana — limpeza de nomes e anotação heurística
de papéis de bloco por prefixo de label — é responsabilidade distinta do lowering.

**Auditorias registradas (não corrigidas, §6):**

- **`is_freestanding`:** `SelectedProgram` preserva o campo, mas `MachineProgram`
  **não o possui** e `lower_program` da máquina o descarta na fronteira
  seleção→máquina. Limitação da fronteira atual — campo não adicionado.
- **`slot_types` × `%tN`:** o doc de `MachineFunction` afirma que os temporários
  `%tN` são registrados em `slot_types`, mas o lowering apenas **copia** o
  `slot_types` da seleção (só parâmetros+locais); os `%tN` são descobertos por
  `StoreSlot` e reconhecidos pelo validador via padrão nominal (`is_temp_slot`).
  Inconsistência documental registrada — `///` não corrigido neste PR.
- **Spans sintéticos:** os erros de `select_instruction` para invariantes já
  resolvidos (`Deref`, `LogicalAnd`/`LogicalOr`, call com retorno sem destino) usam
  um span fixo `Position::new(1, 1)`. Limitação diagnóstica registrada — span não
  alterado.

**Helpers deliberadamente não ancorados:** nenhum de peso — `lower_falar_args`/
`lower_falar_arg` foram dobrados nas regiões de instruções; `line` está incluído nos
componentes de renderização. Não há plumbing isolado relevante nesta onda.

## Onda 6+ — execução, backends, orquestração (adiadas)

Inventariados; revisão atual `estrutural` (exceto frontend e toda a cadeia de
lowerings, agora integrais).

| Arquivo | Camada | Propósito (do módulo-doc/estrutura) | Complexidade | Âncoras atuais | Onda-alvo |
|---|---|---|---|---|---|
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

## Cobertura acumulada (após Onda 5E)

| Métrica | Valor |
|---|---:|
| Arquivos de produção em `src/` (excl. gerados e fixtures) | 30 |
| Arquivos com responsabilidade ancorada | 26 |
| Arquivos apenas inventariados (estrutural) | 4 |
| Regiões antes da Onda 5E | 89 |
| Regiões adicionadas na Onda 5E | 11 |
| Regiões no catálogo | 100 |
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
| lexer | 2 | espaços-comentários, tokenização (Onda 4) |
| parser | 22 | Onda 4 (15): núcleo, programa, tipos, declarações, encaixe, resultado, closures, funções, constantes, comandos, for-each, precedência, primárias, postfix, interpolação; Onda 5B (7): identidade-especialização, leques-template, substituição-ast, callbacks (substituição/instanciação estática), funções-instanciação, leques-instanciação |
| ast | 5 | programa, tipos, comandos, expressões, serialização |
| ir | 12 | modelo + validador (Onda 3); Onda 5C (10): programa-orquestração, contexto-declarações, assinaturas-intrínsecos, funções-blocos, comandos-controle, expressões-valores, bindings-escopos, constantes, renderização textual, conversão de tipos AST→IR |
| cfg | 13 | modelo + validador + `cfg.logica.*` (históricas); Onda 5D (9): programa-orquestração, funções-blocos, instruções-controle, valores-temporários, memória-indireta, construção-blocos, constantes, renderização programa/componentes |
| select | 6 | modelo + validador; Onda 5E (4): programa-blocos, instruções, renderização programa/componentes |
| machine | 9 | modelo + validador; Onda 5E (7): programa-blocos, instruções-pilha, terminadores, operandos-slots, renderização programa/apresentação/componentes |
| backend-text | 1 | validador |
| semantic | 10 | importações, sistema de tipos, escopos, duas-passagens, tratos, funções, comandos, fluxo, expressões, chamadas (Onda 5A) |
| trama | 10 | normalização, jsonl, marco, catálogos e consultas doc/código, manifesto, ledger, projeções |
| **total** | **100** | |

Pendentes de cartografia: interpreter/backend-text/backend-s/runtime (Onda 6),
cli/editor/boot (Onda 7), tests/apps (Ondas 8/9, após ampliar raízes). Nota:
`src/backend_text.rs` tem hoje apenas o validador ancorado
(`backend-text.validacao.invariantes`); o lowering do backend textual segue sem
cartografia até a Onda 6.

## Próximo ponto de retomada

**Onda 6 — execução, backends e runtime.** Com toda a cadeia de lowerings
cartografada (AST → IR → CFG → seleção → máquina), a próxima onda trata a execução
e a emissão: o **interpretador** (`src/interpreter.rs` — executa a máquina validada:
valores de runtime, frames, intrínsecas, coleções), o **backend textual**
(`src/backend_text.rs`) e o **backend `.s`** (`src/backend_s.rs` — emissão nativa,
ABI SysV, toolchain). O **runtime** (`runtime/pinker_rt/src/lib.rs`) está **fora da
raiz atual do scanner** e exigirá uma decisão própria de ampliação de raízes — não
ampliar o scanner antes disso. Não modificar a cadeia de lowerings já concluída
(`instr_select.rs`/`abstract_machine.rs` na 5E; `cfg_ir.rs` na 5D; `ir.rs` na 5C)
nem os validadores.
