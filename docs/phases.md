# Linha do tempo de fases

- Convenção de registro (ativa):
  - **Fase N** = entrega funcional/estrutural que altera capacidade real do projeto.
  - **Rodada documental** = consolidação/curadoria/higiene documental sem nova feature funcional.
  - Rodadas documentais **não recebem número de fase**.

- Fase 50 — casts controlados (escopo mínimo e explícito)
  - continuidade histórica da trilha funcional preservada: Fase 49 segue como fase principal anterior e Fase 48-H1 segue como rodada extraordinária/hotfix sem reordenar roadmap.
  - sintaxe de cast explícito adicionada: `expr virar tipo`, com associatividade à esquerda e precedência pós-unária (sem coerção implícita global).
  - frontend/AST/JSON/printer integrados com novo nó de expressão para cast explícito.
  - semântica desta fase: somente cast inteiro->inteiro (`bombom`, `u8/u16/u32/u64`, `i8/i16/i32/i64`), incluindo aliases resolvidos ao tipo subjacente.
  - casts envolvendo `logica`, `seta`, `ninho` e arrays fixos seguem proibidos nesta fase com diagnóstico explícito.
  - IR estruturada ganhou representação mínima para cast; validação de IR reforçada para rejeitar cast fora da política inteiro->inteiro.
  - decisão de escopo operacional: CFG/Machine/runtime ainda não loweram/executam cast nesta fase; erro explícito preserva segurança (incluindo bloqueio de signed no runtime).
  - próximo item normal do roadmap principal: Bloco 2, item 4 (`sizeof`/alinhamento).

- Fase 49 — acesso a campo e indexação (escopo mínimo de leitura)
  - continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
  - frontend atualizado para cadeia postfix com chamada + acesso a campo (`obj.campo`) + indexação (`arr[idx]`) preservando precedência/associatividade existente.
  - semântica passa a validar leitura de campo em `ninho` e leitura por índice em array fixo, com diagnósticos explícitos para base inválida, campo inexistente e índice não inteiro.
  - IR estruturada ganhou representação mínima para `field access` e `index`, enquanto CFG/execução permanecem deliberadamente sem lowering operacional desses nós nesta fase.
  - decisão de escopo: leitura apenas (sem escrita em LHS), sem bounds-check novo e sem abrir dereferência/aritmética de ponteiro, casts, `sizeof`, alinhamento, `volatile` ou backend nativo.

- Fase 47 — structs (`ninho`) como tipo nomeado composto (escopo mínimo)
  - continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
  - frontend atualizado de forma incremental para aceitar declaração global `ninho Nome { campo: tipo; ... }`
  - parser/AST integrados com item de struct e campos tipados, mantendo uso do nome em posições tipadas existentes
  - semântica registra structs, valida campo duplicado, tipo de campo inexistente, redefinição de nome e rejeita recursão direta (`Node { prox: Node; }`)
  - integração mínima no pipeline: IR reconhece tipo estrutural nominal como categoria `struct` para assinaturas/slots, sem abrir valores operacionais nem acesso a campo
  - cobertura incremental em lexer/parser/semântica/IR para cenário positivo e negativos centrais desta fase
  - fora de escopo mantido: `obj.campo`, leitura/escrita de campo, literais/construtor de struct, layout/ABI/memória explícita, ponteiros e backend `.s`

- Fase 46 — arrays fixos (tipo estrutural mínimo)
  - continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
  - frontend atualizado de forma incremental para aceitar tipo de array fixo na sintaxe `[tipo; N]` em pontos tipados existentes
  - semântica valida tamanho estático simples (`N > 0`), resolve aliases no tipo-base e rejeita tipo-base inválido/inexistente
  - limite deliberado desta fase: arrays fixos aninhados ainda não são suportados (erro explícito), sem indexação/acesso por elemento
  - IR/lowering recebeu representação mínima de tipo para arrays fixos em assinaturas/slots, preservando o pipeline sem redesign
  - cobertura incremental em lexer/parser/semântica/IR para cenário positivo e negativo (tamanho inválido)

