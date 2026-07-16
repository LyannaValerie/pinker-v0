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
útil sozinha. As **Ondas 0–6C** já estão na `main`, fechando a cadeia de lowerings
AST → IR → CFG → seleção → máquina, a execução hospedada e os dois backends
(textual e `.s`/ABI nativa). A **Onda 6** foi decomposta em entregas
independentes: 6A (`src/interpreter.rs`), 6B (`src/backend_text.rs`), 6C
(`src/backend_s.rs`), 6D (ampliação controlada das raízes do scanner) e 6E
(`runtime/pinker_rt/src/lib.rs`). Esta rodada conclui a **Onda 6D**:
generalização de `pink nav` para raízes de código controladas — `src/` e
`runtime/pinker_rt/src/`, ambas ativas e obrigatórias no fluxo oficial —, sem
cartografar o conteúdo do runtime (isso é a Onda 6E).

## Contrato do scanner

O scanner de `pink nav` indexa hoje um **conjunto explícito de raízes
controladas** do repositório (`official_scan_roots()` em `src/nav.rs`, região
`trama.codigo.raizes`): `src/` e `runtime/pinker_rt/src/`, ambas obrigatórias
no fluxo oficial (`pink nav sincronizar`/`verificar`). Cada raiz é validada
antes de qualquer leitura — ausência, caminho que não é diretório ou link
simbólico falham com `E-NAV-SCAN` antes de qualquer escrita do catálogo, sem
gerar índice parcial. Os caminhos registrados em `file` são **repo-relativos**
(`src/nav.rs`, `runtime/pinker_rt/src/lib.rs`), com `/` como separador, nunca
absolutos e nunca contendo `..`. Links simbólicos não são seguidos — nem de
diretório (evita ciclos e fuga da raiz) nem de arquivo — e uma raiz oficial que
seja, ela mesma, um link simbólico é recusada. A extensão indexada em ambas as
raízes é `.rs`. `tests/` e `apps/` permanecem **desativadas**: `tests/` tem
fixtures com textos semelhantes a marcadores dentro de strings (risco de falso
positivo ao varrer sem uma política própria de exclusão) e `apps/` reúne
fontes `.pink`, que exigem uma política de marcadores própria antes de entrar
no scanner. Ambas ficam para ondas específicas (8 e 9).

**Estratégia escolhida: B** — `CodeIndex::scan` permanece como wrapper fino de
raiz única para fixtures/testes (sem prefixo fabricado; o caminho é relativo à
raiz recebida), enquanto a produção usa a API multi-raiz
(`CodeIndex::scan_repo` → `official_scan_roots()` → `scan_roots` →
`collect_source_files` → `scan_file`), única fonte da política de raízes. A
chave de região continua global: nenhuma raiz vira namespace, e uma mesma
chave em duas raízes é reportada como `DuplicateKey` com os dois caminhos.

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

Esta onda **não** conclui a Onda 6 inteira: falta a 6E (cartografia do
conteúdo do runtime nativo) antes de a Onda 6 poder ser dada por encerrada.

## Onda 6E e orquestração (adiadas)

Inventariados; revisão atual `estrutural` para as camadas ainda pendentes.

| Arquivo | Camada | Propósito (do módulo-doc/estrutura) | Complexidade | Âncoras atuais | Onda-alvo |
|---|---|---|---|---|---|
| `runtime/pinker_rt/src/lib.rs` | runtime | Runtime nativo linkado por `pink build --nativo`; alocação, coleções nativas, ABI. Raiz **ativa** desde a Onda 6D, mas ainda **sem cartografia** (nenhuma região aponta para `runtime/`). | transversal | — | 6E |
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

## Testes e apps (adiados — raízes desativadas)

Inventariados para as Ondas 8 e 9. O scanner já indexa duas raízes (`src/` e
`runtime/pinker_rt/src/`, Onda 6D), mas `tests/` e `apps/` permanecem
desativadas por política explícita — `tests/` tem fixtures com textos
parecidos com marcadores dentro de strings (risco de falso positivo sem uma
regra própria de exclusão) e `apps/` reúne fontes `.pink`, que exigem uma
convenção de marcador própria antes de entrar no scanner. Ativar qualquer uma
delas é onda própria.

- `tests/*.rs` — evidência por camada (lexer, parser, semântica, IR/CFG/seleção/
  máquina, interpretador, backends, runtime nativo, Trama, CLI, paridade nativa).
  Marcar apenas grupos de evidência conceituais (ex.: `tests.backend-s.abi-argumentos`,
  `tests.trama.manifesto-imutavel`) — nunca uma âncora por `#[test]`.
- `apps/guardiao_pinker/principal.pink` — Guardião Pinker (auditoria de contratos
  do repositório); marco de app real em Pinker. Candidato: `apps.guardiao.auditoria`.

## Cobertura acumulada (após Onda 6D)

| Métrica | Valor |
|---|---:|
| Arquivos de produção em `src/` (excl. `lib.rs`, fixtures `bin/*` e o gerado `navigation.jsonl`) | 32 |
| Arquivos com responsabilidade ancorada (Ondas 0–6D) | 29 |
| Arquivos de produção ainda pendentes (adiados à Onda 7) | 3 |
| Regiões antes da Onda 6D | 147 |
| Regiões adicionadas na Onda 6D | 1 |
| Regiões no catálogo | 148 |
| Chaves duplicadas | 0 |
| Erros de validação (`nav verificar`) | 0 |

Os **3 pendentes** são `src/main.rs`, `src/editor_tui.rs` e `src/boot.rs` — todos
arquivos de produção reais em `src/`, sem âncoras, explicitamente adiados à Onda 7
(ver a tabela "Onda 6E e orquestração (adiadas)"). A contagem `32 = 29 + 3` é o
corpus completo de produção; `src/lib.rs` (só `pub mod`), os binários-fixture
`src/bin/pinker_fase16x_*.rs` e o catálogo gerado `src/navigation.jsonl` ficam de
fora por não terem responsabilidade nomeável (ver "Arquivos sem candidatos a
âncora"). A métrica não conta as superfícies adiadas como ancoradas; a raiz
`runtime/pinker_rt/src/` está **ativa** no scanner, mas nenhum arquivo dela é
contado como ancorado — ela ainda não recebeu nenhuma região (Onda 6E).

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
| **total** | **148** | |

Pendentes de cartografia: conteúdo do runtime nativo (6E), cli/editor/boot
(Onda 7), tests/apps (Ondas 8/9, após ativar as respectivas raízes).

## Próximo ponto de retomada

**Onda 6E — cartografia integral de `runtime/pinker_rt/src/lib.rs`.** A Onda
6D já ativou `runtime/pinker_rt/src/` como raiz do scanner (obrigatória no
fluxo oficial), mas **não** ancorou nenhum conteúdo nela — nenhuma região do
catálogo real tem `file` começando por `runtime/`. A 6E deve revisar
`runtime/pinker_rt/src/lib.rs` linha a linha e cartografar suas
responsabilidades reais (alocação, coleções nativas, ABI) com âncoras
`@pinker-nav` próprias, sem tocar a política de raízes definida em
`trama.codigo.raizes`. Permanecem depois: Onda 7 — cli/editor/boot; Ondas 8/9
— tests/apps (cada uma exige ativar sua própria raiz primeiro).
