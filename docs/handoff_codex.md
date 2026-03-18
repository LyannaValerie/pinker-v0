# Handoff Codex (executor)

## Rodada atual
- **Fase 26 implementada**: proteção preventiva contra recursão infinita/profundidade excessiva no interpretador com limite interno e erro categorizado, preservando stack trace e renderização de runtime no CLI.

## Objetivo
- Aplicar proteção preventiva de profundidade de chamadas no runtime sem refactor grande do interpretador, mantendo simplicidade, compatibilidade e diagnóstico estável.

## Estado real encontrado
- Continuidade histórica correta: Fase 21a (avaliada/bloqueada) → Fase 21b (concluída) → Fase 22 documental (concluída) → Fase 23a (concluída) → Fase 23b (concluída) → Fase 24 (concluída) → Fase 25 (concluída) → Fase 26 (fase da rodada).
- Workspace local usado como fonte de verdade.
- Base inicial saudável: `cargo build` e `cargo test` passavam antes das mudanças.
- Sem divergência estrutural entre `docs/phases.md`, `docs/agent_state.md` e `docs/handoff_codex.md` nesta rodada; apenas atualização incremental para registrar a Fase 26.

## Ação aplicada
- Introduzida estrutura interna de frame no interpretador (`RuntimeFrame`) com:
  - nome de função,
  - label de bloco opcional,
  - span opcional reservado para uso futuro.
- `call_stack` deixou de ser `Vec<String>` e passou a `Vec<RuntimeFrame>`.
- Atualização do frame atual durante execução de bloco (`block_label = Some(block.label)`).
- Stack trace final passou a ser renderizado por helper dedicado (`render_runtime_trace`) com formato estável:
  - `stack trace:`
  - `  at <função> [bloco: <label>] [instr: <op>]`
- Evolução leve da 23b: `RuntimeFrame` recebeu `current_instr: Option<&'static str>` e o frame ativo é atualizado antes de cada instrução.
- Mantida a proteção contra duplicação de trace ao propagar erro por múltiplos frames.

## Arquivos alterados nesta rodada
- `src/interpreter.rs`
- `tests/interpreter_tests.rs`
- `docs/phases.md`
- `docs/handoff_codex.md`
- `docs/agent_state.md`

## Comandos executados
- `cargo build`
- `cargo check`
- `cargo fmt --check`
- `cargo test`
- `cargo run -q -- --run examples/run_div_zero_cli.pink`

## Resultado
- Comandos de build/check/fmt/test passaram após as mudanças.
- Caso CLI de erro exibe stack trace mais informativo com contexto de bloco + instrução.

## Ganchos futuros preparados (sem implementar agora)
- `RuntimeFrame.block_label: Option<String>` já preenchido quando disponível.
- `RuntimeFrame.current_instr: Option<&'static str>` preenchido de forma leve durante execução.
- `RuntimeFrame.future_span: Option<Span>` reservado para futura evolução com spans por frame.
- `render_runtime_trace` centraliza formatação para evoluções incrementais sem redesign.

## Limites atuais
- Não há debugger/stepping.
- Não há variáveis locais por frame no trace.
- `future_span` ainda não é populado.

## Próximos passos sugeridos
- Quando houver metadado barato de origem por instrução/bloco, preencher `future_span`.
- Opcionalmente enriquecer `current_instr` com origem estrutural (ex.: bloco/offset) mantendo formato textual estável.


## Fase 24 — mensagens de runtime além do stack trace
- Continuidade histórica 21a → 21b → 22 → 23a → 23b → 24 verificada e preservada.
- `runtime_err` passou a enriquecer a mensagem com categoria estável (`[runtime::<tipo>]`) e dica curta para erros frequentes.
- Melhorias aplicadas sem alterar semântica do interpretador: apenas diagnóstico textual.
- Stack trace existente (função + bloco + instrução) foi mantido inalterado.