- Fase 45 — aliases de tipo (`apelido`)
  - continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
  - keyword `apelido` adicionada no frontend e integrada ao parser/AST como declaração global de alias (`apelido Nome = Tipo;`)
  - aliases resolvidos semanticamente para tipo subjacente (sem tipo nominal novo), com uso em parâmetros, retorno, variáveis locais e constantes globais
  - validações de erro adicionadas para alias inexistente, alias duplicado no escopo global e cadeia recursiva de aliases
  - IR/lowering preservam o pipeline existente convertendo aliases para tipos reais antes das camadas downstream
  - cobertura incremental em lexer/parser/semântica/IR/CLI com exemplos versionados `examples/run_alias_tipo_basico.pink` e `examples/check_alias_tipo_inexistente.pink`

- Fase 44 — inteiros signed com largura fixa (`i8`, `i16`, `i32`, `i64`)
  - continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
  - suporte tipado explícito em lexer/parser/semântica/IR/CFG/selected/Machine/runtime textual
  - política desta fase: validação estrita entre larguras e entre signed/unsigned; sem coerção implícita entre famílias
  - literais inteiros continuam representados internamente como `bombom` (`u64`) e podem inicializar/argumentar inteiros (signed/unsigned) em contexto tipado esperado
  - unário `-` aceito para tipos inteiros (incluindo signed fixos), mantendo semântica operacional atual do runtime textual
  - cobertura incremental em testes de lexer/parser/semântica/IR/CLI + exemplo `examples/run_signed_basico.pink`

- Fase 43 — inteiros unsigned com largura fixa (`u8`, `u16`, `u32`, `u64`)
  - continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
  - suporte tipado explícito em lexer/parser/semântica/IR/CFG/selected/Machine/runtime textual
  - política desta fase: validação estrita de tipos unsigned; sem coerção implícita entre larguras
  - literais inteiros continuam representados internamente em `u64` (`bombom`) e podem inicializar/argumentar unsigned de forma direta
  - cobertura incremental em testes de lexer/parser/semântica/IR/CLI + exemplo `examples/run_unsigned_basico.pink`

- Fase 42 — operador `%` nativo (primeira fase funcional do Bloco 1)
  - continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
  - `%` adicionado de forma incremental em lexer/token, parser/AST e semântica
  - precedência de `%` alinhada ao grupo multiplicativo (`*`, `/`)
  - lowering e validações atualizadas no pipeline completo: IR, CFG IR, selected, Machine e backend textual
  - interpretador `--run` atualizado para executar `%` e falhar em `%` por zero com a mesma família de erro de divisão por zero
  - cobertura adicionada por camada (lexer/parser/semântica/IR/CFG/selected/machine/interpreter/CLI)
  - exemplo versionado novo: `examples/run_modulo_basico.pink`

- Rodada documental — consolidação da trilha "voltar aos trilhos" (sem fase funcional nova)
  - `docs/roadmap.md` consolidado como trilha única oficial em 5 blocos (fundação -> memória -> saída `.s` -> bare metal -> tooling)
  - precedência operacional explícita: `roadmap.md` (ordem ativa) > `future.md` (inventário amplo)
  - regra de transição e critério de bloco concluído registrados
  - sem alterações em parser/semântica/IR/CFG/selected/Machine/interpreter/backend

- Fase 9 — concluída
  - disciplina de pilha, underflow, consistência entre predecessores, slots/temporários, aridade

- Fase 10 — concluída
  - checagem leve de tipo no topo da pilha (`br_true`, `ret`, unárias/binárias quando inferível)

- Fase 11 — concluída (revalidada nesta rodada)
  - refinamento de tipos de params/slots no checker da Machine
  - regressões tipadas para `call` e `call_void`
  - `cargo build`, `cargo check`, `cargo fmt --check` e `cargo test` passando

- Fase 12 — concluída
  - enriquecimento de contexto/mensagens na validação da Machine (função, bloco, instrução/terminador, esperado vs recebido)
  - cobertura de testes para underflow, tipos incompatíveis, `ret`, `br_true`, slots, `call` e `call_void`

- Fase 13 — concluída
  - interpretador mínimo da Machine com `--run` (execução de `principal` com frame local de slots/pilha e fluxo por labels)
  - suporte inicial a push/load/store, unárias/binárias, comparações numéricas, `jmp`, `br_true`, `ret` e `ret_void`
  - falhas explícitas para `call`, `call_void`, globals e execução multi-função

