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
útil sozinha. As **Ondas 0–6D** já estavam na `main`, fechando a cadeia de
lowerings AST → IR → CFG → seleção → máquina, a execução hospedada, os dois
backends (textual e `.s`/ABI nativa) e a ampliação controlada das raízes do
scanner. A **Onda 6** foi decomposta em entregas independentes: 6A
(`src/interpreter.rs`), 6B (`src/backend_text.rs`), 6C (`src/backend_s.rs`), 6D
(raízes controladas do scanner) e 6E (`runtime/pinker_rt/src/lib.rs`). A Onda
6E concluiu a **Onda 6 inteira**: o runtime nativo recebeu 15 regiões próprias
na camada `runtime`, cobrindo as 99 funções `extern "C"` diretas mais os 8
wrappers gerados pela macro `formatar_wrappers!` (`pinker_formatar_verso_1..8`)
— 107 símbolos de ABI exportados no total — e os helpers/consts/structs
internos de `runtime/pinker_rt/src/lib.rs` (produção; `#[cfg(test)] mod tests`
fica fora, por decisão explícita da onda). A **Onda 7** concluiu a cartografia
da produção de `src/`: as três superfícies operacionais restantes —
`src/main.rs` (CLI, camada `cli`), `src/editor_tui.rs` (editor TUI, camada
`editor`) e `src/boot.rs` (fronteiras freestanding, camada `boot`) — receberam
20 regiões novas. As **Ondas 8A–8J** ativaram `tests/` como terceira raiz
oficial e cartografaram as evidências do frontend, da semântica, do pipeline,
da execução interpretada, dos backends textual e nativo, da toolchain externa
e dos testes internos do runtime. A convergência cartográfica da Onda 8 foi
aceita com **386 regiões**, das quais **203** estão na camada `evidencia` e
**15** na camada `runtime`. A primeira cápsula operacional/documental da Trama
está completa e acrescenta seis regiões em `tests/nav_catalog_tests.rs`; a
segunda cápsula operacional/documental da Trama está completa e acrescenta seis
regiões em `tests/doc_catalog_tests.rs`; a terceira cápsula
operacional/documental da Trama está completa e acrescenta oito regiões em
`tests/trama_query_tests.rs`. Estado atual explícito: Onda 8A–8J;
catálogo atual = 406; evidencia atual = 223; runtime atual = 15;
`onda_8_complete = true`; `trama_complete = false`.
`apps/` permanece reservada à Onda 9.

A cápsula `pink agente` Onda A acrescenta o núcleo local sob
`development.agent.*`, três regiões de evidência da ferramenta e onze regiões
marker-only nas suítes `trama_ci_tests` e `trama_template_tests`. A Trama
permanece incompleta; os próximos alvos são `trama_manifest_tests` e
`trama_sync_tests`.

## Contrato do scanner

O scanner de `pink nav` indexa hoje um **conjunto explícito de raízes
controladas** do repositório (`official_scan_roots()` em `src/nav.rs`, região
`trama.codigo.raizes`): `src/`, `runtime/pinker_rt/src/` e `tests/`, todas obrigatórias
no fluxo oficial (`pink nav sincronizar`/`verificar`). Cada raiz é validada
antes de qualquer leitura — ausência, caminho que não é diretório ou link
simbólico falham com `E-NAV-SCAN` antes de qualquer escrita do catálogo, sem
gerar índice parcial. Os caminhos registrados em `file` são **repo-relativos**
(`src/nav.rs`, `runtime/pinker_rt/src/lib.rs`), com `/` como separador, nunca
absolutos e nunca contendo `..`. Links simbólicos não são seguidos — nem de
diretório (evita ciclos e fuga da raiz) nem de arquivo — e uma raiz oficial que
seja, ela mesma, um link simbólico é recusada. A extensão indexada nas três
raízes é `.rs`. Apenas `apps/` permanece **desativada**: reúne fontes `.pink`,
que exigem uma política de marcadores própria antes de entrar no scanner, e
fica para a Onda 9. A segurança das fixtures em `tests/` vem do reconhecimento
lexical de comentários reais, não da desativação dessa raiz.

**Estratégia escolhida: B** — `CodeIndex::scan` permanece como wrapper fino de
raiz única para fixtures/testes (sem prefixo fabricado; o caminho é relativo à
raiz recebida), enquanto a produção usa a API multi-raiz
(`CodeIndex::scan_repo` → `official_scan_roots()` → `scan_roots` →
`collect_source_files` → `scan_file`), única fonte da política de raízes. A
chave de região continua global: nenhuma raiz vira namespace, e uma mesma
chave repetida em raízes distintas é reportada como `DuplicateKey` com os dois
caminhos.

## Onda 8A — raiz de evidências e reconhecimento lexical

`tests/` passa a ser a terceira raiz oficial obrigatória, indexando apenas
arquivos `.rs`, ao lado de `src/` e `runtime/pinker_rt/src/`. O scanner mantém
estado léxico mínimo e só reconhece comentários Pinker reais quando `//` é o
primeiro token em contexto de código: strings normais, byte strings, raw
strings e comentários de bloco (inclusive aninhados) não contam. Assim,
fixtures continuam seguras mesmo quando contêm textos que se parecem com
marcadores. `marker_comment`, `raw_string_start` e `char_literal_len` são
helpers de suporte do scanner e permanecem sem região própria nesta onda: a
decisão evita criar região só para implementação auxiliar; o comportamento é
coberto por testes adversariais e a responsabilidade semântica continua no
scanner/catalogação. Nenhuma suíte recebeu região na Onda 8A; ao fim dela o
catálogo seguia com 183 regiões, e o próximo ponto era a Onda 8B.

## Convenção de chaves

`<camada>.<domínio>.<responsabilidade>` — ver `docs/development/README.md` e o
contrato em `src/nav.rs`. Chaves estáveis a movimentos de linha/arquivo; sem
`phase` quando não há proveniência canônica confirmada.

## Cápsula `pink agente` — Onda A

O núcleo em `src/agent.rs` separa spec, confinamento de caminhos, artefatos,
execução de processos e ciclo de vida. Os testes `agent_cli_tests`,
`agent_runner_tests` e `agent_limits_tests` constituem três regiões de evidência.
Nas suítes Trama, a divisão física confirmou sete regiões de CI — inclusive
`has_b64` como suporte próprio sem mover o helper — e quatro regiões do template.
Somente linhas `@pinker-nav` foram adicionadas às duas suítes.

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

## Onda 6A — interpretador da máquina abstrata (concluída)

`src/interpreter.rs` foi **integralmente revisado** (linha a linha). A cartografia
desta onda torna navegável o runtime hospedado do interpretador: entrada de um
`MachineProgram` já validado, frames, slots, pilha, memória indireta simulada,
despacho de intrínsecas e diagnóstico. A fronteira conceitual permanece: a
máquina (`abstract_machine`) define a VM de pilha; o validador da máquina verifica
invariantes estáticas; o interpretador executa a máquina dentro do processo Rust;
o backend textual apenas renderiza uma forma intermediária; o backend `.s` emite
assembly/ABI; `runtime/pinker_rt` é crate nativa separada, ainda fora da raiz do
scanner. O estado hospedado daqui não é runtime nativo linkável.

| Âncora | Responsabilidade |
|---|---|
| `interpreter.modelo.valores-estado` | Valores de execução, handles lógicos, estados de IO/listas/mapas/leques/aleatoriedade/arquivos e frames; diferencia handles, slots e endereços simulados de ponteiros nativos. |
| `interpreter.execucao.programa-globais` | `run_program`/`run_program_with_args`, argumentos CLI, chamada de `principal`, `RunOutcome`, globais e memória inicial em `HashMap`. |
| `interpreter.execucao.funcoes-fluxo` | `call_function`: profundidade, frames, aridade, labels, slots, pilha, percurso de blocos, terminadores e propagação de `sair`/erros com trace. |
| `interpreter.execucao.instrucoes-pilha` | `exec_instr`: padrão desempilhar/operar/empilhar ou armazenar, slots/globais/memória simulada, chamadas, casts, aritmética e impressões de `falar`. |
| `interpreter.intrinsecos.acaso` | Geradores pseudoaleatórios hospedados iniciais, com handles próprios do interpretador. |
| `interpreter.intrinsecos.listas` | Bloco contíguo de listas `bombom`/`verso`, handles tipados, índice e mutação. |
| `interpreter.intrinsecos.mapas-verso-bombom` | Primeiro bloco contíguo de mapa `verso -> bombom` e cursores internos. |
| `interpreter.intrinsecos.leques` | Leques hospedados por handle opaco, tag e payload inteiro/textual. |
| `interpreter.intrinsecos.io-arquivo-texto` | Stdin, arquivos por handle, leitura/escrita/truncamento/fechamento, texto, CSV e JSON mínimo, com efeitos reais no host. |
| `interpreter.intrinsecos.tempo-processos-ambiente` | Relógio, processos, argumentos CLI, ambiente, caminhos, `sair`, `afirmar`, `dormir` e filesystem direto. |
| `interpreter.intrinsecos.conversoes-numero-texto` | Conversões `verso_para_bombom` e `bombom_para_verso`, com validação de aridade/tipos. |
| `interpreter.intrinsecos.mapas-tipados` | Famílias tipadas de mapa (`verso↔verso`, `bombom↔bombom`, `bombom↔verso`) com `criar`/`definir`/`obter`/`tem`/`tamanho`/`remover` e cursores internos, mais a remoção residual de `mapa<verso,bombom>`. |
| `interpreter.hospedeiro.servicos-auxiliares` | Helpers de stdin, aleatoriedade, ambiente, formatação, CSV/JSON, tempo UTC e processos (`Command`, pipes, códigos de saída). |
| `interpreter.execucao.valores-tipos` | Busca de função, `pop*`, coerções para `TypeIR`, ponteiros simulados, aritmética, comparação e signedness defensivos. |
| `interpreter.diagnostico.stack-trace` | Erros enriquecidos, classificação, prevenção de trace duplicado, renderização/truncamento de frames e nomes de instruções. |

**Granularidade de `try_call_intrinsic`:** o dispatcher foi dividido por
responsabilidade semântica, não por intrínseca, respeitando a ordem física do
`match` (sem mover braços). O intervalo final foi cartografado em duas regiões
estáveis — `conversoes-numero-texto` (as duas conversões `verso`↔`bombom`) e
`mapas-tipados` (as demais famílias tipadas de mapa e seus cursores) — em vez de
um único intervalo genérico. Dois braços isolados permanecem **sem âncora
própria** por serem membros de famílias já ancoradas, fisicamente separados
delas: `aleatorio_entre` (pertence a `interpreter.intrinsecos.acaso`) e
`lista_bombom_inserir` (pertence a `interpreter.intrinsecos.listas`); ancorá-los
seria a anti-prática de uma região por intrínseca, e mover código está fora do
escopo. O ramo `_ => NotIntrinsic` de encerramento do dispatcher é plumbing
trivial. Não há `interpreter.intrinsecos.tudo` nem âncora por intrínseca.

**Decisão para `exec_instr`:** uma única região cobre as variantes de
`MachineInstr`, pois todas seguem a mesma disciplina de pilha/slots/memória e
fragmentá-las criaria granularidade por instrução. `falar` aparece aqui como
instruções explícitas (`Print*`), não como intrínseca.

**Helpers de pilha e conversão:** `pop_args`, `pop`, `pop_numeric`, `pop_bool`,
`pop_str`, coerções, casts, aritmética e comparação ficam juntos em
`interpreter.execucao.valores-tipos`, pois são verificações dinâmicas defensivas,
não o sistema estático de tipos.

**Processos:** os braços de processo aparecem no intervalo
`tempo-processos-ambiente`; os helpers `executar_*`, `pipeline_minimo`,
`capturar_*`, validação de comando e código de saída ficam em
`interpreter.hospedeiro.servicos-auxiliares`, porque não são contíguos ao
dispatcher.

**Auditorias registradas (não corrigidas):**

- **Máquina validada × verificações defensivas:** o validador da máquina deve
  garantir funções, labels, aridade, disciplina de pilha e slots/globais bem
  formados, mas o interpretador ainda verifica defensivamente função inexistente,
  aridade, label inexistente, underflow, tipo errado no topo, pilha inválida em
  retorno, slot/global ausente e handle inválido. A sobreposição foi preservada.
- **Memória simulada:** `build_memory` cria endereços `usize` sequenciais e
  artificiais para globais que possuem valor inicial, armazena células em
  `HashMap<usize, RuntimeValue>` e usa `RuntimeValue::Ptr(usize)`; `DerefLoad` e
  `DerefStore` consultam esse mapa. Tamanho/alinhamento não produzem alocação
  nativa; endereço desconhecido vira erro. Ponteiro nulo não tem semântica nativa
  especial além de falhar se o endereço não existir.
- **Volatilidade:** `DerefLoad`/`DerefStore` recebem `is_volatile`, mas o
  comportamento observável do interpretador é o mesmo para operações voláteis e
  não voláteis; helpers `*_fragil`/normal retornam/armazenam de forma equivalente.
- **Inteiros e overflow:** `Int` (`u64`) e `IntSigned` (`i64`) são preservados;
  divisões e módulos por zero são erro; signedness mista passa por normalização
  defensiva com recusa de casos fora de faixa; aritmética usa operadores Rust
  diretos no modo corrente de compilação, sem política explícita de overflow
  própria do interpretador.
- **`sair`:** a intrínseca marca `exit_requested`, interrompe a execução Pinker e
  devolve `RunOutcome.exit_status`; ela não encerra diretamente o processo Rust.
  O bloco em curso para de progredir quando `exec_instr` retorna e o fluxo propaga
  o status.
- **Profundidade e stack trace:** `MAX_CALL_DEPTH` é 64. O frame é inserido ao
  entrar em `call_function` e removido ao sair; os traces preservam ordem dos
  frames ativos, truncam acima de 10 mantendo 5 de cabeça e 5 de cauda, evitam
  anexação duplicada e aceitam `future_span`, que hoje pode ficar indisponível.
- **Handles:** listas, mapas, cursores, leques, arquivos e aleatoriedade usam
  contadores/estados separados; valores numéricos podem coincidir entre famílias,
  mas os domínios são diferenciados pelo `RuntimeValue`/estado consultado, não por
  ponteiros nativos.