### Limites mantidos
- Sem spans completos por instrução/frame (gancho `future_span` segue reservado).
- Sem debugger/stepping/tracing avançado.
- Sem mudanças de gramática/frontend/backend nativo.

### Próximos passos sugeridos
- Expandir catálogo de categorias/dicas apenas para erros que já existem no runtime, mantendo testes por substring estável.
- Quando útil, popular `future_span` com origem real da instrução sem inflar arquitetura.


## Fase 25 — consolidação da renderização final no CLI
- Continuidade histórica 21a → 21b → 22 → 23a → 23b → 24 → 25 verificada e preservada.
- Mudança técnica mínima: `PinkerError::render_for_cli()` passou a compor saída final de runtime com layout previsível.
- Layout final no stderr para runtime:
  - `Erro Runtime:`
  - `  mensagem: <mensagem enriquecida>`
  - `stack trace:` + frames indentados (quando houver)
  - `  span: <span>`
- Permanece igual:
  - categoria estável `[runtime::<tipo>]` e dicas curtas na mensagem principal
  - stack trace por frame (`at <função> [bloco: ...] [instr: ...]`)
- Limites mantidos:
  - sem spans ricos por frame, sem debugger/stepping, sem mudança de semântica de execução.
- Próximo passo sugerido (adiado): extrair golden tests dedicados ao renderer de runtime para reduzir acoplamento com testes e2e de CLI.


## Fase 26 — proteção preventiva contra recursão infinita
- Continuidade histórica 21a → 21b → 22 → 23a → 23b → 24 → 25 → 26 verificada e preservada (sem correção estrutural ampla, apenas atualização incremental da fase atual).
- Implementação mínima no interpretador: constante `MAX_CALL_DEPTH = 128` e guarda antes de `call_stack.push(...)` em `call_function`.
- Ao exceder o limite, o runtime retorna erro controlado (sem depender de stack overflow do processo/OS).
- Categoria adotada: `[runtime::limite_recursao_excedido]`.
- Mensagem explicita que o limite é preventivo do runtime e informa profundidade máxima atingida.
- Stack trace existente (função + bloco + instrução) foi preservado e continua observável no erro de limite.
- Renderização final do CLI (`Erro Runtime`, `mensagem`, `stack trace`, `span`) permaneceu a mesma, sem redesign.

### Testes adicionados/ajustados
- `run_falha_limite_recursao_excedido_tem_categoria_e_trace`: garante categoria estável, mensagem de limite preventivo e stack trace útil.
- `cli_run_erro_runtime_limite_recursao_tem_saida_previsivel`: garante saída previsível da CLI para erro de limite de recursão.
- Novo exemplo mínimo: `examples/run_recursao_limite_cli.pink`.

### Limites mantidos
- Limite de profundidade é fixo no código (sem configuração externa nesta fase).
- Sem spans ricos por frame e sem debugger/stepping.

### Próximos passos sugeridos
- Tornar `MAX_CALL_DEPTH` configurável de forma leve (flag/env) sem alterar semântica padrão.
- Adicionar testes dedicados apenas ao renderer de runtime para reduzir acoplamento com e2e de CLI.


## Fase 27b — truncamento/resumo de stack trace longo
- Continuidade histórica 21a → 21b → 22 → 23a → 23b → 24 → 25 → 26 → 27a → 27b verificada e preservada.
- Mudança técnica mínima: `render_runtime_trace` em `src/interpreter.rs` passou a truncar traces longos.

### Política adotada
- `TRACE_TRUNC_THRESHOLD = 10`: traces com ≤ 10 frames são exibidos integralmente (comportamento anterior).
- Traces com > 10 frames: exibe os primeiros `TRACE_HEAD = 5`, uma linha `... N frames omitidos ...`, e os últimos `TRACE_TAIL = 5`.