- Fase 14 — concluída
  - suporte a chamadas entre funções no interpretador (`call` e `call_void`) com frame por função
  - ordem de argumentos preservada no runtime (desempilha + reverse)
  - erro explícito para função inexistente e para `call_void` recebendo retorno

- Fase 15 — concluída
  - suporte a leitura de globals no interpretador (`load_global`) com mapa de globals por execução
  - avaliação mínima de globals literais inteiras/lógicas para `RuntimeValue`
  - erro explícito para global inexistente e para valor global não suportado em runtime

- Fase 16 — concluída
  - 6 testes negativos de runtime via MachineProgram manual: divisão por zero, slot não inicializado, aridade inválida, call/call_void mismatch, valor global não suportado
  - 8 testes end-to-end via run_code: Not, Div, CmpEq, CmpNe, CmpGe, CmpGt, CmpLe, reassignment de variável mutável
  - 1 teste CLI: exit code não-zero e stderr não vazio em erro de runtime
  - pequeno endurecimento: mensagens de erro em call_function incluem o nome da função ([fn_name])

- Fase 17 — concluída
  - cobertura dedicada de recursão no interpretador sem alteração estrutural
  - 4 testes novos em `interpreter_tests`: fatorial, fibonacci, recursão linear e recursão mútua
  - exemplos CLI adicionados: `examples/run_fatorial.pink` e `examples/run_fibonacci.pink`
  - validação end-to-end com `cargo run -- --run` para ambos os exemplos


- Fase 18 — concluída
  - CI mínima adicionada em `.github/workflows/ci.yml`
  - checks de CI: `cargo build --locked`, `cargo check --locked`, `cargo fmt --check`, `cargo test --locked`
  - política de MSRV definida como Rust `1.78.0` em `rust-toolchain.toml` e documentada no README


- Fase 19 — concluída
  - padronização de mensagens de erro entre validadores IR, CFG IR e Machine
  - IR: contexto de função/bloco/instrução e detalhes esperado vs recebido em incompatibilidades de tipo
  - CFG IR: contexto de função/bloco e enriquecimento pontual com detalhe técnico de instrução
  - Machine mantida sem alteração estrutural, com teste garantindo formato contextual estável


- Fase 20 — concluída
  - ampliação de cobertura end-to-end da CLI `--run` com novos exemplos pequenos e auditáveis
  - novos cenários: global+chamada, mutação local+if/else, recursão+global
  - novo cenário inválido de runtime via CLI: divisão por zero com exit code não-zero e stderr
  - manutenção explícita dos exemplos base `run_soma`, `run_chamada`, `run_global`, `run_global_expr`


- Rodada documental — viabilidade de escrita em globals (bloqueada no estado atual)
  - escrita em globals não é viável com o código atual sem expansão de escopo
  - semântica atual trata globals (`eterno`) como não mutáveis
  - Machine não possui `StoreGlobal` (somente `LoadGlobal`)
  - interpretador opera globals via mapa imutável por execução


- Fase 21 — concluída
  - interpretador passou a anexar stack trace simples em erros de runtime
  - stack trace mostra nomes de funções ativas (ordem externa -> interna)
  - cobertura de testes para erro simples, chamada entre funções, recursão e CLI com stderr enriquecido


- Rodada documental — hotfix de auditoria de duplicação (validado)
  - auditoria de duplicação em `tests/interpreter_tests.rs` executada
  - snapshot atual sem duplicatas dos testes/helpers listados
  - stack trace simples de runtime mantido


- Rodada documental — concluída
  - doc comments de módulo adicionados: `interpreter.rs`, `abstract_machine_validate.rs`, `ir_validate.rs`, `cfg_ir_validate.rs`
  - comentários curtos em blocos densos: worklist de pilha, pop_args, attach_runtime_trace, is_temp_slot, enrich_ir_error, validate_block (CFG)
  - nenhuma mudança funcional; todos os comandos de CI passando