- **Paridade com `pinker_rt`:** a comparação foi limitada e factual. Há famílias
  coincidentes de coleções, texto, arquivo/processo/ambiente e conversões, mas as
  representações são evidentemente diferentes (estado Rust hospedado por handles
  contra runtime nativo linkável). Intrínsecas hospedadas e nativas não tiveram
  paridade completa demonstrada; o runtime não foi cartografado porque o scanner
  continua limitado a `src/`.

**Helpers deliberadamente não ancorados:** os helpers mínimos de memória
`deref_load_normal`/`deref_load_fragil`/`deref_store_normal`/`deref_store_fragil`
foram deixados no entorno diagnóstico/execução por serem wrappers triviais; a
semântica consultável está em `programa-globais`, `instrucoes-pilha` e na auditoria
de volatilidade. `current_function` fica dobrado em valores/tipos como helper
defensivo de busca.

**Ambiguidades/inconsistências registradas:** volatilidade sem efeito observável;
política de overflow não explicitada pelo interpretador; paridade com
`runtime/pinker_rt` não demonstrada nesta onda; scanner ainda não cobre runtime.

## Onda 6B — backend textual (concluída)

`src/backend_text.rs` **integralmente revisado** (linha a linha) e cartografado.
O validador já estava coberto por `backend-text.validacao.invariantes` (em
`src/backend_text_validate.rs`, intocado, preservado); a 6B acrescenta modelo,
os dois caminhos de lowering, o pipeline público e a renderização.

| Âncora | Responsabilidade |
|---|---|
| `backend-text.modelo.representacao` | Structs/enums do backend textual (programa, global, função, bloco, instrução, arg de `falar`, terminador). |
| `backend-text.lowering.cfg-programa` | `lower_program`: lowering direto `ProgramCfgIR` → `BackendTextProgram`. |
| `backend-text.lowering.selecao-programa` | `lower_selected_program`: `SelectedProgram` → `BackendTextProgram` (caminho usado). |
| `backend-text.lowering.instrucoes-selecionadas` | `map_selected_instr` (+ `map_selected_term` dobrado): `SelectedInstr`/`SelectedTerminator` → representação textual. |
| `backend-text.pipeline.emissao` | `emit_program`: select → validate → lower_selected → validate → render. |
| `backend-text.renderizacao.programa` | `render_program`: módulo/modo/globais/funções/blocos em pseudo-assembly. |
| `backend-text.renderizacao.instrucoes` | `render_instruction`: `mov`/`unop`/`binop`/`call`/`falar`. |
| `backend-text.renderizacao.componentes` | `render_terminator`/`render_operand`/`render_temp`/`op_name`/`binop_name`/`line`. |

**Separação de responsabilidades:** o **modelo** (`modelo.representacao`) define as
estruturas; o **lowering** (`lowering.*`) constrói a representação a partir de CFG
ou de seleção; o **pipeline** (`pipeline.emissao`) encadeia seleção, validação e
renderização; a **validação** (`backend-text.validacao.invariantes`, intocada)
verifica invariantes; a **renderização** (`renderizacao.*`) serializa. O backend
textual produz pseudo-assembly **validável**, não código nativo — o backend `.s`
(Onda 6C) é a emissão nativa/ABI.

**Decisões de granularidade:** `map_selected_instr` fica **uma região ampla** (não
por variante de `SelectedInstr`); `map_selected_term`, trivial e adjacente, foi
**dobrado** em `lowering.instrucoes-selecionadas`. A renderização usa três regiões
(programa, instruções, componentes). Os dois mapeadores de `falar`
(`map_falar_args_from_cfg`/`_from_selected`) são helpers de lowering fisicamente
situados entre os renderizadores e ficam **sem âncora** (documentado no código).

**Auditorias registradas (não corrigidas):**

- **§7.1 Dois caminhos de lowering:** `lower_program` (direto de `ProgramCfgIR`) e
  `lower_selected_program` (de `SelectedProgram`) constroem o mesmo modelo por
  caminhos distintos. `emit_program`, a CLI `--pseudo-asm` (`src/main.rs`) e
  `src/backend_s.rs` usam **apenas** `lower_selected_program`; `lower_program`
  (direto) e `emit_program` são `pub` **sem chamadores na árvore**. Paridade
  completa entre os dois caminhos não foi demonstrada.
- **§7.2 Informação preservada × descartada:** preservados módulo,
  `is_freestanding`, globais, tipo de retorno, nomes de parâmetros/locais, blocos,
  labels e temporários; **descartados** os tipos de parâmetros/locais
  (`BackendTextFunction` guarda só nomes, ao contrário de `SelectedFunction`), a
  volatilidade e os spans.
- **§7.3 Deref/volatilidade/cast:** em ambos os caminhos, `DerefLoad` vira
  `Unary`/`Deref` **sem** a volatilidade; `DerefStore` e `Cast` são recusados com
  span sintético `Position::new(1,1)`.
- **§7.4 Modelo × validador — operadores unários representáveis mas rejeitados:**
  `BackendTextInstruction::Unary` pode carregar `BitNot` e `Deref`, e o lowering
  os produz: `map_selected_instr` gera `Unary`/`BitNot` para `SelectedInstr::BitNot`
  e `Unary`/`Deref` para `DerefLoad`; o caminho direto (`lower_program`) também
  gera `Unary`/`Deref` para `DerefLoad`. Contudo, `backend_text_validate` só aceita
  `Neg` (operando inteiro) e `Not` (operando lógico) — qualquer outro operador cai
  em `"unop textual com operando inválido"`. Logo, `BitNot` e `Deref` são
  **representáveis e renderizáveis** (`op_name` os serializa como `bitnot`/`deref`),
  mas **não atravessam o pipeline validado** atual. Limitação registrada, não
  corrigida.
- **§7.4 Modelo × validador — tipagem e superfície pública do validador:** o
  validador textual (a) representa a assinatura de cada função **apenas pelo tipo de
  retorno** (mapa `sigs`); (b) trata **todos** os parâmetros e locais como
  `TypeIR::Bombom` (não há tipos no modelo textual); (c) em chamadas, confere que os
  argumentos são operandos resolvíveis e que o tipo de retorno é compatível, mas
  **não verifica aridade nem os tipos declarados** dos parâmetros; (d) reconstrói o
  mapa de temporários **por bloco** (a inferência de temporário é por bloco, não por
  função); (e) **não valida** os argumentos de `BackendTextInstruction::Falar`. Além
  disso, `lower_program`, `lower_selected_program` e `render_program` são `pub`, então
  um consumidor pode construir ou renderizar um `BackendTextProgram` **sem** passar
  por `backend_text_validate::validate_program`. Entre as funções públicas do módulo
  `backend_text`, `emit_program` é a única que encapsula seleção, validação da
  seleção, lowering textual, validação textual e renderização; a CLI executa
  manualmente um fluxo equivalente para `--pseudo-asm` (parte da seleção já validada,
  chama `lower_selected_program`, valida o backend textual e chama `render_program`).
  Fronteira registrada, não corrigida.
- **§7.5 Chamadas/retorno:** `Call{dest}` → `Call` com destino e `ret_type`;
  `CallVoid` → `Call` sem destino e `ret_type` `Nulo`. O renderer tem um ramo
  defensivo `(sem destino, retorno não-nulo)` que imprime `_` — **não** produzido
  pelos mapeadores.
- **§7.6 `falar`:** permanece instrução própria, com mapeadores de CFG e de
  seleção; não é intrínseca do backend.
- **§7.7 Renderização:** strings são emitidas entre aspas **sem escape** de aspas,
  barras ou caracteres de controle.

## Onda 6C — backend `.s` e ABI nativa (concluída)

`src/backend_s.rs` **integralmente revisado** (linha a linha) e cartografado. O
arquivo hospeda **três superfícies conceituais distintas** que não podem ser
descritas como um único backend, uma única representação ou uma única política
de validação. A Onda 6C separa-as em 24 regiões.

**Os três caminhos públicos** — todos recebem `&SelectedProgram`, mas diferem na
representação intermediária e no renderer:

| Função pública | Entrada | Validação | Representação intermediária | Renderer | Consumidor real | Runtime |
|---|---|---|---|---|---|---|
| `emit_from_selected` | `&SelectedProgram` | `validate_supported_subset` | `BackendTextProgram` (via `backend_text::lower_selected_program`) | `render_program` | caminho `.s` textual (`--asm-s`) | não |
| `emit_external_toolchain_subset` | `&SelectedProgram` | embutida em `extract_external_callconv_program` | `ExternalCallConvProgram` | `render_external_x86_64_linux_callconv_impl(.., false)` | `pink build` (toolchain externa) | não (mas emite refs a `pinker_rt` se há `falar`/intrínsecas) |
| `emit_external_toolchain_subset_nativo` | `&SelectedProgram` | embutida em `extract_external_callconv_program` | `ExternalCallConvProgram` | `render_external_x86_64_linux_callconv_impl(.., true)` | `pink build --nativo` | sim (`pinker_rt_iniciar` + `libpinker_rt.a`) |

O caminho textual **não** compartilha representação com os dois caminhos
montáveis: `BackendTextProgram` (metadados `abi.*` como comentários, operações
ainda abstratas) ≠ `ExternalCallConvProgram` (corpos de bloco já textualizados em
`Vec<String>`, frames, offsets, ABI SysV real). O `.s` textual **não** é
assembly GAS montável; os dois caminhos externos sim.

| Âncora | Responsabilidade |
|---|---|
| `backend-s.pipeline.textual-selecionado` | `emit_from_selected`: entrada do `.s` textual; valida subset textual, delega a `backend_text::lower_selected_program` e serializa com `render_program`. |
| `backend-s.pipeline.toolchain-externa` | `emit_external_toolchain_subset`: entrada do montável hospedado; constrói `ExternalCallConvProgram` e renderiza com `runtime_init=false`. |
| `backend-s.pipeline.nativo-runtime` | `emit_external_toolchain_subset_nativo`: entrada do build nativo; mesma representação externa, `runtime_init=true` (chama `pinker_rt_iniciar`). |
| `backend-s.validacao.subset-textual` | `validate_supported_subset`: subset aceito **só** pelo caminho textual (`is_supported_type`), independente das validações do caminho montável. |
| `backend-s.modelo.callconv-externa` | `ExternalCallConvProgram` e componentes; corpos de bloco em `Vec<String>`, papéis de registrador fixos; **não** é `BackendTextProgram`. |
| `backend-s.abi.registradores-argumentos` | `REG_RET`/`ARG_REGS`/`REG_TMP`: papéis fixos SysV; args 7+ pela pilha com padding de 16. |
| `backend-s.lowering.globais-rodata` | Início de `extract_external_callconv_program`: dedup de globais, `bombom`/`logica` literais em `.rodata`, exigência de `principal`. |
| `backend-s.lowering.funcoes-frames` | Validação por função + `slot_offsets` (8 bytes/slot, param→local→temp), `raw_stack`, `stack_size` arredondado a 16. |
| `backend-s.lowering.blocos-terminadores` | Abertura do laço de blocos + seleção de terminador (`Jmp`/`Br`/`Ret`, rodata de `verso` de retorno). |
| `backend-s.lowering.operacoes-memoria` | Corpo: `Mov`, aritmética linear, comparações, `DerefLoad`/`DerefStore` mínimos, `Cast` `u32↔u64`. |
| `backend-s.lowering.chamadas-sysv` | `Call` (ternário via `cmov`, resolução de intrínsecas, ABI SysV com args de pilha) e `CallVoid`. |
| `backend-s.lowering.falar-runtime` | `Falar` (chamadas a `pinker_falar_*`) + catch-all do subset. |
| `backend-s.renderizacao.callconv-programa` | Cabeçalho + `.rodata` (globais e strings length-prefixed) do renderer montável. |
| `backend-s.abi.prologo-parametros` | Prólogo montável: `principal`→`main`, `pushq %rbp`, `pinker_rt_iniciar` condicional, reserva de frame, stores de parâmetros. |
| `backend-s.abi.blocos-terminadores` | Emissão montável de blocos e terminadores (`jmp`/`cmpq $0`+`jne`/`leave`+`ret`). |
| `backend-s.lowering.operacoes-lineares` | `lower_linear_binop` + os seis `lower_cmp_*` (`set*`/`movzbq`, comparações unsigned). |
| `backend-s.lowering.operandos-slots` | `collect_temp_ids`, `load_operand` (imediatos/slots/global RIP/`leaq` de string) e `temp_key`. |
| `backend-s.validacao.labels-tipos` | `validate_external_block_labels` + predicados de tipo do caminho montável. |
| `backend-s.runtime.intrinsecas-por-aridade` | Resolução de intrínsecas de aridade variável (`pinker_formatar_verso_N`, processos). |
| `backend-s.runtime.simbolos-intrinsecas` | Catálogo estático `runtime_intrinsic_symbol` (texto/listas/mapas/arquivo/tempo/acaso/ambiente/leques). |
| `backend-s.dados.strings-rodata` | Dedup de literais `verso`, labels `.Lpinker_verso_N`, `escape_gas_string`. |
| `backend-s.renderizacao.abi-textual-programa` | `render_program`: `.s` textual baseado em `BackendTextProgram` (metadados `abi.*` como comentários, freestanding). |
| `backend-s.renderizacao.abi-textual-instrucoes` | `render_instruction`/`render_terminator` textuais. |
| `backend-s.renderizacao.abi-textual-componentes` | `render_operand`/`render_unary`/`render_binop`/metadados `@arg`/`@ret`. |

**Decisões de granularidade (§8):**

- **`extract_external_callconv_program` (~566 linhas) foi dividido**, não mantido
  amplo: seis regiões contíguas dentro da função (`globais-rodata`,
  `funcoes-frames`, `blocos-terminadores`, `operacoes-memoria`, `chamadas-sysv`,
  `falar-runtime`) seguindo responsabilidades físicas reais.
- **O `match` de `SelectedInstr` não teve braços movidos.** Dados/memória
  (`Mov`/aritmética/comparação/`Deref`/`Cast`) ficam em `operacoes-memoria`;
  `Call`/`CallVoid` em `chamadas-sysv`; `Falar` e o catch-all em `falar-runtime`.
- **Renderers separados por representação:** o montável (`callconv-programa` +
  `abi.prologo-parametros` + `abi.blocos-terminadores`, sobre
  `ExternalCallConvProgram`) e o textual (`abi-textual-*`, sobre
  `BackendTextProgram`) são domínios/regiões distintos.