### O que permaneceu igual
- Traces curtos: comportamento idêntico ao anterior.
- Categorias de erro, mensagens enriquecidas, renderização CLI (`Erro Runtime`, `mensagem`, `stack trace`, `span`).
- Semântica de execução, frontend, gramática.
- `RuntimeFrame`, `attach_runtime_trace`, `set_current_instr`, `machine_instr_name` sem alteração.

### Testes adicionados
- `run_trace_curto_sem_truncamento`: verifica que traces curtos não são truncados.
- `run_trace_longo_e_truncado`: verifica que recursão infinita produz trace com `frames omitidos`.
- `run_trace_longo_preserva_frames_iniciais_e_finais`: verifica que frames iniciais e finais estão presentes.
- `cli_run_limite_recursao_trace_truncado_na_saida`: verifica saída CLI truncada no caso de limite de recursão.

### Limites mantidos
- Limiar e tamanhos de head/tail são fixos no código (sem configuração externa nesta fase).
- Sem spans ricos por frame e sem debugger/stepping.

### Próximos passos sugeridos
- Tornar `TRACE_TRUNC_THRESHOLD`, `TRACE_HEAD`, `TRACE_TAIL` configuráveis via flag/env.
- Adicionar golden tests dedicados ao renderer de runtime.


## Fase 27a — adicionar `sempre que`
- Continuidade histórica 21a → 21b → 22 → 23a → 23b → 24 → 25 → 26 → 27a verificada e preservada.
- Implementação mínima concluída para `sempre que <condicao> { ... }`.

### Como a forma composta foi reconhecida
- Lexer passou a reconhecer `sempre` e `que` como keywords distintas.
- Parser exige o par composto `KwSempre` seguido de `KwQue` no início do statement.

### Partes do pipeline tocadas
- AST: novo `Stmt::While(WhileStmt)` com `condition`, `body` e `span`.
- Semântica: valida condição lógica em `sempre que` e verifica bloco do corpo como bloco normal.
- IR estruturada: novo `InstructionIR::While`.
- Validação IR: condição do loop deve ser `logica` e corpo é validado recursivamente.
- CFG IR: lowering para padrão com `loop_cond_N` (branch), `loop_N` (corpo) e `loop_join_N` (saída).
- Demais camadas permanecem usando infraestrutura existente de branch/jump/back-edge.

### Limites e adiamentos explícitos
- Não foram adicionados `enquanto`, `para`, `quebrar`, `continuar` ou labels de loop.
- Não houve redesign de runtime/diagnóstico; fase restrita ao suporte mínimo do construto.
- Não houve necessidade de correção documental estrutural da timeline; apenas adição incremental da Fase 27a.


## Fase 28a — adicionar `quebrar` para `sempre que`
- Continuidade histórica 21a → 21b → 22 → 23a → 23b → 24 → 25 → 26 → 27a → 27b → 28a verificada e preservada.
- Implementado suporte mínimo a `quebrar;` dentro de `sempre que`.

### Partes do pipeline tocadas
- Lexer/token: nova keyword `quebrar`.
- Parser/AST: novo `Stmt::Break(BreakStmt)`.
- Semântica: validação de contexto de loop (`quebrar` fora de `sempre que` é erro).
- IR estruturada: `InstructionIR::Break { loop_exit_label, span }`.
- CFG IR: lowering de `break` para `jmp` ao bloco de saída do loop (`loop_join_*`).
- Execução (`--run`): comportamento efetivo via CFG/seleção/machine sem nova instrução de runtime.

### O que ficou fora do escopo
- `continuar`, labels de loop, `quebrar` com alvo e redesign de controle de fluxo.
- Mudanças de spans/contexto avançado de diagnóstico.

### Testes adicionados
- Parser: aceita `quebrar` dentro de `sempre que`.
- Semântica: rejeita `quebrar` fora de loop.
- IR/CFG/selected: cobertura mínima de lowering com `quebrar`.
- Interpretador/CLI `--run`: loop é interrompido corretamente ao encontrar `quebrar`.
