# Handoff Codex (executor)

## Rodada atual
- **Fase 24 implementada**: mensagem principal de erro de runtime enriquecida além do stack trace, preservando o trace existente.

## Objetivo
- Melhorar o diagnóstico de runtime sem refactor grande do interpretador: manter simplicidade e compatibilidade, mas com formato de trace mais estruturado.

## Estado real encontrado
- Continuidade histórica correta: Fase 21a (avaliada/bloqueada) → Fase 21b (concluída) → Fase 22 documental (concluída) → Fase 23a (concluída) → Fase 23b (concluída) → Fase 24 (fase da rodada).
- Workspace local usado como fonte de verdade.
- Base inicial saudável: `cargo build` e `cargo test` passavam antes das mudanças.
- Divergência documental na rodada anterior: `docs/agent_state.md` apontava Fase 22 como fase atual, enquanto o pedido da rodada era Fase 23.

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