- Rodada documental — doc comments estruturais (concluída)
  - doc comments de módulo adicionados: `abstract_machine.rs`, `cfg_ir.rs`, `ir.rs`, `semantic.rs`
  - doc comments em structs/enums centrais: `MachineProgram`, `MachineFunction`, `MachineInstr`, `MachineTerminator`, `BasicBlockIR`, `InstructionCfgIR`, `TerminatorIR`, `TempIR`, `OperandIR`, `FunctionIR`, `BlockIR`, `ValueIR`, `TypeIR`, `InstructionIR`
  - comentários de seção em `semantic.rs`: passagem 1 (declaração), passagem 2 (verificação), análise de alcançabilidade
  - comentários em construtores internos: `FunctionLowerer`/`BlockBuilder` (CFG IR), `LoweringContext`/`FunctionLowerer` (IR), padrão load→op→store (Machine)
  - nenhuma mudança funcional; todos os comandos de CI passando


- Fase 22 — concluída
  - stack trace de runtime evoluiu para frames estruturados (`RuntimeFrame`) em vez de lista ad hoc de strings
  - renderização padronizada via helper (`render_runtime_trace`) no formato `at <função> [bloco: <label>]`
  - mensagem final de erro de runtime preservada com trace estável e legível
  - ganchos leves preparados: `block_label: Option<String>` e `future_span: Option<Span>` por frame (span ainda não preenchido)


- Fase 23 — concluída
  - stack trace passou a incluir contexto da instrução em execução por frame (`[instr: <op>]`) com custo baixo
  - renderização centralizada manteve estabilidade e agora combina função + bloco + instrução no mesmo frame
  - gancho leve adicional preparado: `current_instr: Option<&'static str>` por frame (coleta simples, sem spans completos)


- Fase 24 — concluída
  - mensagem principal de runtime passou a incluir prefixo estável por categoria (`[runtime::<tipo>]`)
  - erros comuns ganharam dica curta e estável (ex.: divisão por zero, slot não inicializado, função/global inexistente, aridade inválida)
  - stack trace existente foi preservado sem mudança de semântica de execução


- Fase 25 — concluída
  - renderização final de erro de runtime no CLI consolidada em helper (`PinkerError::render_for_cli`)
  - layout final de runtime no CLI padronizado em blocos estáveis: `Erro Runtime`, `mensagem`, `stack trace` (quando houver) e `span`
  - mensagem principal categorizada (`[runtime::<tipo>]`) e stack trace por frame foram preservados sem mudança semântica


- Fase 26 — concluída
  - proteção preventiva de profundidade de chamadas no interpretador com limite interno estável (`MAX_CALL_DEPTH = 128`)
  - ao exceder o limite, runtime falha de forma controlada com categoria `[runtime::limite_recursao_excedido]`
  - diagnóstico deixa explícito que é limite preventivo do runtime (não stack overflow real do sistema)
  - stack trace existente e renderização final do CLI foram preservados



- Fase 27 — concluída
  - adicionado suporte de superfície para loop condicional com a forma composta `sempre que <condicao> { ... }`
  - reconhecimento léxico/sintático via keywords `sempre` + `que` no parser
  - novo nó de AST para loop condicional e integração mínima no pipeline (semântica → IR → CFG → seleção → Machine/`--run`)
  - sem novos controles avançados de fluxo (`quebrar`, `continuar`, labels de loop), mantidos fora de escopo


- Fase 28 — concluída
  - truncamento/resumo de stack trace muito longo em erros de runtime
  - política simples: traces com mais de 10 frames são resumidos (primeiros 5 + `... N frames omitidos ...` + últimos 5)
  - traces curtos (≤ 10 frames) permanecem sem alteração
  - linha de omissão indica explicitamente a quantidade de frames omitidos
  - nenhuma mudança de semântica de execução, categorias de erro ou frontend
  - renderização consolidada do CLI (`Erro Runtime`, `mensagem`, `stack trace`, `span`) preservada


- Fase 29 — concluída
  - adicionado suporte mínimo a `quebrar;` dentro de `sempre que`
  - parser/AST reconhecem `quebrar` como statement dedicado
  - semântica rejeita `quebrar` fora de loop com diagnóstico explícito
  - IR estruturada inclui instrução `break` e CFG IR baixa para salto ao `loop_join`
  - execução `--run` interrompe o loop corretamente sem expandir escopo (`continuar`/labels seguem fora)


- Fase 30 — adicionar `continuar` para `sempre que`
  - adicionado suporte mínimo a `continuar;` dentro de `sempre que`
  - parser/AST reconhecem `continuar` como statement dedicado
  - semântica rejeita `continuar` fora de loop com diagnóstico explícito
  - IR estruturada inclui `Continue` com alvo interno de continuidade do loop
  - CFG IR lowera `continuar` para salto ao bloco de condição da próxima iteração
  - execução `--run` passa a pular para a próxima iteração corretamente