- **Catálogo de símbolos do runtime** dividido em duas regiões: resolução por
  aridade (`intrinsecas-por-aridade`) e o catálogo estático amplo
  (`simbolos-intrinsecas`) — **sem** uma região por intrínseca nem `runtime.tudo`.
- **Helpers dobrados:** `temp_key` em `operandos-slots`; `is_arity_runtime_intrinsic`
  em `intrinsecas-por-aridade`; `register_rodata_strings_for_operand`/
  `escape_gas_string` em `dados.strings-rodata`; os seis `lower_cmp_*` numa única
  região com `lower_linear_binop`.
- **Helpers deliberadamente não ancorados (plumbing):** `ensure_dest_is_local_or_param`
  (entre o renderer montável e os helpers de lowering), o rabo de montagem de
  `extract_external_callconv_program` (`blocks.push`/`functions.push`/`Ok(..)`) e
  `line`/`err` no fim do arquivo.
- Não se criou região por opcode, por variante de enum nem por intrínseca (§5).

**Auditorias registradas (não corrigidas, §7):**

- **§7.1 Três entradas públicas:** todas recebem `&SelectedProgram`; diferem em
  representação intermediária (`BackendTextProgram` × `ExternalCallConvProgram`),
  validação (subset textual × validações embutidas), renderer e consumidor
  (`--asm-s` × `pink build` × `pink build --nativo`). As duas externas diferem
  **apenas** pelo `runtime_init`.
- **§7.2 `.s` textual × montável:** `render_program(&BackendTextProgram)` emite
  `mov $slot`/`unop`/`binop`, `@arg`/`@ret` e metadados `abi.*` como **comentários**
  `;` — convenções textuais, **não** montáveis diretamente por GAS. O modo
  freestanding embute `boot.entry`/linker script/kernel stub/`.Lpinker_hang`
  **só** nesse renderer. Difere estruturalmente do assembly x86 real de
  `render_external_x86_64_linux_callconv_impl`. Sem equivalência presumida.
- **§7.3 Modelo externo e perda de informação:** `ExternalCallConvProgram`
  **preserva** nome de função, `stack_size`, `slot_offsets`, labels, **os nomes
  dos parâmetros** (`ExternalCallConvFunction.params: Vec<String>`), globais/
  strings de `.rodata` e **o terminador estruturado** de cada bloco
  (`ExternalCallConvBlock.terminator: ExternalCallConvTerminator`, com `Jmp`/`Br`/
  `Ret` mantidos como enum até a renderização). **Descarta** os tipos de
  retorno/parâmetro/local e a associação estruturada nome–tipo, os spans, a
  volatilidade e `is_freestanding`. Quem vira `Vec<String>` é **o corpo de
  instruções** de cada bloco (`ExternalCallConvBlock.body`): os temporários deixam
  de ser instruções estruturadas e passam a aparecer indiretamente em slots e
  linhas de assembly já textualizadas.
- **§7.4 Target/portabilidade:** target único Linux x86-64, sintaxe GAS AT&T,
  ABI SysV, registradores físicos codificados diretamente; sem abstração de target.
- **§7.5 Stack frame:** ordem param→local→temp; **todo slot ocupa 8 bytes** mesmo
  para tipos menores; `raw_stack = (slot_index-1)*8`; frame arredondado a 16 (0
  sem slots); prólogo `pushq %rbp`/`movq %rsp,%rbp`/`subq`; 6 primeiros params dos
  `ARG_REGS`, 7º+ de `16(%rbp)`; epílogo `leave`+`ret`.
- **§7.6 ABI de chamadas:** `%rax` retorno; 6 `ARG_REGS`; args extra empilhados do
  último ao primeiro com padding; `addq` de limpeza após `call`; retorno guardado
  em temporário; `CallVoid` sem store. Conformidade SysV não declarada só pelos
  comentários — o comportamento real foi descrito.
- **§7.7 Aritmética/comparações/signedness:** suportadas `Add`/`Sub`/`Mul`
  (`addq`/`subq`/`imulq`) e as seis comparações (`set*`+`movzbq`). A **aritmética
  não tem distinção explícita de signedness** no backend; `Eq`/`Ne` usam
  `sete`/`setne` e são **neutras** quanto a signedness; **apenas** as comparações
  de ordenação `<`/`>`/`<=`/`>=` usam condições **unsigned**
  (`setb`/`seta`/`setbe`/`setae`). Divisão, módulo, shift e bitwise **não** são
  lowerados (catch-all). Sem política de overflow própria.
- **§7.8 Memória indireta/volatilidade:** `DerefLoad`/`DerefStore` aceitam só
  `bombom`/`u32`/`u64`, **sempre `movq` de 8 bytes** (não estreita `u32`);
  `is_volatile=true` é recusado (só `Pointer{is_volatile:false}`). Offsets de
  campo/array vêm do lowering anterior. Difere do interpretador hospedado.
- **§7.9 Casts:** só `u32→u64` e `u64→u32` a partir de slot tipado (consulta
  `slot_types`), emitindo `movl %eax, %eax`; outros casts recusados.
- **§7.10 Globais/strings:** globais só `bombom`/`logica` literais, dedup por nome,
  `.section .rodata`/`.quad`; strings dedup por valor, layout `[.quad tamanho]
  [.ascii bytes]`, `escape_gas_string` trata `\`/`"`/`\n`/`\t` — **caracteres de
  controle não tratados passam crus**; carga RIP-relative (`leaq`).
- **§7.11 Labels/símbolos:** exige bloco `entry`, recusa label duplicado, valida
  alvos de `jmp`/`br`; prefixo `.L<fn>_<label>`; `principal`→`main`; nomes de
  função/global usados diretamente como símbolos **sem sanitização** — colisões
  possíveis não são checadas.
- **§7.12 Ternário:** `__ternario(cond,a,b)` é tratado especialmente — exige
  aridade 3, avalia ambos os lados eager, usa `%rax`/`%r10`/`%r11`, `cmpq $0` +
  `cmoveq`, **sem `call` real**.
- **§7.13 `falar`:** instrução própria; cada pedaço → `pinker_falar_pedaco_verso`/
  `_logica`/`_bombom`, `pinker_falar_espaco` como separador, `pinker_falar_fim` ao
  final. O caminho hospedado (não nativo) **também** emite referências a esses
  símbolos de `pinker_rt` — não há independência completa de `libpinker_rt.a`.
- **§7.14 Intrínsecas/símbolos do runtime:** resolução por nome
  (`runtime_intrinsic_symbol`) e por aridade
  (`runtime_intrinsic_symbol_por_aridade`); famílias de texto/lista/mapa/arquivo/
  caminho/processo/tempo/ambiente/acaso/leque; função inexistente → erro. **Mapear
  um símbolo não prova paridade** com `runtime/pinker_rt` (não cartografado nesta
  onda).
- **§7.15 Inicialização nativa:** `runtime_init && symbol=="main"` → `call
  pinker_rt_iniciar` logo após `movq %rsp,%rbp` (pilha alinhada a 16), **antes** da
  reserva de frame; `argc`/`argv` pela ABI C do `main`. É a **única** diferença
  observável entre os dois caminhos externos.
- **§7.16 Freestanding:** `is_freestanding`, `FREESTANDING_BOOT_ENTRY_SYMBOL`/
  `_FUNCTION`, linker script e kernel stub textuais e o loop `.Lpinker_hang`
  pertencem **só** ao renderer baseado em `BackendTextProgram`; o
  `ExternalCallConvProgram` **descarta** a intenção freestanding.
- **§7.17 Diagnósticos:** todos os erros usam `PinkerError::BackendTextValidation`
  com span sintético `(1,1)` via `err`; validações espalhadas pelo lowering
  garantem as invariantes que os `.expect` do renderer montável (condição/retorno
  carregáveis) assumem.

## Onda 6D — raízes controladas de código (concluída)

`src/nav.rs` ganhou uma raiz nova de política: `official_scan_roots()` define
as raízes de código controladas do repositório — hoje `src/` e
`runtime/pinker_rt/src/`, ambas **ativas** e ambas **obrigatórias** no fluxo
oficial (`pink nav sincronizar`/`verificar`). A onda cartografa essa política
em uma região nova, `trama.codigo.raizes`, sem invadir `trama.codigo.catalogo`
(orquestração: `scan`/`scan_repo`) nem `trama.codigo.consulta` (leitura do
JSONL versionado, que continua sem revarrer nada).

- **Raízes ativas:** `src/` e `runtime/pinker_rt/src/`, extensão `.rs` em
  ambas. `tests/` e `apps/` seguem **desativadas** — `tests/` por ter fixtures
  com textos parecidos com marcadores dentro de strings (risco de falso
  positivo sem uma política de exclusão própria) e `apps/` por reunir fontes
  `.pink`, que precisam de uma convenção de marcador própria antes de entrar no
  scanner. Ambas ficam para as Ondas 8 e 9.
- **Caminhos:** sempre repo-relativos, com `/`, nunca absolutos, nunca com
  `..`. Nenhum prefixo é fabricado — o caminho nasce de
  `relative_path` da raiz + caminho do arquivo dentro dela.
- **Unicidade global:** a chave de região continua global entre raízes; a
  mesma chave em `src/` e em `runtime/pinker_rt/src/` é reportada como
  `DuplicateKey` com os dois arquivos. Nenhuma raiz vira namespace de chave.
- **Determinismo:** os arquivos de todas as raízes são combinados, ordenados
  por caminho repo-relativo e cada um é varrido no máximo uma vez — a ordem em
  que as raízes são declaradas não altera o JSONL final.
- **Symlinks:** nunca seguidos, nem de diretório (evita ciclos e fuga da
  raiz) nem de arquivo (não é catalogado); uma raiz oficial que seja, ela
  mesma, um link simbólico é recusada.
- **Raiz obrigatória ausente:** `src/` ou `runtime/pinker_rt/src/` ausente,
  inacessível, não-diretório ou recusada por symlink falha com `E-NAV-SCAN`
  **antes** de qualquer escrita do catálogo — sem índice parcial e sem
  sobrescrever o último catálogo válido.
- **Runtime ativado, não cartografado:** `runtime/pinker_rt/src/` já é
  varrida pelo scanner (é uma raiz ativa), mas `runtime/pinker_rt/src/lib.rs`
  **não** recebeu nenhuma âncora `@pinker-nav` nesta onda — nenhuma região do
  catálogo real tem `file` começando por `runtime/`. Cartografar o conteúdo do
  runtime é trabalho da **Onda 6E**, não desta.
- **Catálogo:** 147 → **148** regiões (uma única região nova,
  `trama.codigo.raizes`); nenhuma chave anterior foi removida; camada `trama`
  10 → **11** regiões.

| Raiz | Estado | Extensão | Cartografia |
|---|---|---|---|
| `src/` | ativa | `.rs` | existente |
| `runtime/pinker_rt/src/` | ativa | `.rs` | aguardando 6E |
| `tests/` | desativada | futura `.rs` | Onda 8 |
| `apps/` | desativada | futura `.pink` | Onda 9 |

Esta onda **não** concluiu a Onda 6 inteira: faltava a 6E (cartografia do
conteúdo do runtime nativo), entregue a seguir.

## Onda 6E — cartografia do runtime nativo (concluída)

`runtime/pinker_rt/src/lib.rs` (2096 linhas; produção nas linhas 1–1802,
`#[cfg(test)] mod tests` nas linhas 1804–2096 **fora** desta onda por decisão
explícita) recebeu 15 regiões na camada nova `runtime`, cobrindo as 99 funções
`pub extern "C" fn`/`pub unsafe extern "C" fn` diretas mais os 8 wrappers
`pinker_formatar_verso_1..8` gerados pela macro `formatar_wrappers!` — 107
símbolos de ABI exportados no total — e os helpers, constantes e `struct`s
internos que as sustentam. Só comentários `//
@pinker-nav:*` foram inseridos — nenhuma assinatura, visibilidade, ABI, tipo,
layout, algoritmo, tratamento de erro, import ou dependência mudou; o `git
diff` do arquivo contém somente linhas adicionadas de comentário.

| Chave | Domínio | Faixa (após formatação) | Responsabilidade e limites observáveis |
|---|---|---|---|
| `runtime.inicializacao.bootstrap` | inicializacao | 24–63 | Constantes de layout do alocador (`ALINHAMENTO`, `CABECALHO`) e estado global (`ARGC`/`ARGV` em atômicos) capturado por `pinker_rt_iniciar`; expõe `pinker_rt_argc`/`pinker_rt_argv`/`pinker_rt_versao`. As constantes de alocação ficam fisicamente no preâmbulo, junto ao estado global de inicialização — nota de fronteira honesta preservada no summary. |
| `runtime.memoria.alocador` | memoria | 70–110 | `pinker_alocar`/`pinker_liberar`: alocador manual com cabeçalho de tamanho; `pinker_liberar` confia, sem validar, que o ponteiro veio de `pinker_alocar` e ainda não foi liberado. |
| `runtime.texto.operacoes` | texto | 126–362 | Família de operações sobre o verso length-prefixed; helpers `unsafe` (`verso_bytes`, `verso_str`) leem via `from_raw_parts`/`from_utf8_unchecked` sem validar ponteiro ou UTF-8; cada transformação aloca um novo bloco cujo ownership passa ao chamador. |
| `runtime.conversoes.numero-texto` | conversoes | 369–393 | `pinker_verso_para_bombom` (aborta o processo via `eprintln!`+`process::exit` em texto não numérico) e `pinker_bombom_para_verso`. |
| `runtime.texto.formatacao` | texto | 400–476 | Núcleo de `formatar_verso` e as variantes `pinker_formatar_verso_0..8` geradas pela macro `formatar_wrappers!`; aridade fixa (0 a 8 argumentos), sem variante para aridade maior. |
| `runtime.io.saida` | io | 489–524 | Impressão de `falar` direto em stdout, sem buffer próprio; erro de escrita em `pinker_falar_pedaco_verso` é silenciosamente ignorado (`let _ =`). |
| `runtime.listas.dinamicas` | listas | 541–672 | Lista dinâmica com header fixo e elementos de 8 bytes; contém `erro_fatal`, o helper compartilhado por todos os domínios seguintes que **aborta o processo** — nota de fronteira explícita no summary. |
| `runtime.mapas.dinamicos` | mapas | 690–900 | Mapa com headers paralelos de chaves/valores, busca linear, comparação por conteúdo (verso) ou valor (bombom), cursor de iteração por snapshot. |
| `runtime.leques.variantes` | leques | 918–996 | Leque com carga: header `[tag][n][cap][cargas]`, cadeia composável `criar_0`+`anexar`; verificação de tag antes de ler a carga. |
| `runtime.arquivos.io` | arquivos | 1015–1223 | Tabela de arquivos abertos em **estado global** protegido por `Mutex`/`OnceLock`; toda escrita persiste imediatamente em disco; handle fechado ou inválido aborta via `erro_fatal`. |
| `runtime.caminhos.sistema` | caminhos | 1230–1314 | Consultas e operações de sistema de arquivos sobre caminhos, delegando a `std::fs`/`std::path`. |
| `runtime.tempo.relogio` | tempo | 1321–1360 | Tempo Unix e formatação ISO-8601 UTC usando o mesmo algoritmo civil (Howard Hinnant) do interpretador; sem suporte a fuso horário além de UTC. |
| `runtime.aleatorio.gerador` | aleatorio | 1367–1430 | Geradores em tabela global protegida por `Mutex`, avançados por um LCG idêntico ao do interpretador; **não é criptográfico**. |
| `runtime.ambiente.argumentos` | ambiente | 1448–1587 | Leitura de `argc`/`argv` global e de variáveis de ambiente; busca por chave nomeada (`chave valor` ou `chave=valor`). |
| `runtime.processos.execucao` | processos | 1594–1801 | Execução de subprocessos via `std::process::Command`, aridade fixa (0/1 argumento extra); stdout/stderr decodificados como UTF-8 estrito. |

