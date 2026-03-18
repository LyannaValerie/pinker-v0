# Linha do tempo de fases

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


- Fase 21a — avaliada (bloqueada no estado atual)
  - escrita em globals não é viável com o código atual sem expansão de escopo
  - semântica atual trata globals (`eterno`) como não mutáveis
  - Machine não possui `StoreGlobal` (somente `LoadGlobal`)
  - interpretador opera globals via mapa imutável por execução


- Fase 21b — concluída
  - interpretador passou a anexar stack trace simples em erros de runtime
  - stack trace mostra nomes de funções ativas (ordem externa -> interna)
  - cobertura de testes para erro simples, chamada entre funções, recursão e CLI com stderr enriquecido


- Fase 21b — hotfix validado
  - auditoria de duplicação em `tests/interpreter_tests.rs` executada
  - snapshot atual sem duplicatas dos testes/helpers listados
  - stack trace simples de runtime mantido


- Rodada documental — concluída
  - doc comments de módulo adicionados: `interpreter.rs`, `abstract_machine_validate.rs`, `ir_validate.rs`, `cfg_ir_validate.rs`
  - comentários curtos em blocos densos: worklist de pilha, pop_args, attach_runtime_trace, is_temp_slot, enrich_ir_error, validate_block (CFG)
  - nenhuma mudança funcional; todos os comandos de CI passando


- Fase 22 documental — concluída
  - doc comments de módulo adicionados: `abstract_machine.rs`, `cfg_ir.rs`, `ir.rs`, `semantic.rs`
  - doc comments em structs/enums centrais: `MachineProgram`, `MachineFunction`, `MachineInstr`, `MachineTerminator`, `BasicBlockIR`, `InstructionCfgIR`, `TerminatorIR`, `TempIR`, `OperandIR`, `FunctionIR`, `BlockIR`, `ValueIR`, `TypeIR`, `InstructionIR`
  - comentários de seção em `semantic.rs`: passagem 1 (declaração), passagem 2 (verificação), análise de alcançabilidade
  - comentários em construtores internos: `FunctionLowerer`/`BlockBuilder` (CFG IR), `LoweringContext`/`FunctionLowerer` (IR), padrão load→op→store (Machine)
  - nenhuma mudança funcional; todos os comandos de CI passando


- Fase 23a — concluída
  - stack trace de runtime evoluiu para frames estruturados (`RuntimeFrame`) em vez de lista ad hoc de strings
  - renderização padronizada via helper (`render_runtime_trace`) no formato `at <função> [bloco: <label>]`
  - mensagem final de erro de runtime preservada com trace estável e legível
  - ganchos leves preparados: `block_label: Option<String>` e `future_span: Option<Span>` por frame (span ainda não preenchido)


- Fase 23b — concluída
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



- Fase 27a — concluída
  - adicionado suporte de superfície para loop condicional com a forma composta `sempre que <condicao> { ... }`
  - reconhecimento léxico/sintático via keywords `sempre` + `que` no parser
  - novo nó de AST para loop condicional e integração mínima no pipeline (semântica → IR → CFG → seleção → Machine/`--run`)
  - sem novos controles avançados de fluxo (`quebrar`, `continuar`, labels de loop), mantidos fora de escopo


- Fase 27b — concluída
  - truncamento/resumo de stack trace muito longo em erros de runtime
  - política simples: traces com mais de 10 frames são resumidos (primeiros 5 + `... N frames omitidos ...` + últimos 5)
  - traces curtos (≤ 10 frames) permanecem sem alteração
  - linha de omissão indica explicitamente a quantidade de frames omitidos
  - nenhuma mudança de semântica de execução, categorias de erro ou frontend
  - renderização consolidada do CLI (`Erro Runtime`, `mensagem`, `stack trace`, `span`) preservada


- Fase 28a — concluída
  - adicionado suporte mínimo a `quebrar;` dentro de `sempre que`
  - parser/AST reconhecem `quebrar` como statement dedicado
  - semântica rejeita `quebrar` fora de loop com diagnóstico explícito
  - IR estruturada inclui instrução `break` e CFG IR baixa para salto ao `loop_join`
  - execução `--run` interrompe o loop corretamente sem expandir escopo (`continuar`/labels seguem fora)