- Fase 31 — melhorar spans/source context em erros de runtime e parser
  - erros de runtime com span dummy (`1:1..1:1`) passam a exibir `localização: indisponível (erro detectado na instrução de máquina)` em vez do span inútil
  - adicionado método `render_for_cli_with_source(source)` em `PinkerError`
  - erros de lexer/parser/semântica passam a incluir a linha de origem e indicador de coluna (`^`) no output do CLI
  - `main.rs` atualizado para usar `render_for_cli_with_source` em todos os erros após leitura do arquivo-fonte
  - 3 testes de CLI atualizados para verificar `localização: indisponível` (antes verificavam `span: 1:1..1:1`)
  - 3 novos testes adicionados: source context em parse error (CLI), source context em erro semântico (CLI), localização indisponível em runtime (unitário)
  - formato de runtime e stack trace preservados sem mudança estrutural


- Fase 32 — consolidar exemplos versionados e cobertura CLI para loops com `sempre que`, `quebrar` e `continuar`
  - consolidação da cobertura CLI de loop para usar exemplos versionados do repositório em vez de fontes temporárias ad hoc
  - adição de exemplos mínimos `examples/run_quebrar.pink` e `examples/run_continuar.pink`
  - teste CLI de `sempre que` passa a usar o exemplo versionado existente `examples/run_sempre_que.pink`
  - sem mudanças na semântica de `sempre que`, `quebrar` e `continuar`; apenas consolidação auditável


- Fase 33 — consolidar exemplos versionados e cobertura negativa para loops inválidos, e organizar backlog futuro em `docs/future.md`
  - adicionados exemplos versionados negativos para validação semântica de loop inválido:
    - `examples/check_quebrar_fora_loop.pink`
    - `examples/check_continuar_fora_loop.pink`
  - adicionados testes CLI reprodutíveis com `--check` cobrindo:
    - `quebrar` fora de loop
    - `continuar` fora de loop
  - cobertura positiva de loops (`run_sempre_que`, `run_quebrar`, `run_continuar`) preservada sem alteração semântica
  - `docs/handoff_opus.md` descontinuado com redirecionamento explícito
  - backlog futuro estruturado em `docs/future.md`


- Fase 34 — adicionar operadores bitwise básicos à linguagem Pinker
  - adicionados operadores binários: `&`, `|`, `^`, `<<`, `>>`
  - pipeline atualizado com diff mínimo: lexer/token, parser/AST, semântica, IR, CFG IR, seleção, Machine e interpretador
  - política de tipos adotada: bitwise e shifts válidos apenas para `bombom` (inválidos para `logica`)
  - cobertura incremental adicionada em testes de lexer/parser/semântica/IR/CFG/selected/machine/interpreter
  - novo exemplo mínimo de execução: `examples/run_bitwise_basico.pink`
  - fora de escopo preservado: operadores compostos (`&=`, `|=`, `^=`, `<<=`, `>>=`), `&&`, `||`, novos tipos inteiros e redesign amplo


- Fase 35 — robustez de lowering CFG para `talvez/senao` com fall-through em ambos os ramos
  - continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
  - consolidada cobertura estrutural em `tests/cfg_ir_tests.rs` para `if-else` onde ambos os ramos fazem fall-through e convergem em bloco `join`
  - cobertura end-to-end reforçada com execução CLI de `examples/algoritmo_complexo.pink` em `tests/interpreter_tests.rs`
  - comportamento funcional de lowering/execução mantido (sem nova feature, sem redesign amplo)
  - limite atual mantido: robustez coberta por testes direcionados, sem refactor estrutural do lowerer


- Fase 36 — adicionar operadores lógicos `&&` e `||` com short-circuit
  - continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
  - operadores adicionados ao frontend: `&&` (`AmpAmp`) e `||` (`PipePipe`) no lexer/parser/AST
  - política de tipos adotada: `&&` e `||` aceitam apenas `logica` e retornam `logica` (uso com `bombom` é erro semântico)
  - short-circuit real implementado no lowering CFG: criação de blocos `logic_rhs_*`, `logic_short_*`, `logic_join_*` com desvio condicional sem avaliar o RHS quando não necessário
  - cobertura adicionada em lexer/parser/semântica/IR/CFG/interpreter + exemplos `run_logica_curto_circuito_and.pink` e `run_logica_curto_circuito_or.pink`
  - fora de escopo preservado: truthiness implícito, overloads/coerções complexas e novos operadores compostos