Fronteiras de ABI observadas: todas as 15 regiões cobrem **exportação ABI**
(`#[no_mangle]` + `extern "C"`) junto dos helpers internos que a sustentam no
mesmo arquivo — a onda não separou "representação de dados" (headers/structs
como `ArquivoAberto`, `EstadoIo`, `EstadoAcaso`) de "operações" (funções
exportadas) em regiões distintas, porque no runtime nativo ambas vivem
fisicamente entrelaçadas por domínio (ex.: `struct ArquivoAberto`/`EstadoIo`
abre `runtime.arquivos.io`, que também contém toda a API pública de arquivo).
Isso é uma decisão de fronteira, não uma afirmação de separação arquitetural
que o código não sustenta.

Limitações honestas confirmadas e refletidas nos summaries: `erro_fatal`
**aborta o processo** (índice fora da faixa, separador/padrão vazio, chave
ausente, cursor esgotado, OOM, aridade incompatível); estado global em
`Mutex`/atômicos (`ARGC`/`ARGV`, `EstadoIo`, `EstadoAcaso`); helpers `unsafe`
que leem via `from_raw_parts` sem validar o ponteiro recebido; toda
transformação de verso aloca um novo bloco cujo ownership passa ao chamador;
várias famílias (formatação, execução de processo) têm aridade fixa e não
aceitam variantes arbitrárias.