- Fase 37 — adicionar licença do projeto e documentar seu uso básico
  - continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
  - repositório não possuía licença ativa antes desta fase
  - licença MIT adicionada em `LICENSE` (texto padrão reconhecível; sem customização)
  - `Cargo.toml` atualizado com campo `license = "MIT"`
  - `README.md` atualizado com seção curta `## Licença` apontando para `LICENSE`
  - nenhuma mudança de semântica, parser, interpretador ou qualquer camada funcional do compilador


- Fase 38 — humanizar a renderização de `--machine` sem alterar a Machine
  - continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
  - renderização de `--machine` tornou-se substancialmente mais legível para humanos
  - parâmetros e locais do usuário agora exibem nomes limpos (`x, y` em vez de `%x#0, %y#0`)
  - temporários internos do compilador (`%t0`, `%t1`, …) listados separadamente em linha `temps` no cabeçalho da função
  - temporários mantêm formato `%tN` nas instruções — distinção visual clara entre variáveis do usuário e artefatos do compilador
  - blocos recebem anotação de papel como comentário: `entry`, `then_*`, `else_*`, `loop_cond_*`, `loop_*`, `loop_join_*`, `join_*`, `logic_rhs_*`, `logic_short_*`, `logic_join_*`
  - Machine, interpretador, semântica e outras camadas NÃO foram alterados
  - `--selected`, `--cfg-ir`, `--pseudo-asm` e `--run` NÃO foram alterados
  - 7 novos testes adicionados em `abstract_machine_tests.rs`; 4 testes exatos atualizados para novo formato
  - `showcase_completo.pink` validado como caso de inspeção manual — saída visivelmente mais pedagógica


- Fase 39 — humanizar instruções individuais de `--machine` sem alterar semântica
  - continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
  - escopo exclusivo em `render_instr`/`render_term` da camada Machine textual (`vm`/`term`)
  - instrução original foi mantida visível e ganhou comentário curto estável na mesma linha (`; ...`)
  - prioridade 1 coberta: `load_slot`, `store_slot`, `load_global`, `push_int`, `push_bool`, `call`, `call_void`
  - prioridade 2 coberta: aritméticas, unárias, comparações e bitwise (`add/sub/mul/div`, `neg/not`, `cmp_*`, `bitand/bitor/bitxor/shl/shr`)
  - prioridade 3 coberta: terminadores `br_true`, `jmp`, `ret`, `ret_void`
  - `--selected`, `--cfg-ir`, `--pseudo-asm`, `--run`, parser, lowering e interpretador não foram alterados
  - testes da Machine atualizados para novo formato e cobertura adicional por substring para `call`, `br_true`, `jmp`, `ret`, mantendo checks de nomes limpos e linha `temps`


## Fase 40 — contextualizar os comentários de `--machine`
- continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
- `render_instr`/`render_term` agora usa heurísticas simples e baratas para comentários mais contextuais.
- `br_true`: if/loop/curto-circuito com mensagens específicas por labels.
- `jmp`: alvo contextual (`loop_cond`, `loop_join`, `join`/`logic_join`).
- `store_slot`: diferencia temporário `%tN` de variável local do usuário.
- `call` e `call_void`: incluem nome e aridade; `call_void` explicita ausência de retorno.
- `ret` e `ret_void`: comentários ajustados para linguagem mais direta sem esconder terminador.
- Sem alterações em semântica, parser, lowering, interpretador, `--selected`, `--cfg-ir`, `--pseudo-asm` ou `--run`.


## Fase 41 — tornar os comentários de `--machine` sensíveis ao papel do fluxo
- continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
- Escopo mantido estrito em renderização textual de `--machine` (`render_term`/heurísticas de comentário e anotação de bloco).
- `br_true` passou a considerar também o bloco atual para diferenciar melhor `if`, `sempre que` e curto-circuito lógico.
- `jmp` ganhou comentários específicos para `join_*`, `logic_join_*`, `loop_break_cont_*` e `loop_continue_cont_*`, além dos casos já existentes.
- Comentários de blocos de convergência foram ajustados para enfatizar retomada de fluxo (`join_*` e `logic_join_*`).
- Sem alteração de semântica, Machine, lowering, parser, interpretador, opcodes ou flags; `--selected` permaneceu inalterado.


## Rodada documental estratégica — roadmap macro até uso geral/sistemas/self-hosting/kernel
- rodada **não funcional** (sem mudança de parser, semântica, lowering, interpretador, backend ou testes funcionais)
- análise ampla do estado real do workspace concluída com leitura orientada de docs, pipeline e testes
- `docs/roadmap.md` criado como mapa mestre de longo prazo (estado atual, lacunas, dependências, prioridades e critérios de revisão)
- continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
- build e testes revalidados nesta rodada documental

## Fase 48 — ponteiros como categoria de tipo (`seta`)
- continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
- primeiro item funcional do Bloco 2 entregue com escopo mínimo, conservador e auditável.
- frontend atualizado com keyword `seta` e tipo `seta<tipo>` em `parse_type`, reaproveitando os pontos tipados já existentes.
- semântica valida tipo-base de `seta` com resolução normal de aliases/arrays/structs; rejeita base inexistente e mantém compatibilidade estrita entre ponteiros pelo tipo-base.
- decisão explícita desta fase: `seta<seta<T>>` permanece rejeitado com diagnóstico específico para evitar expansão de escopo.
- IR/lowering ganhou categoria `Pointer` (`seta<?>`) somente para transportar informação de tipo no pipeline (assinaturas/slots), sem semântica operacional.
- fora de escopo mantido: dereferência, memória indireta (load/store por endereço), aritmética de ponteiros, campo/indexação via ponteiro, casts (`virar`), `sizeof`/alinhamento, `volatile`, ABI/backend `.s`.

## Fase 48-H1 — hotfixes de corretude e manutenção (rodada extraordinária)
- rodada extraordinária de hotfixes; **não** é fase funcional nova nem rodada documental regular.
- roadmap principal inalterado; registrada como `Fase 48-H1` para preservar continuidade.
- **HF-1** (HIGH): `Type::PartialEq` customizado ignora spans — comparação estrutural pura sem poluição por `Span`.
- **HF-2** (HIGH): `PinkerError::Runtime` usa `Option<Span>` em vez de dummy span `1:1..1:1`; heurística `is_dummy_span` removida.
- **HF-3** (HIGH): runtime rejeita explicitamente tipos signed (`i8`–`i64`) com erro claro, evitando resultados silenciosamente incorretos em representação u64-only.
- **HF-4** (HIGH): validação de range de literais inteiros na semântica (ex.: `300` em contexto `u8` agora é erro).
- **HF-5** (MEDIUM): `main.rs` simplificado com macro `try_or_exit!`, booleanos de necessidade de pipeline e early return após `--check`.
- **HF-6** (MEDIUM): bifurcação `selected→backend_text` vs `machine→interpreter` documentada em comentário inline em `main.rs`.
- **HF-7** (MEDIUM): escopo else-if assimétrico documentado — decisão arquitetural intencional (não é bug).
- **HF-8** (MEDIUM): modelo `KwSempre` + `KwQue` documentado — duas keywords separadas por design.
- **HF-9** (MEDIUM): CI alinhada com `rust-toolchain.toml` — `dtolnay/rust-toolchain@master` com `toolchain: "1.78.0"`.
- **HF-10** (MEDIUM): `clippy` adicionado ao CI e a `rust-toolchain.toml`; 4 warnings corrigidos (only_used_in_recursion, assigning_clones, if_same_then_else, new_without_default).
- **HF-11** (MEDIUM): `cargo doc --no-deps` adicionado ao CI com `-D warnings`.
- **HF-15** (LOW): mensagem de sucesso só aparece quando nenhuma flag de saída está ativa.
- **HF-16** (LOW): `Cargo.toml` authors corrigido com `<>` no email.
- **HF-17** (LOW): `docs/future.md` atualizado para marcar itens já implementados (tipos fixos, seta, ninho, arrays, apelido, bitwise, lógicos) e reordenar prioridades.