- **Testes de cartografia:** `tests/nav_cartography_tests.rs` ganhou
  `camada_runtime_cartografa_o_runtime_nativo`, validando as 15 chaves
  esperadas, a contagem exata de 15 regiões na camada `runtime`, que todas
  apontam para `runtime/pinker_rt/src/lib.rs` (nenhuma para `src/`) e a
  presença dos domínios principais. A asserção obsoleta da Onda 6D ("runtime
  não deveria ter regiões cartografadas") foi removida de
  `camada_trama_separa_catalogo_raizes_e_consulta`, já que deixou de ser
  verdade a partir desta onda.
- **Catálogo:** 148 → **163** regiões (15 novas, todas na camada `runtime`);
  nenhuma chave anterior foi removida; nenhuma duplicada; camada `runtime` de
  0 → **15** regiões.

## Onda 7 — cartografia das superfícies operacionais (concluída)

As três superfícies operacionais restantes em `src/` — CLI, editor TUI e
fronteiras de boot freestanding — receberam 20 regiões novas em três camadas
novas (`cli`, `editor`, `boot`). Só comentários `// @pinker-nav:*` foram
inseridos — nenhuma assinatura, mensagem, flag, condição, formato de saída,
exit code, path ou processo mudou; o `git diff` de cada um dos três arquivos
contém somente linhas adicionadas de comentário.

### `src/main.rs` — camada `cli` (15 regiões)

| Chave | Domínio | Faixa (após formatação) | Responsabilidade e limites observáveis |
|---|---|---|---|
| `cli.config.modelos` | config | 35–161 | Constantes de códigos de saída e limites de paginação; `clamp_limit`/`json_escape`/`json_string_array`; `struct`s de configuração por subcomando e os `enum`s de subcomando (`DocSub`, `NavSub`, `CliCommand`). |
| `cli.ajuda.usage` | ajuda | 168–300 | `usage`/`nav_usage`/`doc_usage`/`build_usage`/`editor_usage`/`repl_usage`: montam texto de ajuda com `format!`; sem side effects. |
| `cli.parsing.subcomandos` | parsing | 307–689 | Parsers de argumentos por subcomando (`parse_build_args`, `parse_editor_args`, `parse_repl_args`, `parse_doc_args`, `parse_nav_args`): reconhecem flags e o argumento posicional, retornando `Result<Config..., String>`. |
| `cli.parsing.roteamento` | parsing | 696–806 | `parse_args`: separa o argv em `flag_args`/`runtime_tail`, despacha para build/editor/repl/doc/nav ou monta `CliCommand::Analyze(Config)`. |
| `cli.execucao.entrada` | execucao | 813–869 | `try_or_exit!`, `main()`, `scan_code` e `run_nav`: ponto de entrada do processo e roteamento de `CliCommand`/`NavSub`. |
| `cli.nav.consulta` | nav | 876–1067 | `load_code_catalog`, `run_nav_mostrar`, `run_nav_buscar`, `run_nav_listar`: leem o catálogo gerado; `run_nav_mostrar` valida marcador/hash da fonte antes de imprimir. Nenhuma das três escreve em disco. |
| `cli.nav.sincronizacao-verificacao` | nav | 1074–1127 | `run_nav_sincronizar` **escreve** `src/navigation.jsonl` via `write_atomic` quando não há divergência; `run_nav_verificar` é **somente leitura** — compara o renderizado com o disco e reporta divergências sem gravar. |
| `cli.doc.consulta` | doc | 1134–1521 | `load_doc_config`, `run_doc`, `scan_docs`, `load_doc_catalog`, `write_atomic` (único mecanismo de escrita atômica desta base — grava `.tmp` e usa `fs::rename`) e as consultas somente-leitura `run_doc_mostrar`/`run_doc_listar`/`run_doc_buscar`/`run_doc_rota`/`print_doc_results_json`. |
| `cli.doc.sincronizacao` | doc | 1528–1602 | `run_doc_sincronizar`: **escreve** o catálogo, o ledger e as projeções documentais quando `verify()` não reporta divergência. |
| `cli.doc.mudancas` | doc | 1609–1699 | `LEDGER_REL`, `write_ledger`, `run_doc_importar`: grava manifestos de mudança; `--check` reporta sem gravar; conteúdo idêntico ao existente é tratado como idempotente, conteúdo diferente falha (`change::immutable_error`). |
| `cli.doc.verificacao` | doc | 1706–1772 | `run_doc_verificar`: **somente leitura** — recomputa catálogo/ledger/projeções em memória e compara com o disco, acumulando divergências sem escrever. |
| `cli.execucao.editor-repl` | execucao | 1779–1798 | `run_editor` (abre `EditorTui::from_path` + `run()`) e `run_repl` (delega a `repl::run_repl()`, não é stub local); ambos `process::exit(1)` em erro. |
| `cli.analise.pipeline` | analise | 1805–2016 | `run_analyze`: conduz parse → imports → semântica → IR/CFG/seleção/máquina/backends conforme as flags do `Config`; `--asm-s` emite texto (não monta/linka); `--run` executa via interpretador. |
| `cli.build.nativo` | build | 2023–2165 | `run_build` (grava `.s` em disco), `locate_pinker_rt_lib` (**localiza**, não constrói, a staticlib pré-buildada), `detect_cc_driver` (**detecta** um driver C disponível) e `link_nativo` (invoca o driver externo para montar/linkar). |
| `cli.modulos.importacao` | modulos | 2172–2431 | `parse_program_from_source` e o resolvedor de imports (`load_module_program`, `load_program_with_imports`, helpers de item importável) — detecção de ciclo, colisão de nome e requalificação de tipos por módulo. |

### `src/editor_tui.rs` — camada `editor` (4 regiões)

`#[cfg(test)] mod tests` (linhas finais do arquivo) **não** foi cartografado
nesta onda — mesma decisão de fronteira da Onda 6E, revisão adiada.

| Chave | Domínio | Faixa (após formatação) | Responsabilidade e limites observáveis |
|---|---|---|---|
| `editor.estado.modelo` | estado | 15–36 | Constantes de exibição (`OUTPUT_LINES`/`EDITOR_LINES`), `struct EditorTui` e `from_path` (lê o arquivo com `source.lines()`, separa em linhas e não armazena terminadores originais nem a presença de newline final). |
| `editor.sessao.comandos` | sessao | 43–179 | `run` (laço leitura-execução), `execute_command` (interpreta `:quit`/`:help`/`:tokens`/`:ast`/`:save`/`:append`/`:set`), `run_tokens_command`/`run_ast_command` (ações Pinker reais — **preview**, não editam AST persistente), `save_file` (grava com `fs::write`, sem escrita atômica, a fonte recomposta; `:save` não preserva byte a byte CRLF nem newline final), `set_line`. |
| `editor.render.saida` | render | 186–225 | `current_source` (junta `lines` com LF, normalizando CRLF e sem restaurar newline final original), `render` (desenha o painel com ANSI), `push_output` (empilha mensagem). |
| `editor.analise.checagem` | analise | 233–240 | `parse_and_check_program`: tokeniza + parseia + roda `semantic::check_program` sobre uma string; usada SOMENTE por `:ast` (via `run_ast_command`) para produzir o `Program` em memória do preview — `:tokens` (`run_tokens_command`) chama `Lexer::tokenize` diretamente e não usa esta função. |

### `src/boot.rs` — camada `boot` (1 região, arquivo inteiro)

| Chave | Domínio | Faixa (após formatação) | Responsabilidade e limites observáveis |
|---|---|---|---|
| `boot.geracao.fronteira-freestanding` | geracao | 5–18 | `FREESTANDING_BOOT_ENTRY_FUNCTION`/`FREESTANDING_BOOT_ENTRY_SYMBOL` (constantes textuais), `freestanding_linker_script` (string literal de script `ld`) e `freestanding_kernel_stub` (string com `call principal` + laço `jmp` para si mesmo). Só produzem strings/constantes de fronteira — nenhuma função executa, aloca, linka, monta ou inicializa hardware/stack/Multiboot/UEFI. |

- **Testes de cartografia:** `tests/nav_cartography_tests.rs` ganhou
  `camada_operacional_cartografa_cli_editor_boot`, validando as 20 chaves
  esperadas, a contagem exata por camada (`cli` 15, `editor` 4, `boot` 1), que
  cada região aponta para o arquivo correto da sua camada sem cruzamento entre
  os três, domínios representativos e uma amostra de chaves anteriores (0–6E)
  que permanece presente e fora de `cli`/`editor`/`boot`.
- **Catálogo:** 163 → **183** regiões (20 novas); nenhuma chave anterior
  removida; nenhuma duplicada; camada `cli` 0 → **15**, `editor` 0 → **4**,
  `boot` 0 → **1**.

## Onda 8B — evidências léxicas e sintáticas

Esta etapa seleciona três arquivos de evidência do frontend —
`tests/common/mod.rs`, `tests/lexer_tests.rs` e `tests/parser_tests.rs` — e
adiciona 19 regiões na camada `evidencia`. São agrupamentos de evidência, não
uma alegação de completude da gramática ou dos contratos do frontend.

### Chaves cartografadas

- `evidencia.frontend.pipeline-basico` — os 3 helpers compartilhados do
  frontend (`tokenize`, `parse`, `parse_and_check`).
- Léxico (25 testes em `tests/lexer_tests.rs`):
  `evidencia.lexico.tokens-e-spans`, `evidencia.lexico.diagnostico`,
  `evidencia.lexico.palavras-controle`, `evidencia.lexico.operadores`,
  `evidencia.lexico.tipos-fixos`, `evidencia.lexico.palavras-de-construcao` e
  `evidencia.lexico.arrays-acessos-e-modificadores`.
- Parser (36 testes em `tests/parser_tests.rs`):
  `evidencia.parser.ast-basica-e-spans`,
  `evidencia.parser.diagnostico-e-limites-literais`,
  `evidencia.parser.controle-de-fluxo`,
  `evidencia.parser.desugaring-para-cada`,
  `evidencia.parser.diretivas-topo-e-asm-inline`,
  `evidencia.parser.tipos-qualificados-e-verso`,
  `evidencia.parser.expressoes-e-precedencia`,
  `evidencia.parser.postfix-cast-deref-e-operadores-tipo`,
  `evidencia.parser.tipos-numericos`,
  `evidencia.parser.aliases-arrays-e-structs` e
  `evidencia.parser.ponteiros-e-colecoes`.

O teste estrutural valida as 19 chaves específicas da Onda 8B por arquivo,
domínio e camada `evidencia`: 25 atributos `#[test]` do lexer e 36 do parser,
cada um pertencendo a exatamente uma região e cada região de lexer/parser com
ao menos um teste. Ele também confirma os três helpers de common dentro da
região e `render_ast` fora dela. Assim, permanece compatível com futuras
regiões de evidência. Ao concluir a Onda 8B, o catálogo tinha 202 regiões; a
cartografia de `tests/semantic_tests.rs` permanecia então reservada à Onda 8C,
concluída na seção correspondente abaixo.

## Arquivos sem candidatos a âncora

Registrados para não desaparecerem da análise; não recebem âncoras.

| Arquivo | Motivo |
|---|---|
| `src/lib.rs` | Apenas declarações de módulos (`pub mod ...`). |
| `src/bin/pinker_fase16x_*.rs` | Binários-fixture minúsculos (3–35 linhas) usados por testes de I/O; sem responsabilidade nomeável. |
| `src/navigation.jsonl` | Catálogo **gerado**; nunca é fonte de âncoras. |

## Onda 8C — evidências semânticas e contratos de tipos

`tests/semantic_tests.rs` foi integralmente cartografado: 340 testes em 34
regiões de evidência (`domain: semantica`, `layer: evidencia`). As chaves são:
`evidencia.semantica.entrada-principal`, `evidencia.semantica.retornos`,
`evidencia.semantica.mutabilidade`, `evidencia.semantica.chamadas`,
`evidencia.semantica.intrinsecas-entrada-ambiente`,
`evidencia.semantica.intrinsecas-caminhos-e-sistema`,
`evidencia.semantica.intrinsecas-argumentos-e-contexto`,
`evidencia.semantica.intrinsecas-arquivos-io`,
`evidencia.semantica.intrinsecas-texto-e-estruturados`,
`evidencia.semantica.intrinsecas-processos`,
`evidencia.semantica.funcoes-sem-retorno`,
`evidencia.semantica.controle-fluxo-e-diagnostico`,
`evidencia.semantica.operadores-logicos-e-bitwise`,
`evidencia.semantica.acesso-campos-e-indexacao`, `evidencia.semantica.casts`,
`evidencia.semantica.peso-e-alinhamento`,
`evidencia.semantica.tipos-numericos-largura-fixa`,
`evidencia.semantica.aliases-arrays-e-ninhos`,
`evidencia.semantica.ponteiros-e-aritmetica`,
`evidencia.semantica.ninhos-diagnostico`,
`evidencia.semantica.aritmetica-modulo-e-literais`,
`evidencia.semantica.escrita-por-indice`, `evidencia.semantica.listas`,
`evidencia.semantica.mapas`, `evidencia.semantica.acaso`,
`evidencia.semantica.imports-por-familia`,
`evidencia.semantica.leques-simples`, `evidencia.semantica.leques-com-carga`,
`evidencia.semantica.encaixe-e-bindings`,
`evidencia.semantica.leques-recursivos-e-multiplas-cargas`,
`evidencia.semantica.genericos`, `evidencia.semantica.tratamento-de-erro`,
`evidencia.semantica.funcoes-locais-e-carinho` e
`evidencia.semantica.tratos-e-impls`.

Os agrupamentos seguem contratos de entrada, retornos, mutabilidade e chamadas;
famílias de intrínsecas; tipos compostos, ponteiros e coleções; leques,
encaixe, genéricos, tratamento de erro, funções locais e tratos/impls. A suíte
combina assertions exatas com assertions parciais. Há exemplos carregados por
`include_str!`, tratados como casos observados. As limitações são explícitas: a
suíte verifica aceitação/rejeição pelo pipeline de frontend (`parse_and_check` =
parse seguido de checagem semântica); parte das rejeições ocorre já no
parse/desugaring e parte no checker; não executa runtime; `is_ok` não prova
comportamento operacional; `contains` não fixa a mensagem inteira; e os
exemplos não provam completude. O catálogo passa de 202 para 236 regiões, com
as 202 anteriores preservadas. A Onda 8 permanece em andamento.

## Onda 8D — evidências do pipeline IR, CFG, seleção e máquina

Sete suítes do pipeline foram integralmente cartografadas em duas trilhas
paralelas (especialistas distintos, worktrees e contas separados, revisão
cruzada), com **134 testes** em **58 regiões** de evidência.

**Trilha A — IR e CFG (M_A = 70 testes, N_A = 25 regiões):**

- `tests/ir_tests.rs` — 25 testes, 7 regiões (`domain: ir`): lowering AST→IR e
  renderização textual/CLI. Chaves: `evidencia.ir.lowering-programa`,
  `evidencia.ir.renderizacao-estruturas-basicas`, `evidencia.ir.renderizacao-cli`,
  `evidencia.ir.lowering-controle-de-laco`, `evidencia.ir.lowering-operacoes-textuais`,
  `evidencia.ir.lowering-tipos-numericos`, `evidencia.ir.lowering-tipos-compostos`.
- `tests/ir_validate_tests.rs` — 7 testes, 4 regiões (`domain: ir`): invariantes de
  IR construída manualmente; aceitação/rejeição observadas. Chaves:
  `evidencia.ir.validacao-aceitacao-basica`, `evidencia.ir.validacao-retorno-e-condicao`,
  `evidencia.ir.validacao-chamadas-e-nulo`, `evidencia.ir.validacao-estrutura-e-diagnostico`.
- `tests/cfg_ir_tests.rs` — 23 testes, 8 regiões (`domain: cfg`): lowering IR→CFG,
  blocos, terminadores, curto-circuito, renderização. Chaves:
  `evidencia.cfg.lowering-e-renderizacao-basica`, `evidencia.cfg.renderizacao-cli`,
  `evidencia.cfg.lowering-lacos`, `evidencia.cfg.lowering-operadores-e-join`,
  `evidencia.cfg.lowering-ponteiros-e-agregados`, `evidencia.cfg.lowering-limite-asm`,
  `evidencia.cfg.lowering-verso`, `evidencia.cfg.lowering-curto-circuito`.
- `tests/cfg_ir_validate_tests.rs` — 15 testes, 6 regiões (`domain: cfg`): invariantes
  de CFG construído manualmente; erros observados. Chaves:
  `evidencia.cfg.validacao-aceitacao-basica`, `evidencia.cfg.validacao-blocos-e-alvos`,
  `evidencia.cfg.validacao-condicao-e-retorno`, `evidencia.cfg.validacao-chamada-e-referencias`,
  `evidencia.cfg.validacao-alcancabilidade-e-renderizacao`, `evidencia.cfg.validacao-diagnostico`.

**Trilha B — seleção e máquina abstrata (M_B = 64 testes, N_B = 33 regiões):**

- `tests/instr_select_tests.rs` — 12 testes, 6 regiões (`domain: select`): seleção
  abstrata sobre CFG → `SelectedInstr`; renderização. Chaves:
  `evidencia.select.blocos-e-terminadores`, `evidencia.select.chamadas-e-operadores`,
  `evidencia.select.renderizacao-cli`, `evidencia.select.rejeicao-call-sem-destino`,
  `evidencia.select.fluxos-de-laco`, `evidencia.select.operadores-bitwise-e-modulo`.
- `tests/abstract_machine_tests.rs` — 23 testes, 11 regiões (`domain: machine`):
  lowering da seleção para máquina de pilha, comparação de representações,
  renderização. Chaves: `evidencia.machine.lowering-blocos-e-terminadores`,
  `evidencia.machine.lowering-chamadas`, `evidencia.machine.lowering-operadores-e-temporarios`,
  `evidencia.machine.renderizacao-cli`, `evidencia.machine.comparacao-representacoes`,
  `evidencia.machine.validacao-programa-e-slots`, `evidencia.machine.lowering-bitwise-e-modulo`,
  `evidencia.machine.renderizacao-slots-e-temporarios`, `evidencia.machine.renderizacao-chamadas`,
  `evidencia.machine.renderizacao-terminadores-e-fluxos`, `evidencia.machine.renderizacao-papeis-de-blocos`.
- `tests/abstract_machine_stack_tests.rs` — 29 testes, 16 regiões (`domain: machine`):
  validação da pilha (underflow, tipos, merges, slots, aridade, retorno) e
  renderização de casos válidos. Chaves: `evidencia.machine.renderizacao-programa-valido`,
  `evidencia.machine.validacao-underflow-operadores`, `evidencia.machine.validacao-chamadas-aridade-e-underflow`,
  `evidencia.machine.validacao-formato-diagnostico`, `evidencia.machine.validacao-branch`,
  `evidencia.machine.renderizacao-branch-valido`, `evidencia.machine.validacao-retorno`,
  `evidencia.machine.renderizacao-retorno-valido`, `evidencia.machine.validacao-pilha-retvoid-e-merges`,
  `evidencia.machine.validacao-slots-existencia`, `evidencia.machine.validacao-slots-tipados`,
  `evidencia.machine.validacao-tipos-operacoes-e-retorno`, `evidencia.machine.validacao-tipos-chamadas`,
  `evidencia.machine.renderizacao-casos-validos`, `evidencia.machine.validacao-programa-invalido`,
  `evidencia.machine.renderizacao-cli-golden`.

As regiões separam **lowering**, **renderização** e **validação** quando são
responsabilidades distintas. As suítes combinam assertions exatas (`assert_eq!`),
parciais (`contains`) e estruturais. Limites explícitos: a IR não constrói CFG; o
CFG não é SSA completa nem seleciona instruções nem representa ISA física; a
seleção é abstrata (não escolhe x86-64, não aloca registradores, não emite bytes);
a máquina abstrata é representação de pilha, não um interpretador — validação
estática não é execução, e renderização textual não é assembly nativo montável. O
catálogo passa de **236 para 294 regiões**, com as 236 anteriores preservadas
(chave, arquivo, domínio, camada, summary e hash). Nenhuma lógica de teste foi
alterada: as sete suítes receberam apenas marcadores `// @pinker-nav:`. A Onda 8
permanece **in-progress**.

## Onda 8E — evidências da execução interpretada

`tests/interpreter_tests.rs` foi cartografada em **46 regiões** de evidência
(`domain: interpreter`, `layer: evidencia`), cobrindo **534 dos 538 testes**
`#[test]` da suíte. A lista congelada de chaves, a contagem de testes por região
e a lista fechada de quatro exclusões relativas à 8E estão no teste estrutural
`onda_8e_cartografa_evidencias_da_execucao_interpretada`
(`tests/nav_cartography_tests.rs`), que também compara o catálogo versionado com
a regeneração canônica para detectar drift de summary, faixa ou hash. As regiões
seguem a ordem física da suíte e agrupam responsabilidades reais, não proximidade:

- **Execução núcleo e semântica de linguagem** — `execucao-nucleo-estado-aritmetica-fluxo`,
  `execucao-operadores-aritmeticos-relacionais-e-sinais`,
  `execucao-chamadas-e-curto-circuito`, `fluxo-controle-lacos-basicos`,
  `execucao-recursao-e-fluxo-interpretador-e-cli`,
  `execucao-operadores-e-fluxo-cli-exemplos`, `execucao-cli-exemplos-basicos`,
  `execucao-funcoes-usuario-tratos-e-genericos`.
- **Texto e verso** — `texto-verso-intrinsecas-consulta-transformacao`,
  `texto-io-por-handle-e-arquivos-releitura`,
  `texto-verso-e-io-textual-por-caminho`,
  `texto-dividir-substituir-juntar-e-buscar`,
  `texto-formatar-verso`, `texto-formatar-cli-exemplos`.
- **Entrada, argumentos e ambiente** — `entrada-argumentos-nomeados-e-flags`,
  `entrada-contexto-ambiente-e-saida`,
  `entrada-argumentos-e-ambiente-cli-exemplos`.
- **Arquivos, caminhos, CSV, JSON e tempo** — `arquivos-introspeccao-caminho-e-diretorios`,
  `arquivos-handle-fechado-e-fluxo-completo`,
  `arquivos-e-ambiente-fallback-cli-exemplos`, `arquivos-csv-serializacao`,
  `arquivos-json-serializacao`, `arquivos-csv-json-cli-exemplos`,
  `tempo-unix-e-formatacao`.
- **Ponteiros e recortes de baixo nível** — `ponteiros-seta-operacional`,
  `ponteiros-boot-freestanding-e-subset-nativo`,
  `ponteiros-array-fixo-e-cast-memoria-cli`,
  `ponteiros-escrita-indice-e-array-fixo`,
  `checagem-cli-modulos-e-recortes-linguagem`.
- **Processos** — `processos-externo-executar`, `processos-argv-explicito`,
  `processos-captura-stdout`, `processos-captura-stderr`,
  `processos-entrada-stdin`, `processos-pipeline`.
- **Coleções, aleatoriedade e leques** — `colecoes-lista-bombom`,
  `colecoes-mapa-verso-bombom`, `colecoes-iteracao-lista-e-mapa`,
  `aleatoriedade-semente`, `leques-trazer-recursos-e-programas-brinquedo`.
- **Diagnósticos de runtime** — `diagnostico-simbolo-inexistente`,
  `diagnostico-runtime-avaliacao-e-chamadas`,
  `diagnostico-runtime-execucao-invalida`, `diagnostico-stack-trace-truncamento`,
  `diagnostico-render-fonte-e-operador-bitnot`,
  `execucao-repl-e-render-erro-fonte`.

**Limites explícitos honestos.** A suíte exercita a execução **interpretada** da
Pinker (helper `run_code`/`run_code_with_args` em processo e o binário `pink`
sobre exemplos `.pink` versionados via `cli_run`/`cli_check`); ela **não** prova
paridade com o backend nativo. Cada summary distingue comportamento **observado**
de inferência: casos verificados por `assert_eq!` são igualdade exata, casos por
`contains` são apenas inclusão de substring (mensagens tratadas como estáveis só
onde há `contains`), e casos por `is_ok`/`unwrap()` comprovam ausência de erro,
não semântica integral. Nenhuma região afirma que uma intrínseca inteira está
validada — apenas o recorte presente (ex.: CSV/JSON planos mínimos, tempo sem
valor absoluto de relógio, aleatoriedade determinística por semente sem garantia
estatística, `juntar_caminho` sem canonicalização). Regiões CLI que enumeram
múltiplas features descrevem a responsabilidade de **harness de execução via
processo**, sem alegar cobertura total de cada feature citada.

Quatro testes de backend/CLI permanecem explicitamente fora das regiões da
Onda 8E:
`cli_build_gera_artefato_s_no_diretorio_padrao`,
`cli_build_com_imports_gera_artefato_no_out_dir`,
`cli_build_sem_arquivo_falha_com_uso` e
`cli_build_falha_semantica_retorna_erro`. Essa exclusão é relativa às 46
regiões 8E: os dois primeiros passam a ter owner na Onda 8G; as duas falhas
continuam sem owner global. O gate estrutural rejeita qualquer quinta exclusão
relativa à 8E.

O catálogo passa de **294 para 340 regiões**, com as 294 anteriores preservadas
(chave, arquivo, domínio, camada, summary e hash). O diff em
`tests/interpreter_tests.rs` é **marker-only** (230 linhas `// @pinker-nav:`
inseridas, zero remoções, nenhuma assertion, fixture, nome de teste ou lógica
alterados). As 46 regiões possuem chaves únicas e cobrem 534 testes exatamente
uma vez; `nav verificar` e a comparação byte a byte com a regeneração canônica
estão verdes. A Onda 8 e a cartografia da Trama permanecem **in-progress**.

## Onda 8F — evidências do backend textual pseudo-asm

`tests/backend_text_tests.rs` recebeu seis regiões de evidência no domínio
`backend-text`, cobrindo exatamente **7 dos 9 testes** da suíte: programa mínimo;
controle de fluxo (`emite_if_else` e `emite_if_sem_else`); chamada direta com
temporário e operação binária; chamada sem retorno e retorno nulo; globais; e o
golden da apresentação pseudo-asm. Em `tests/common/mod.rs`, duas regiões de
contagem zero delimitam exclusivamente `render_backend_text` e
`render_cli_pseudo_asm_output`.

As oito chaves são `evidencia.backend-text.pipeline-helper`,
`evidencia.backend-text.apresentacao-cli-helper`,
`evidencia.backend-text.renderizacao-programa-minimo`,
`evidencia.backend-text.renderizacao-controle-fluxo`,
`evidencia.backend-text.renderizacao-chamada-binaria`,
`evidencia.backend-text.renderizacao-chamada-void-retorno-nulo`,
`evidencia.backend-text.renderizacao-globais` e
`evidencia.backend-text.apresentacao-cli-pseudo-asm`.

O helper `render_cli_pseudo_asm_output` é uma **apresentação sintética em
memória**, não um processo CLI: concatena o cabeçalho, a renderização textual e
o rodapé histórico `Análise semântica concluída sem erros.`. Seu teste é um
golden exato desse helper e não alega execução processual da CLI. O backend
textual continua sendo pseudo-assembly auditável, não assembly nativo montável.

Os testes `validador_cfg_falha_quando_cfg_invalida` e
`check_ignora_flags_de_emissao` ficam fora das regiões 8F. Junto aos quatro
`cli_build_*` excluídos da 8E, o gate estrutural congela a lista exata de seis
exclusões **relativas às regiões 8F** e rejeita absorção indevida por elas; isso
permite que os dois builds de sucesso tenham owner 8G. `tests/backend_text_tests.rs`
mantém seus 9 testes e recebeu apenas 30 linhas de marcadores; os dois helpers
receberam apenas 10 linhas de marcadores em `tests/common/mod.rs`, sem mudança de
assertions, fixtures, nomes, imports, helpers ou lógica.

O catálogo passa de **340 para 348 regiões**, com as 340 entradas anteriores
preservadas por projeção estável. As oito regiões têm chaves únicas, conteúdo
não vazio,
marcadores ordenados, domínio `backend-text` e camada `evidencia`; o catálogo
versionado é idêntico à regeneração canônica. A Onda 8 permanece
**in-progress** e `trama_complete` permanece `false`.

## Onda 8G — evidências do backend `.s` textual

A Onda 8G adiciona exatamente sete regiões no domínio `backend-s` e camada
`evidencia`. Em `tests/common/mod.rs`, duas regiões de contagem zero delimitam
somente `render_backend_s` e `render_cli_asm_s_output`; o helper
`render_backend_s_external_subset` permanece fora. Em
`tests/backend_s_tests.rs`, quatro regiões cobrem seus cinco testes nas
contagens congeladas 1/2/1/1. Em `tests/interpreter_tests.rs`, uma região cobre
os dois builds textuais de sucesso. A distribuição total por região é
**[0, 0, 1, 2, 1, 1, 2]**.

As chaves aprovadas são `evidencia.backend-s.pipeline-helper`,
`evidencia.backend-s.apresentacao-cli-helper`,
`evidencia.backend-s.apresentacao-cli-asm-s`,
`evidencia.backend-s.renderizacao-fluxo-e-abi-textual`,
`evidencia.backend-s.validacao-subset-textual`,
`evidencia.backend-s.freestanding-intencao-textual` e
`evidencia.backend-s.build-cli-artefato-textual`.

`render_cli_asm_s_output` é uma apresentação **sintética em memória**, não um
processo CLI. Já os testes `cli_build_gera_artefato_s_no_diretorio_padrao` e
`cli_build_com_imports_gera_artefato_no_out_dir` são híbridos: executam o
processo `pink build` e comprovam sucesso/saída, criação do `.s` e conteúdo
textual mínimo, mas não montagem, link ou execução do artefato. Os testes
`cli_build_sem_arquivo_falha_com_uso` e
`cli_build_falha_semantica_retorna_erro` continuam como os dois futures sem
owner global.

O recorte congela literalmente `tests/backend_s_external_toolchain_tests.rs`
(79 testes): **external toolchain — etapa não iniciada**; e
`tests/backend_nativo_tests.rs` (47 testes): **backend nativo/paridade — etapa
não iniciada**, incluindo sua evidência de runtime. Ambos permanecem com zero
`@pinker-nav`, zero regiões no catálogo e, portanto, zero testes cobertos. O
helper `render_backend_s_external_subset` permanece **futuro** e fora das sete
regiões 8G. Nenhuma região 8G alega código x86 montável, montagem, link ou
execução nativa. Nos três arquivos de evidência o diff é estritamente
marker-only: 10 linhas em
`tests/common/mod.rs`, 20 em `tests/backend_s_tests.rs` e 5 em
`tests/interpreter_tests.rs`.

O catálogo passa de **348 para 355 regiões** e a camada `evidencia`, de
**165 para 172**. As 348 entradas anteriores são congeladas por projeção estável
que exclui apenas os campos posicionais e inclui `schema`, `key`, `kind`,
`domain`, `layer`, `file`, `summary`, `hash` e `status`. A regeneração canônica
permanece obrigatória. `onda_8f_complete = true`, `onda_8g_complete = true`, `onda_8_complete = false` e
`trama_complete = false`.

## Onda 8H — evidências da toolchain externa do backend `.s`

A Onda 8H adiciona exatamente **dez regiões** no domínio `backend-s` e camada
`evidencia`, liberando o congelamento que a Onda 8G mantinha sobre
`tests/backend_s_external_toolchain_tests.rs`. Nove regiões cobrem os **79
testes** dessa suíte e uma décima delimita o helper compartilhado
`render_backend_s_external_subset` em `tests/common/mod.rs`
(`evidencia.backend-s-externo.pipeline-helper`), que antes permanecia sem owner.
As regiões vizinhas `evidencia.backend-s.pipeline-helper` e
`evidencia.backend-s.apresentacao-cli-helper` continuam intactas: nenhuma foi
ampliada, reduzida ou fundida.

A distribuição por região, em ordem física no arquivo central, é
**[16, 8, 3, 2, 3, 22, 9, 11, 5]** — soma 79, sem teste órfão e sem ownership
duplicado. As chaves são
`evidencia.backend-s-externo.renderizacao-recortes-versionados`,
`evidencia.backend-s-externo.fronteira-ninho-heterogeneo`,
`evidencia.backend-s-externo.fronteira-conversao-virar`,
`evidencia.backend-s-externo.renderizacao-verso-rodata`,
`evidencia.backend-s-externo.renderizacao-quebrar-continuar`,
`evidencia.backend-s-externo.execucao-real-recortes-versionados`,
`evidencia.backend-s-externo.execucao-real-abi-frame-interprocedural`,
`evidencia.backend-s-externo.fronteira-subset-textual`,
`evidencia.backend-s-externo.validacao-estrutural-sintetica` e
`evidencia.backend-s-externo.pipeline-helper`.

**Limites de honestidade congelados pelo gate.** Dos 79 testes, **74 passam
pelo helper compartilhado** `render_backend_s_external_subset` e **5 constroem
um `SelectedProgram` sintético** à mão, chamando
`emit_external_toolchain_subset` diretamente, sem front-end. **31 testes**
montam, linkam e executam de fato: renderizam o `.s`, gravam-no em diretório
temporário, detectam em tempo de execução um driver C (`cc`, `gcc` ou `clang`)
que faz a montagem e a linkedição, executam o binário e validam **apenas
`status.code()`**. Esses 31 são **condicionais**: são pulados silenciosamente
fora de Linux x86_64 ou quando não há driver C — **a suíte pode passar sem
exercer essa evidência**. Os **48 testes restantes** não criam processo algum.
**18 testes são recusas** com mensagem específica. Não há **nenhuma validação
de stdout** e o `stderr` aparece somente como mensagem de falha da asserção de
compilação, nunca como conteúdo validado. Todo o caminho exercitado é o
**hospedado, com `runtime_init=false`**; não há `libpinker_rt.a` nem qualquer
referência a `pinker_rt` no código executável do arquivo central.

O diff nos dois arquivos de evidência é estritamente **marker-only**: 45 linhas
de comentário em `tests/backend_s_external_toolchain_tests.rs` e 5 em
`tests/common/mod.rs`; removidos os marcadores, ambos os arquivos são
byte-equivalentes à base. `tests/backend_nativo_tests.rs` **permaneceu fora**
desta onda, com 47 testes, zero `@pinker-nav` e zero regiões no catálogo; foi
cartografado depois, na Onda 8I.

O catálogo passa de **355 para 365 regiões** e a camada `evidencia`, de **172
para 182**: dez chaves adicionadas, nenhuma removida, nenhuma renomeada e
`SEMANTIC_CHANGES_EXISTING = 0` (apenas deslocamentos posicionais derivados da
inserção dos comentários). As 355 entradas anteriores são congeladas por
projeção estável. A Onda 8H está **completa**; `onda_8_complete = false` e
`trama_complete = false`. Esta onda **não** declara toolchain incondicional,
paridade nativa, runtime funcional, execução multiplataforma nem self-hosting.
A próxima etapa é o **backend nativo**.

## Onda 8I — evidências, execução e paridade do backend nativo

A Onda 8I adiciona exatamente **catorze regiões** no domínio `backend-s` e
camada `evidencia`, liberando o congelamento que as Ondas 8G e 8H mantinham
sobre `tests/backend_nativo_tests.rs`. São **quatro regiões de suporte**
(`suporte-lowering-memoria`, `suporte-driver-c`, `suporte-matriz-paridade-b11`,
`suporte-paridade-stdout`), que delimitam helpers e dados e possuem **zero
testes por design**, e **dez regiões de evidência**, que cartografam os **47
testes** da suíte. A distribuição das dez regiões de evidência, em ordem física,
é **[2, 5, 7, 2, 3, 1, 7, 1, 1, 18]** — soma **47**, sem teste órfão e sem
ownership duplicado.

**Classificação das catorze regiões.** As catorze regiões se dividem em **três
regiões de evidência exclusivamente textual** (`emissao-init-runtime`,
`emissao-abi-e-fluxo-textual`, `emissao-simbolos-runtime-textual`), que reúnem
os 14 testes em memória; **sete regiões de evidência processual**
(`execucao-exit-fumaca-abi`, `paridade-stdout-colecoes`, `paridade-marco-b11`,
`paridade-stdout-programas-maiores`, `paridade-argv`,
`execucao-exit-controle-fluxo`, `paridade-stdout-fases-avancadas`), que reúnem
os 33 testes processuais; e **quatro regiões de suporte**, sem ownership direto
de testes. Somam 3 + 7 + 4 = 14 regiões e 14 + 33 = 47 testes.

**Textual versus processual.** Dos 47 testes, **14 rodam exclusivamente em
memória** e **33 são processuais**. Entre os testes em memória há **13 chamadas
diretas ao emissor hospedado** `emit_external_toolchain_subset`
(`runtime_init=false`) e **uma única chamada direta ao emissor nativo**
`emit_external_toolchain_subset_nativo` (`runtime_init=true`); a diferença
comprovada entre os dois é a presença de `call pinker_rt_iniciar` no prólogo de
`main`. **Sete testes cujo nome contém "nativo" exercitam, de fato, o emissor
hospedado**: apenas procuram referências textuais a símbolos `pinker_*` e não
ligam `libpinker_rt.a` — presença textual de símbolo não prova implementação
funcional do runtime.

**O que esta onda afirma — e o que não afirma.** A Onda 8I **cartografa as
evidências existentes** do backend nativo: estrutura, localização, ownership,
catálogo e fronteiras. Ela **não certifica semanticamente a implementação dos
testes**. Nada aqui prova que a toolchain foi executada, que uma comparação de
stdout ocorreu de fato, que um exit observado é o exit real do ELF ou que nenhum
teste degradou para no-op. As contagens abaixo são **inventário estrutural**.

**Inventário dos caminhos processuais.** Os 33 testes processuais **declaram**
executar `pink build --nativo`, delegar montagem e linkedição a um driver C
externo, ligar `libpinker_rt.a` com `-lpthread`, `-ldl` e `-lm`, e executar o
ELF produzido. Inventariados: **30 caminhos de comparação de stdout** contra o
interpretador usado como oráculo e **3 testes que declaram validar apenas o
exit**. **25 call sites** de `paridade_stdout` e **1 consumidor** de
`paridade_stdout_e_exit`; **4 testes** escrevem a comparação inline. **Dois
pontos** declaram `argv`. **29 desses caminhos comparam atualmente o exit contra
uma constante**; apenas o marco B11 confronta o exit com o retorno observado no
interpretador.

A matriz `CASOS_PARIDADE_B11` reúne **14 casos sobre 13 exemplos distintos** (a
fase221 aparece duas vezes, com e sem argv), consumidos por **um único
`#[test]`** — não equivale a 14 testes independentes, e uma falha antecipada
impede os casos seguintes. Essa agregação é uma **limitação existente da suíte
nativa**, registrada pela cartografia; não é um erro da cartografia.

**Guardas e execução degradada.** Os 33 testes processuais possuem guardas de
plataforma, driver C e `libpinker_rt.a` — **sete conjuntos escritos inline**, os
demais concentrados nos helpers — **capazes de produzir execução degradada ou
no-op** sem sinalizar skip. A validação diferencial com `PATH` vazio observou 47
testes passando sem compilação, sem diretório temporário nativo e sem mensagem
de skip do driver. Essa limitação **permanece registrada, mas não bloqueia a
Trama**: provar ausência de skip silencioso exige instrumentação de runtime, não
cartografia.

O diff em `tests/backend_nativo_tests.rs` é estritamente **marker-only**: 70
linhas de comentário; removidos os marcadores, o arquivo é byte-equivalente à
base. O catálogo passa de **365 para 379 regiões** e a camada `evidencia`, de
**182 para 196**: catorze chaves adicionadas, nenhuma removida, nenhuma
renomeada, `SEMANTIC_CHANGES_EXISTING = 0` e `POSITIONAL_CHANGES_EXISTING = 0`.
As 365 entradas anteriores são congeladas por projeção estável (comprimento
`157379`, FNV-1a `14667879393081127943`).

**Contrato do gate cartográfico.** O gate 8I prova catálogo (14 chaves, 379
regiões totais, 196 na camada `evidencia`, regeneração canônica), história
(as 365 regiões anteriores congeladas), estrutura do arquivo central (47
`#[test]`, 14 regiões contíguas e sem sobreposição, ordem física preservada),
classificação (3 textuais + 7 processuais + 4 de suporte), ownership
(`[2,5,7,2,3,1,7,1,1,18]`, sem órfão e sem duplicata) e **definição única, na
região de suporte aprovada, de cada um dos oito símbolos de suporte**. A Onda 8G
exige, por redundância, que `tests/backend_nativo_tests.rs` mantenha exatamente
47 `#[test]`, além das 14 regiões.

O gate **não** contém analisador léxico de Rust. Ele não decide qual `assert_eq!`
foi resolvida, qual binding está vigente, se uma instrução é alcançável, de onde
vem o stdout ou o exit, nem se a toolchain rodou. Essas perguntas foram
inventariadas e ficam **formalmente deferidas**.

### Pós-Trama Nova — infraestrutura de paridade nativa

Trabalho deferido, fora do escopo cartográfico da Trama:

- prova tipada de exit com `std::process::Termination`;
- registro estruturado de execução (o que rodou, o que foi pulado);
- motivos explícitos de skip, em vez de guardas silenciosas;
- paridade real de exit em todos os caminhos, não só no marco B11;
- granularidade dos casos B11 — um `#[test]` por caso;
- limpeza de diretórios temporários por RAII;
- execução do CI em duas fases, separando build hospedado de build nativo.

Nenhum desses itens é implementado agora.

A Onda 8I está **completa no escopo cartográfico**: `onda_8i_complete = true`,
`onda_8_complete = false`, `trama_complete = false`. Esta onda **não** declara
paridade completa da linguagem, backend nativo completo, runtime completo,
cobertura incondicional, suporte multiplataforma, `clang` validado nem
self-hosting. A próxima etapa atual da Trama é a **cartografia do runtime**, na
Onda 8J.

## Onda 8J — evidências internas do runtime nativo

A **Onda 6E já cartografou integralmente a produção** de
`runtime/pinker_rt/src/lib.rs` em **15 regiões `runtime.*`**. A Onda 8J **não
altera e não recartografa** essas quinze regiões: a cápsula da 6E continua
cobrindo somente produção, e o `#[cfg(test)] mod tests` permanece **fora** da
camada `runtime`. Suas evidências foram cartografadas agora, na camada
`evidencia`, sob o prefixo `evidencia.runtime.`.

A Onda 8J adiciona exatamente **sete regiões** na camada `evidencia`, cobrindo
os **22 `#[test]`** e o **único helper** (`verso_de`) do bloco de testes
internos do runtime. A distribuição, em ordem física, é **[4, 2, 3, 5, 4, 3, 1]**
— soma **22**, sem teste órfão, sem ownership duplicado e sem região de
evidência vazia.

| Chave | Domínio | Testes | Cobertura |
|---|---|---:|---|
| `evidencia.runtime.memoria-alocador` | memoria | 4 | Abertura de `#[cfg(test)] mod tests` e do `use super::*;`; alinhamento e usabilidade do bloco de `pinker_alocar`, não sobreposição entre alocações, alocação de zero bytes, `pinker_liberar` sobre nulo. |
| `evidencia.runtime.inicializacao-abi` | inicializacao | 2 | Captura de `argc`/`argv` por `pinker_rt_iniciar` e versão corrente da ABI. |
| `evidencia.runtime.texto-verso` | texto | 3 | Helper `verso_de` (pertence exclusivamente a esta região, embora seja consumido depois por outros testes) e as operações de verso: tamanho em code points, concatenação e igualdade por conteúdo. |
| `evidencia.runtime.listas-dinamicas` | listas | 5 | Anexar/obter/tamanho, crescimento, `definir`, `inserir` com deslocamento de sufixo e `tirar_ultimo`. |
| `evidencia.runtime.mapas-dinamicos` | mapas | 4 | Chave bombom, chave verso comparada por conteúdo, remoção preservando ordem com ausência silenciosa e crescimento. |
| `evidencia.runtime.leques-carga` | leques | 3 | Tag e cargas posicionais, aninhamento recursivo e crescimento. |
| `evidencia.runtime.mapas-iterador-snapshot` | mapas | 1 | Snapshot das chaves do iterador de mapas; contém o fechamento físico de `mod tests`. |

**Toda a evidência da Onda 8J é evidência em memória.** Nenhum dos 22 testes
cria processo, toca o sistema de arquivos ou invoca toolchain. A onda **não**
declara evidência processual nem paridade.

**Relação com as quinze regiões de produção.** Por menção/call site — relação
estrutural, não semântica —, **seis regiões produtivas** possuem alguma
evidência interna relacionada: `runtime.inicializacao.bootstrap`,
`runtime.memoria.alocador`, `runtime.texto.operacoes`,
`runtime.listas.dinamicas`, `runtime.mapas.dinamicos` e
`runtime.leques.variantes`. As **nove restantes não possuem teste interno
relacionado**:

- `runtime.conversoes.numero-texto`;
- `runtime.texto.formatacao`;
- `runtime.io.saida`;
- `runtime.arquivos.io`;
- `runtime.caminhos.sistema`;
- `runtime.tempo.relogio`;
- `runtime.aleatorio.gerador`;
- `runtime.ambiente.argumentos`;
- `runtime.processos.execucao`.

Isso é uma **lacuna registrada, não um bloqueio da Trama**. A Onda 8J **não
criou nenhum teste novo**: a alteração em `runtime/pinker_rt/src/lib.rs` é
**marker-only** — 35 linhas de comentário `// @pinker-nav:*` adicionadas (7
regiões × 5 linhas), zero linhas removidas, zero linhas de código ou de teste
alteradas.

**Limites observados durante o inventário.** A suíte `backend_nativo_tests`
pode passar em **modo degradado sem a staticlib** do runtime; com
`PINKER_RT_LIB` presente, houve **evidência processual observada** durante o
inventário. Isso **confirma, mas não modifica**, os limites já documentados na
Onda 8I.

**Contrato do gate cartográfico.** O gate
`onda_8j_cartografa_evidencias_internas_do_runtime` prova catálogo (as sete
chaves exatas, 386 regiões totais, 203 na camada `evidencia`, 15 na camada
`runtime`, regeneração canônica), estrutura do arquivo (22 `#[test]`, helper
único, sete regiões contíguas, disjuntas e em ordem física, abertura e
fechamento do módulo pertencendo à primeira e à última região), ownership
(`[4,2,3,5,4,3,1]`, `unowned_tests=[]`, `duplicate_ownership=[]`,
`evidence_regions_without_tests=[]`, `ownership_sum=22`), classificação (camada
`evidencia`, evidência em memória, arquivo do runtime), preservação da produção
(as 15 chaves `runtime.*` inalteradas e inteiramente anteriores ao bloco de
testes, 155 símbolos produtivos com ownership único — 147 definições textuais
mais os 8 wrappers gerados por `formatar_wrappers!` — e 107 símbolos de ABI —
99 `extern "C" fn` diretas mais os mesmos 8 wrappers) e história (as **379
regiões anteriores** congeladas por projeção estável e FNV-1a).

O gate **não** contém analisador semântico. Não decide se as asserções estão
semanticamente corretas, não prova segurança de memória, ausência de
vazamentos, equivalência com o interpretador, execução de ELF, completude de
ABI, dataflow, resolução de macros nem alcançabilidade.

**Nota sobre referências históricas.** As seções anteriores deste inventário
(entre elas as das Ondas 6A, 6E, 7 e 8G) descrevem o estado **no momento de
cada onda**. Onde afirmam que o `#[cfg(test)] mod tests` do runtime está fora
da cartografia, isso continua verdadeiro **para a camada `runtime`**: o bloco
segue fora dela, e passou a ser coberto pela camada `evidencia` a partir desta
onda.

### Pós-Trama — evidência interna do runtime

Trabalho deferido, fora do escopo cartográfico da Trama:

- testes para os nove domínios sem evidência interna;
- prova tipada e registro de execução;
- skips observáveis;
- segurança de memória;
- vazamentos e RAII;
- completude de ABI;
- portabilidade;
- self-hosting.

Nenhum desses itens é implementado agora.

A Onda 8J está **completa no escopo cartográfico**: `onda_8j_complete = true`,
`onda_8_complete = false`, `trama_complete = false`. A próxima etapa é a
**auditoria de convergência da Onda 8**.

## Testes ativos e apps adiados

`tests/` é raiz oficial ativa desde a Onda 8A. O scanner tem três raízes
oficiais: `src/`, `runtime/pinker_rt/src/` e `tests/`. Fixtures com textos
parecidos com marcadores são protegidas pelo reconhecedor lexical implementado
na Onda 8A. `apps/` continua desativada até a Onda 9: reúne fontes `.pink`, que
exigem uma convenção de marcador própria antes de entrar no scanner.

Já estão cartografados como evidência `tests/common/mod.rs` (parcialmente;
tests/common/mod.rs helpers: 8), `tests/lexer_tests.rs`, `tests/parser_tests.rs`,
`tests/semantic_tests.rs` e — desde a Onda 8D — as sete suítes de pipeline
`tests/ir_tests.rs`, `tests/ir_validate_tests.rs`, `tests/cfg_ir_tests.rs`,
`tests/cfg_ir_validate_tests.rs`, `tests/instr_select_tests.rs`,
`tests/abstract_machine_tests.rs` e `tests/abstract_machine_stack_tests.rs`;
todas, integralmente. O backend textual está parcialmente cartografado na Onda
8F (7/9 testes), e a suíte `tests/backend_s_tests.rs` está integralmente
cartografada na Onda 8G (5/5), junto a dois builds textuais de sucesso da suíte
do interpretador. Desde a Onda 8H, `tests/backend_s_external_toolchain_tests.rs`
também está integralmente cartografada (79/79), e desde a Onda 8I,
`tests/backend_nativo_tests.rs` (47/47). As demais suítes `tests/*.rs` (runtime,
paridade restante, Trama, documentais, CLI, apps) continuam pendentes.

- `tests/*.rs` — evidência por camada (lexer, parser, semântica, IR/CFG/seleção/
  máquina, interpretador, backends, runtime nativo, Trama, CLI, paridade nativa).
  Marcar apenas grupos de evidência conceituais (ex.: `tests.backend-s.abi-argumentos`,
  `tests.trama.manifesto-imutavel`) — nunca uma âncora por `#[test]`.
- `apps/guardiao_pinker/principal.pink` — Guardião Pinker (auditoria de contratos
  do repositório); marco de app real em Pinker. Candidato: `apps.guardiao.auditoria`.

## Cobertura acumulada (após Onda 8J)

| Métrica | Valor |
|---|---:|
| Produção em `src/` | 32 |
| Produção de `src/` ancorada | 32 |
| Produção de `src/` pendente | 0 |
| Produção em `runtime/pinker_rt/src/` | 1 |
| Produção do runtime ancorada | 1 |
| Produção total nas duas raízes de produção | 33 |
| Arquivos de produção ancorados | 33 |
| Arquivos de produção pendentes | 0 |
| Evidência em `tests/common/mod.rs` | Parcial: 8 helpers |
| Evidência em `tests/lexer_tests.rs` | Integral |
| Evidência em `tests/parser_tests.rs` | Integral |
| Evidência em `tests/semantic_tests.rs` | Integral |
| Evidência nas 7 suítes de pipeline (IR/CFG/seleção/máquina) | Integral (Onda 8D) |
| Evidência em `tests/interpreter_tests.rs` | 536/538 (534 na Onda 8E + 2 builds textuais na 8G; 2 falhas sem owner) |
| Evidência em `tests/backend_text_tests.rs` | 7/9 (Onda 8F; 2 exclusões relativas) |
| Evidência em `tests/backend_s_tests.rs` | 5/5 (Onda 8G) |
| Evidência em `tests/backend_s_external_toolchain_tests.rs` | 79/79 (Onda 8H; 31 condicionais que montam/linkam/executam, 48 só em memória) |
| Evidência em `tests/backend_nativo_tests.rs` | 47/47 (Onda 8I; 33 processuais sob três guardas, 14 só em memória) |
| Evidência em `runtime/pinker_rt/src/lib.rs` (`mod tests`) | 22/22 (Onda 8J; todas em memória) |
| Demais suítes `tests/*.rs` | Pendentes |
| Regiões antes da Onda 7 | 163 |
| Regiões adicionadas na Onda 7 | 20 |
| Regiões adicionadas na Onda 8D | 58 |
| Regiões adicionadas na Onda 8E | 46 |
| Regiões adicionadas na Onda 8F | 8 |
| Regiões adicionadas na Onda 8G | 7 |
| Regiões adicionadas na Onda 8H | 10 |
| Regiões adicionadas na Onda 8I | 14 (4 de suporte + 10 de evidência) |
| Regiões adicionadas na Onda 8J | 7 (evidência interna do runtime) |
| Regiões no catálogo | 386 |
| Raízes oficiais ativas | 3 |
| Chaves duplicadas | 0 |
| Erros de validação (`nav verificar`) | 0 |

A produção das **duas raízes de produção** do scanner (`src/` e
`runtime/pinker_rt/src/`) está agora **integralmente ancorada** — os 3
pendentes da Onda 6E (`src/main.rs`, `src/editor_tui.rs`, `src/boot.rs`)
receberam suas 20 regiões nesta onda (ver "Onda 7 — cartografia das
superfícies operacionais"). A contagem `33 = 33 + 0` é o corpus completo de
produção nas duas raízes de produção do scanner (`src/` e
`runtime/pinker_rt/src/`); `src/lib.rs` (só `pub mod`), os binários-fixture
`src/bin/pinker_fase16x_*.rs` e o catálogo gerado `src/navigation.jsonl` ficam de
fora por não terem responsabilidade nomeável (ver "Arquivos sem candidatos a
âncora"). O único arquivo de produção do runtime,
`runtime/pinker_rt/src/lib.rs`, permanece **totalmente ancorado** desde a Onda
6E (15 regiões cobrindo as 99 funções ABI diretas mais os 8 wrappers
`pinker_formatar_verso_1..8` gerados pela macro `formatar_wrappers!` — 107
símbolos de ABI exportados no total — e os helpers internos; 0
símbolos não classificados fora do `#[cfg(test)] mod tests`, explicitamente
excluído).

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
| interpreter | 15 | modelo/estado, execução (programa, fluxo, instruções, valores/tipos), intrínsecos hospedados (8 regiões: acaso, listas, mapas-verso-bombom, leques, io-arquivo-texto, tempo-processos-ambiente, conversões-número-texto, mapas-tipados), serviços auxiliares do host, diagnóstico |
| backend-text | 9 | validador; Onda 6B (8): modelo, lowering (cfg-programa, seleção-programa, instruções-selecionadas), pipeline emissão, renderização (programa, instruções, componentes) |
| backend-s | 24 | Onda 6C: pipeline (textual-selecionado, toolchain-externa, nativo-runtime), validação (subset-textual, labels-tipos), modelo (callconv-externa), abi (registradores-argumentos, prólogo-parâmetros, blocos-terminadores), lowering (globais-rodata, funções-frames, blocos-terminadores, operações-memória, chamadas-sysv, falar-runtime, operações-lineares, operandos-slots), renderização (callconv-programa, abi-textual programa/instruções/componentes), runtime (intrínsecas-por-aridade, símbolos-intrínsecas), dados (strings-rodata) |
| semantic | 10 | importações, sistema de tipos, escopos, duas-passagens, tratos, funções, comandos, fluxo, expressões, chamadas (Onda 5A) |
| trama | 11 | normalização, jsonl, marco, catálogos e consultas doc/código, raízes de código controladas (Onda 6D), manifesto, ledger, projeções |
| runtime | 15 | Onda 6E: inicialização/bootstrap, alocador, texto (operações, conversões, formatação), io, listas, mapas, leques, arquivos, caminhos, tempo, aleatório, ambiente, processos |
| cli | 15 | Onda 7: config-modelos, ajuda-usage, parsing (subcomandos, roteamento), execução (entrada, editor-repl), nav (consulta, sincronização-verificação), doc (consulta, sincronização, mudanças, verificação), análise-pipeline, build-nativo, módulos-importação |
| editor | 4 | Onda 7: estado-modelo, sessão-comandos, render-saída, análise-checagem |
| boot | 1 | Onda 7: geração-fronteira-freestanding (arquivo inteiro) |
| evidencia | 215 | Onda 8B (19) + Onda 8C (34) + Onda 8D (58: ir 11, cfg 14, select 6, machine 27) + Onda 8E (46: interpreter) + Onda 8F (8: backend textual) + Onda 8G (7: backend `.s` textual) + Onda 8H (10: toolchain externa do backend `.s`) + Onda 8I (14: backend nativo — 4 de suporte, 10 de evidência) + Onda 8J (7: evidência interna do runtime) + cápsula de `nav_catalog_tests` (6) + cápsula de `doc_catalog_tests` (6) |
| **total** | **398** | |

Pendentes de cartografia: as demais suítes `tests/*.rs` na Onda 8 e `apps/` na
Onda 9. As três superfícies operacionais (cli/editor/boot) foram concluídas na
Onda 7.

## Fechamento formal da Onda 8

A auditoria de convergência encontrou **zero bloqueadores da Onda 8** e aceitou
a cadeia cartográfica 8A–8J, mas exigiu um commit formal de fechamento. Token do
registro: `TRAMA_WAVE_8_CONVERGENCE_AUDIT_ACCEPTED_IMPLEMENTATION_REQUIRED`.
Este commit realiza somente esse fechamento cumulativo, congela o estado
386/203/15 e preserva os gates históricos; não inicia uma nova cápsula.
No vocabulário daquele marco: catálogo atual = 386; evidencia atual = 203;
runtime atual = 15.

Estado canônico atual:

- `onda_8_complete = true`
- `trama_complete = false`

### TRAMA_OPERATIONAL_REMAINDER

- cápsulas operacionais/documentais;
- suítes restantes de evidência de CLI;
- raiz `apps/` da Onda 9 e política de marcadores `.pink`.

`pink nav mapa` não é requisito do repositório: a auditoria não encontrou um
requisito de fechamento de navegação orientada a arquivos respaldado pelo
estado atual.

### POST_TRAMA_TEST_INFRASTRUCTURE

- exit tipado;
- ledger de execução;
- skips observáveis;
- infraestrutura de execução para paridade nativa;
- lacunas de testes internos do runtime.

### FUTURE_COMPILER_OR_RUNTIME_WORK

- segurança de memória;
- vazamentos e RAII;
- completude de ABI;
- portabilidade;
- backend/runtime completos;
- self-hosting.

## Primeira cápsula operacional/documental da Trama

A primeira cápsula operacional/documental da Trama está completa.
`tests/nav_catalog_tests.rs: 6 regiões`, classificadas como
**2 suporte + 4 evidência**, cobrem a estrutura exata de
**5 constantes + 4 helpers + 6 testes**. O catálogo passa a 392/209/15 sem
remover ou alterar semanticamente as 386 regiões do fechamento da Onda 8.

O escopo continua deliberadamente limitado: a suíte usa repositórios
sintéticos, executa `pink nav` em processos filhos, faz asserções seletivas e
usa limpeza explícita, não RAII, portanto pode deixar sobras após panic. A
cartografia não prova a correção semântica completa da CLI, não é validação
exaustiva do catálogo e não prova comportamento de processos independente de
plataforma. Também não conclui a Trama, não ativa a Onda 9 e não oferece
suporte a apps/.

No vocabulário daquela cápsula: catálogo atual = 392; evidencia atual = 209;
runtime atual = 15. Estado de continuidade registrado por ela, preservado como
marco histórico: próximo alvo: tests/doc_catalog_tests.rs;
alvo subsequente: tests/trama_query_tests.rs. `apps/` permanece reservada à Onda 9.

## Segunda cápsula operacional/documental da Trama

A segunda cápsula operacional/documental da Trama está completa.
`tests/doc_catalog_tests.rs: 6 regiões`, classificadas como
**2 suporte + 4 evidência**, cobrem a estrutura exata de
**3 constantes + 4 helpers + 4 testes**. As duas regiões de suporte agrupam a
configuração documental (`DOC_TOML`, `PORTAL`, `CORE`) e o maquinário de
processo (`temp_repo`, `write`, `fixture`, `run`); cada uma das quatro regiões
de evidência possui exatamente um teste — sincronização e verificação,
catálogo desatualizado, extração de seção e âncora `@pinker-doc` desbalanceada.

A cartografia é **somente por linhas de marcador**: retirar as 30 linhas
`@pinker-nav:` reconstrói byte a byte a fonte-base
`c2759331fb4377521c4f51479ae732c74ef0e4121d4e5ee0415a41eb67a37dd3`. Nenhuma
linha original foi alterada, nenhum teste foi reordenado, nenhum helper foi
movido e nenhuma fixture foi reescrita. O catálogo passa a 398/215/15 sem
remover ou alterar semanticamente as 392 regiões da base mergeada — projeção
estável preservada em 170076 bytes / `a887306f8c75b75c` — nem as 386 regiões
históricas da Onda 8 — projeção estável preservada em 168339 bytes /
`16afa50464b712b5`. A projeção completa das 398 regiões mede 171741 bytes /
`61ac3eb554228685`. Estado atual explícito desta cápsula: catálogo atual = 398;
evidencia atual = 215; runtime atual = 15; `onda_8_complete = true`;
`trama_complete = false`.

O escopo continua deliberadamente limitado: a suíte usa repositórios
sintéticos, executa `pink doc` em processos filhos, faz asserções seletivas e
usa limpeza explícita, não RAII, portanto pode deixar sobras após panic. A
cartografia não prova a correção semântica completa da CLI, não é validação
exaustiva do catálogo documental e não prova comportamento de processos
independente de plataforma. Também não conclui a Trama, não ativa a Onda 9 e
não oferece suporte a apps/.

Estado de continuidade: próximo alvo: tests/trama_query_tests.rs.
`apps/` permanece reservada à Onda 9.

## Terceira cápsula operacional/documental da Trama

A terceira cápsula operacional/documental da Trama está completa.
`tests/trama_query_tests.rs: 8 regiões`, classificadas como
**2 suporte + 6 evidência**, cobrem a estrutura exata de
**4 constantes + 6 helpers + 10 testes**. As duas regiões de suporte agrupam a
configuração de fixture (`DOC_TOML`, `PORTAL`, `CORE`, `SRC`) e o maquinário de
processo (`temp_repo`, `write`, `fixture`, `doc`, `nav`, `code`); as seis
regiões de evidência cobrem consulta somente-catálogo, deriva de fonte,
estabilidade de JSON, limite de resultados, erros de catálogo e códigos de
saída de consulta.

A cartografia é **somente por linhas de marcador**: retirar as 40 linhas
`@pinker-nav:` reconstrói byte a byte a fonte-base
`65ba7f360174b1ef15ab5b9e246e3a0fce8e888359798898986969ff9c016f3f`. O texto
semelhante a marcador dentro do literal Rust `SRC`
(`// @pinker-nav:start rosa.identidade.core`) permanece conteúdo de fixture e
**não** é contado como marcador real. Nenhuma linha original foi alterada,
nenhum teste foi reordenado, nenhum helper foi movido e nenhuma fixture foi
reescrita. O catálogo passa a 406/223/15 sem remover ou alterar semanticamente
as 398 regiões da base predecessora — projeção estável preservada em 171741
bytes / `61ac3eb554228685` —, as 392 regiões pós-nav-catalog — 170076 bytes /
`a887306f8c75b75c` — nem as 386 regiões históricas da Onda 8 — 168339 bytes /
`16afa50464b712b5`. A projeção completa das 406 regiões mede 174073 bytes /
`60d9bd7d48f90c55`. Estado atual explícito desta cápsula: catálogo atual = 406;
evidencia atual = 223; runtime atual = 15; `onda_8_complete = true`;
`trama_complete = false`.

O escopo continua deliberadamente limitado: a suíte usa repositórios
sintéticos, executa `pink doc` e `pink nav` em processos filhos, faz asserções
seletivas e usa limpeza explícita, não RAII, portanto pode deixar sobras após
panic. A presença de campos JSON não é validação de schema completa; a
igualdade repetida não prova toda a semântica de ranking; a remoção observada
de fontes não prova toda independência possível de fonte; os casos de erro de
catálogo não são validação exaustiva do parser; a cobertura de códigos de saída
não é exaustiva. A cartografia não prova a correção semântica completa da CLI,
não é validação exaustiva do catálogo e não prova comportamento de processos
independente de plataforma. Também não conclui a Trama, não ativa a Onda 9 e
não oferece suporte a apps/.

Prontidão para fechamento: `ADDITIONAL_CAPSULES_REQUIRED`. A consulta é a última
cápsula individualmente nomeada, mas **não** é a última cápsula operacional:
antes de um fechamento formal resta uma tranche operacional consolidada
cobrindo exatamente `tests/trama_ci_tests.rs`, `tests/trama_manifest_tests.rs`,
`tests/trama_projection_tests.rs`, `tests/trama_scale_tests.rs`,
`tests/trama_sync_tests.rs` e `tests/trama_template_tests.rs`. O fechamento
formal não é autorizado por esta cápsula e a Onda 9 não começa automaticamente.
`apps/` permanece reservada à Onda 9.

## Próximo ponto de retomada

As seis suítes operacionais — antes descritas como as demais suítes operacionais de `tests/`
da tranche consolidada — estão cartografadas pelas
Ondas A–C. A tarefa singular seguinte é o fechamento operacional formal da Onda
D, em trabalho separado. A Onda 9 não começa automaticamente e `apps/`
permanece reservada à Onda 9; `trama_complete = false`.

## Cápsula `pink agente` — Onda B

A Onda A completa permanece projetável. A Onda B completa acrescenta quatro
regiões de produção (`development.agent.git-diff`,
`development.agent.marker-only`, `development.agent.projection` e
`development.agent.sensitivity`) e dez regiões marker-only em
`tests/trama_manifest_tests.rs` e `tests/trama_sync_tests.rs`: quatro support e
seis evidence. O catálogo passa a 439 regiões, 247 chaves `evidencia.*` e 15
regiões runtime.

Onda C é a próxima cápsula, com `tests/trama_projection_tests.rs` e
`tests/trama_scale_tests.rs`. `trama_complete = false`; Onda 9 inativa;
`apps/` reservada. Git/diff, marker-only, projection e sensibilidade são provas
operacionais delimitadas: não fornecem sandbox de SO, semântica de negócio,
cobertura exaustiva nem restauração crash-proof. A Onda B não publica pelo
próprio runner e não conclui a Trama.

## Cápsula `pink agente` — Onda C

A Onda C completa acrescenta quatro regiões de produção
(`development.agent.pr-body`, `development.agent.publication`,
`development.agent.remote-checks` e `development.agent.resume`) e nove
regiões marker-only em `tests/trama_projection_tests.rs` e
`tests/trama_scale_tests.rs`: quatro support e cinco evidence. O catálogo
passa a 452 regiões, 256 chaves `evidencia.*` e 15 regiões runtime.

O runner valida o corpo de PR, cria commit por conjunto exato, faz um push
normal, cria ou reconcilia uma única PR, verifica o corpo remoto e observa
somente os checks do SHA candidato. O dogfood divide publicação e retomada:
`publicar` deixa `CHECKS_PENDING`; `retomar` reconcilia identidades e só
aceita após todos os checks obrigatórios em `SUCCESS`. A allowlist GH exclui
edição, comentário, review, rerun, merge e auto-merge.

As seis suítes operacionais estão cartografadas, mas o fechamento formal
continua reservado à Onda D. `trama_complete = false`; Onda 9 inativa;
`apps/` reservada. Não se alega sandbox, atomicidade distribuída,
disponibilidade do GitHub, correção remota automática ou cobertura exaustiva.

A correção de agregação de checks repetidos permanece na região
`development.agent.remote-checks`, sem nova região. Como `ci.yml` dispara em
`push` e `pull_request`, o mesmo SHA pode expor múltiplas ocorrências `rust`;
a função pura `classify_required_check_states` agrega todas as ocorrências com
precedência `BLOCKED > PENDING > SUCCESS`, e a multiplicidade sozinha nunca
bloqueia. A duplicidade declarativa da spec continua rejeitada no parsing. Os
totais permanecem `452 / 256 / 15`; a projeção medida passa a
`188231 / c9bbcf91746b3080`. O candidato anterior não canônico
`452 / 256 / 15 / 188231 / 0b2368dcb80c6f23` fica registrado apenas como
referência histórica do run rejeitado por `LOCAL_CHECK_AGGREGATION_BUG`, sem
rerun nem bypass.
